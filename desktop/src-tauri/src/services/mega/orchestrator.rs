//! Mega Plan Orchestrator
//!
//! Orchestrates multi-feature development with dependency-aware batch execution.
//! Creates isolated worktrees for each feature and manages parallel execution.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures_util::future::join_all;
use tokio::sync::{mpsc, RwLock};
use tokio_util::sync::CancellationToken;

use crate::models::mega::{
    Feature, FeatureState, FeatureStatus, MegaExecutionStatus, MegaPlan, MegaStatus,
};
use crate::models::worktree::CreateWorktreeRequest;
use crate::services::worktree::WorktreeManager;
use crate::models::prd::Prd;

/// Configuration for the mega orchestrator
#[derive(Debug, Clone)]
pub struct MegaOrchestratorConfig {
    /// Project root directory
    pub project_root: PathBuf,
    /// Maximum concurrent features
    pub max_concurrent: usize,
    /// Whether to auto-generate PRDs
    pub auto_generate_prds: bool,
    /// Poll interval for status updates (seconds)
    pub poll_interval_seconds: u64,
}

impl Default for MegaOrchestratorConfig {
    fn default() -> Self {
        Self {
            project_root: PathBuf::from("."),
            max_concurrent: 3,
            auto_generate_prds: true,
            poll_interval_seconds: 5,
        }
    }
}

/// Errors from mega orchestrator operations
#[derive(Debug, thiserror::Error)]
pub enum MegaOrchestratorError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Worktree error: {0}")]
    WorktreeError(String),

    #[error("PRD generation error: {0}")]
    PrdGenerationError(String),

    #[error("Feature execution error: {0}")]
    FeatureExecutionError(String),

    #[error("Dependency error: {0}")]
    DependencyError(String),

    #[error("Cancelled")]
    Cancelled,

    #[error("Invalid state: {0}")]
    InvalidState(String),
}

/// Events emitted during mega orchestration
#[derive(Debug, Clone)]
pub enum MegaEvent {
    /// Orchestration started
    Started { plan_id: String },
    /// Batch started
    BatchStarted { batch_index: usize, feature_count: usize },
    /// Feature worktree created
    FeatureWorktreeCreated { feature_id: String, path: PathBuf },
    /// PRD generated for feature
    PrdGenerated { feature_id: String, prd_path: PathBuf },
    /// Feature execution started
    FeatureStarted { feature_id: String },
    /// Feature story completed
    FeatureStoryCompleted { feature_id: String, story_id: String },
    /// Feature completed
    FeatureCompleted { feature_id: String },
    /// Feature failed
    FeatureFailed { feature_id: String, error: String },
    /// Batch completed
    BatchCompleted { batch_index: usize },
    /// Orchestration completed
    Completed { success: bool },
    /// Progress update
    Progress { completed: usize, total: usize, percentage: f32 },
    /// Error occurred
    Error { message: String },
}

/// Mega plan orchestrator for multi-feature development
pub struct MegaOrchestrator {
    /// Configuration
    config: MegaOrchestratorConfig,
    /// Worktree manager
    worktree_manager: WorktreeManager,
    /// The mega plan being executed
    mega_plan: Arc<RwLock<MegaPlan>>,
    /// Execution status
    status: Arc<RwLock<MegaStatus>>,
    /// Cancellation token
    cancellation_token: CancellationToken,
}

impl MegaOrchestrator {
    /// Create a new mega orchestrator
    pub fn new(config: MegaOrchestratorConfig, mega_plan: MegaPlan) -> Self {
        let plan_id = mega_plan.name.clone();

        Self {
            config,
            worktree_manager: WorktreeManager::new(),
            mega_plan: Arc::new(RwLock::new(mega_plan)),
            status: Arc::new(RwLock::new(MegaStatus::new(plan_id))),
            cancellation_token: CancellationToken::new(),
        }
    }

    /// Get the cancellation token
    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancellation_token.clone()
    }

    /// Cancel execution
    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }

    /// Get current status
    pub async fn get_status(&self) -> MegaStatus {
        self.status.read().await.clone()
    }

    /// Execute the mega plan with full automation
    pub async fn execute_auto(
        &self,
        event_tx: mpsc::Sender<MegaEvent>,
    ) -> Result<MegaStatus, MegaOrchestratorError> {
        // Start execution
        {
            let mut status = self.status.write().await;
            status.start();
        }

        let plan_id = {
            let plan = self.mega_plan.read().await;
            plan.name.clone()
        };

        let _ = event_tx.send(MegaEvent::Started { plan_id }).await;

        loop {
            // Check for cancellation
            if self.cancellation_token.is_cancelled() {
                let mut status = self.status.write().await;
                status.status = MegaExecutionStatus::Cancelled;
                return Err(MegaOrchestratorError::Cancelled);
            }

            // Get current batch of features
            let batch = self.get_current_batch().await;
            if batch.is_empty() {
                break; // All batches complete
            }

            let batch_index = {
                self.status.read().await.current_batch
            };

            let _ = event_tx
                .send(MegaEvent::BatchStarted {
                    batch_index,
                    feature_count: batch.len(),
                })
                .await;

            // Create worktrees for all features in batch
            let target_branch = {
                let plan = self.mega_plan.read().await;
                plan.target_branch.clone()
            };

            for feature in &batch {
                if self.cancellation_token.is_cancelled() {
                    return Err(MegaOrchestratorError::Cancelled);
                }

                match self
                    .create_feature_worktree(&feature.id, &target_branch)
                    .await
                {
                    Ok(path) => {
                        let mut status = self.status.write().await;
                        let state = status
                            .features
                            .entry(feature.id.clone())
                            .or_insert_with(FeatureState::default);
                        state.worktree = Some(path.clone());

                        let _ = event_tx
                            .send(MegaEvent::FeatureWorktreeCreated {
                                feature_id: feature.id.clone(),
                                path,
                            })
                            .await;
                    }
                    Err(e) => {
                        let _ = event_tx
                            .send(MegaEvent::FeatureFailed {
                                feature_id: feature.id.clone(),
                                error: e.to_string(),
                            })
                            .await;
                    }
                }
            }

            // Generate PRDs for each feature if configured
            if self.config.auto_generate_prds {
                self.generate_prds_parallel(&batch, event_tx.clone()).await?;
            }

            // Execute features in parallel
            self.execute_features_parallel(&batch, event_tx.clone()).await?;

            // Wait for batch completion
            self.wait_for_batch_completion(&batch).await?;

            // Complete and merge features
            for feature in &batch {
                if self.cancellation_token.is_cancelled() {
                    return Err(MegaOrchestratorError::Cancelled);
                }

                let feature_status = {
                    let status = self.status.read().await;
                    status
                        .features
                        .get(&feature.id)
                        .map(|s| s.status)
                        .unwrap_or(FeatureStatus::Pending)
                };

                if feature_status == FeatureStatus::InProgress {
                    match self.complete_feature(&feature.id).await {
                        Ok(_) => {
                            let mut status = self.status.write().await;
                            if let Some(state) = status.features.get_mut(&feature.id) {
                                state.complete();
                            }

                            let _ = event_tx
                                .send(MegaEvent::FeatureCompleted {
                                    feature_id: feature.id.clone(),
                                })
                                .await;
                        }
                        Err(e) => {
                            let mut status = self.status.write().await;
                            if let Some(state) = status.features.get_mut(&feature.id) {
                                state.fail(e.to_string());
                            }

                            let _ = event_tx
                                .send(MegaEvent::FeatureFailed {
                                    feature_id: feature.id.clone(),
                                    error: e.to_string(),
                                })
                                .await;
                        }
                    }
                }
            }

            // Advance to next batch
            {
                let mut status = self.status.write().await;
                let current_batch = status.current_batch;
                status.completed_batches.push(current_batch);
                status.current_batch += 1;
            }

            let _ = event_tx
                .send(MegaEvent::BatchCompleted { batch_index })
                .await;

            // Emit progress
            let progress = {
                let status = self.status.read().await;
                status.completion_percentage()
            };

            let (completed, total) = {
                let status = self.status.read().await;
                let completed = status
                    .features
                    .values()
                    .filter(|s| s.status == FeatureStatus::Completed)
                    .count();
                let total = status.features.len();
                (completed, total)
            };

            let _ = event_tx
                .send(MegaEvent::Progress {
                    completed,
                    total,
                    percentage: progress,
                })
                .await;

            // Save status
            self.save_status().await?;
        }

        // Complete execution
        {
            let mut status = self.status.write().await;
            status.complete();
        }

        let success = {
            let status = self.status.read().await;
            status
                .features
                .values()
                .all(|s| s.status == FeatureStatus::Completed)
        };

        let _ = event_tx.send(MegaEvent::Completed { success }).await;

        Ok(self.status.read().await.clone())
    }

    /// Get features for the current batch based on dependencies
    pub async fn get_current_batch(&self) -> Vec<Feature> {
        let plan = self.mega_plan.read().await;
        let status = self.status.read().await;

        let completed: HashSet<String> = status
            .features
            .iter()
            .filter(|(_, s)| s.status == FeatureStatus::Completed)
            .map(|(id, _)| id.clone())
            .collect();

        plan.features
            .iter()
            .filter(|f| {
                // Not already completed or in progress
                let feature_status = status
                    .features
                    .get(&f.id)
                    .map(|s| s.status)
                    .unwrap_or(FeatureStatus::Pending);

                feature_status == FeatureStatus::Pending
                    && f.dependencies.iter().all(|dep| completed.contains(dep))
            })
            .take(self.config.max_concurrent)
            .cloned()
            .collect()
    }

    /// Create a worktree for a feature
    async fn create_feature_worktree(
        &self,
        feature_id: &str,
        target_branch: &str,
    ) -> Result<PathBuf, MegaOrchestratorError> {
        let request = CreateWorktreeRequest {
            task_name: feature_id.to_string(),
            target_branch: target_branch.to_string(),
            base_path: None,
            prd_path: None,
            execution_mode: "auto".to_string(),
        };

        let worktree = self
            .worktree_manager
            .create_worktree(&self.config.project_root, request)
            .await
            .map_err(|e| MegaOrchestratorError::WorktreeError(e.to_string()))?;

        Ok(PathBuf::from(worktree.path))
    }

    /// Generate PRDs for features in parallel
    pub async fn generate_prds_parallel(
        &self,
        features: &[Feature],
        event_tx: mpsc::Sender<MegaEvent>,
    ) -> Result<(), MegaOrchestratorError> {

        let tasks: Vec<_> = features
            .iter()
            .map(|f| {
                let feature_id = f.id.clone();
                let description = f.description.clone();
                let event_tx = event_tx.clone();
                let status = Arc::clone(&self.status);
                let cancellation_token = self.cancellation_token.clone();

                async move {
                    if cancellation_token.is_cancelled() {
                        return Err(MegaOrchestratorError::Cancelled);
                    }

                    // Get worktree path
                    let worktree_path = {
                        let status = status.read().await;
                        status
                            .features
                            .get(&feature_id)
                            .and_then(|s| s.worktree.clone())
                    };

                    if let Some(wt_path) = worktree_path {
                        // Generate a placeholder PRD
                        // In a real implementation, this would call an LLM agent
                        let prd = Prd::new(&feature_id);
                        let prd_path = wt_path.join("prd.json");

                        prd.to_file(&prd_path)
                            .map_err(|e| MegaOrchestratorError::PrdGenerationError(e.to_string()))?;

                        // Update status
                        {
                            let mut status = status.write().await;
                            if let Some(state) = status.features.get_mut(&feature_id) {
                                state.prd_generated = true;
                                state.prd_path = Some(prd_path.clone());
                            }
                        }

                        let _ = event_tx
                            .send(MegaEvent::PrdGenerated {
                                feature_id: feature_id.clone(),
                                prd_path,
                            })
                            .await;
                    }

                    Ok::<_, MegaOrchestratorError>(())
                }
            })
            .collect();

        let results = join_all(tasks).await;

        for result in results {
            result?;
        }

        Ok(())
    }

    /// Execute features in parallel
    pub async fn execute_features_parallel(
        &self,
        features: &[Feature],
        event_tx: mpsc::Sender<MegaEvent>,
    ) -> Result<(), MegaOrchestratorError> {

        let tasks: Vec<_> = features
            .iter()
            .map(|f| {
                let feature_id = f.id.clone();
                let event_tx = event_tx.clone();
                let status = Arc::clone(&self.status);
                let cancellation_token = self.cancellation_token.clone();

                async move {
                    if cancellation_token.is_cancelled() {
                        return Err(MegaOrchestratorError::Cancelled);
                    }

                    // Mark feature as in progress
                    {
                        let mut status = status.write().await;
                        let state = status
                            .features
                            .entry(feature_id.clone())
                            .or_insert_with(FeatureState::default);
                        state.start();
                    }

                    let _ = event_tx
                        .send(MegaEvent::FeatureStarted {
                            feature_id: feature_id.clone(),
                        })
                        .await;

                    // In a real implementation, this would:
                    // 1. Load the PRD from the worktree
                    // 2. Use DependencyAnalyzer to generate story batches
                    // 3. Execute stories using IterationLoop
                    // 4. Run quality gates
                    // For now, we just mark it as ready for completion

                    Ok::<_, MegaOrchestratorError>(())
                }
            })
            .collect();

        let results = join_all(tasks).await;

        for result in results {
            result?;
        }

        Ok(())
    }

    /// Wait for all features in a batch to complete
    async fn wait_for_batch_completion(
        &self,
        features: &[Feature],
    ) -> Result<(), MegaOrchestratorError> {
        // In a real implementation, this would poll the status
        // and wait for all features to complete or fail
        // For now, we assume immediate completion

        let feature_ids: HashSet<_> = features.iter().map(|f| f.id.clone()).collect();

        loop {
            if self.cancellation_token.is_cancelled() {
                return Err(MegaOrchestratorError::Cancelled);
            }

            let all_done = {
                let status = self.status.read().await;
                feature_ids.iter().all(|id| {
                    status.features.get(id).map_or(false, |s| {
                        s.status == FeatureStatus::Completed
                            || s.status == FeatureStatus::Failed
                            || s.status == FeatureStatus::InProgress // For now, treat in_progress as "done" since we don't have real execution
                    })
                })
            };

            if all_done {
                break;
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(
                self.config.poll_interval_seconds,
            ))
            .await;
        }

        Ok(())
    }

    /// Complete a feature (commit, merge, cleanup)
    async fn complete_feature(&self, feature_id: &str) -> Result<(), MegaOrchestratorError> {
        let worktree_path = {
            let status = self.status.read().await;
            status
                .features
                .get(feature_id)
                .and_then(|s| s.worktree.clone())
        };

        if let Some(_path) = worktree_path {
            // Complete the worktree
            let commit_message = format!("feat({}): Complete feature implementation", feature_id);

            self.worktree_manager
                .complete_worktree(&self.config.project_root, feature_id, Some(&commit_message))
                .await
                .map_err(|e| MegaOrchestratorError::WorktreeError(e.to_string()))?;
        }

        Ok(())
    }

    /// Save current status to file
    async fn save_status(&self) -> Result<(), MegaOrchestratorError> {
        let status = self.status.read().await;
        let status_path = self.config.project_root.join(".mega-status.json");

        status
            .to_file(&status_path)
            .map_err(|e| MegaOrchestratorError::IoError(std::io::Error::other(e.to_string())))?;

        Ok(())
    }

    /// Load status from file
    pub async fn load_status(
        project_root: &Path,
    ) -> Result<Option<MegaStatus>, MegaOrchestratorError> {
        let status_path = project_root.join(".mega-status.json");

        if !status_path.exists() {
            return Ok(None);
        }

        let status = MegaStatus::from_file(&status_path)
            .map_err(|e| MegaOrchestratorError::IoError(std::io::Error::other(e.to_string())))?;

        Ok(Some(status))
    }

    /// Resume execution from saved status
    pub async fn resume(
        &self,
        event_tx: mpsc::Sender<MegaEvent>,
    ) -> Result<MegaStatus, MegaOrchestratorError> {
        // Load saved status
        if let Some(saved_status) = Self::load_status(&self.config.project_root).await? {
            // Update our status with the saved one
            {
                let mut status = self.status.write().await;
                *status = saved_status;
                status.status = MegaExecutionStatus::Running;
            }
        }

        // Continue execution
        self.execute_auto(event_tx).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_plan() -> MegaPlan {
        let mut plan = MegaPlan::new("Test Project");

        // F001 -> no deps
        plan.add_feature(Feature::new("F001", "User Authentication"));

        // F002 -> F001
        let mut f2 = Feature::new("F002", "User Profile");
        f2.dependencies = vec!["F001".to_string()];
        plan.add_feature(f2);

        // F003 -> no deps
        plan.add_feature(Feature::new("F003", "Database Setup"));

        // F004 -> F001, F003
        let mut f4 = Feature::new("F004", "Data API");
        f4.dependencies = vec!["F001".to_string(), "F003".to_string()];
        plan.add_feature(f4);

        plan
    }

    #[tokio::test]
    async fn test_orchestrator_creation() {
        let plan = create_test_plan();
        let config = MegaOrchestratorConfig::default();
        let orchestrator = MegaOrchestrator::new(config, plan);

        let status = orchestrator.get_status().await;
        assert_eq!(status.status, MegaExecutionStatus::Pending);
    }

    #[tokio::test]
    async fn test_get_current_batch() {
        let plan = create_test_plan();
        let config = MegaOrchestratorConfig::default();
        let orchestrator = MegaOrchestrator::new(config, plan);

        let batch = orchestrator.get_current_batch().await;

        // F001 and F003 have no dependencies, so they should be in the first batch
        assert_eq!(batch.len(), 2);
        assert!(batch.iter().any(|f| f.id == "F001"));
        assert!(batch.iter().any(|f| f.id == "F003"));
    }

    #[tokio::test]
    async fn test_cancellation() {
        let plan = create_test_plan();
        let config = MegaOrchestratorConfig::default();
        let orchestrator = MegaOrchestrator::new(config, plan);

        orchestrator.cancel();
        assert!(orchestrator.cancellation_token.is_cancelled());
    }
}
