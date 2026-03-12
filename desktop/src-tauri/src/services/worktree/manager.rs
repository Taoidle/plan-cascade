//! Worktree Manager
//!
//! Manages git worktrees as first-class session runtimes.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sha2::{Digest, Sha256};
use tokio::sync::RwLock;
use url::Url;

use crate::models::worktree::{
    CompleteWorktreeResult, CreateManagedWorktreeRequest, CreatePullRequestRequest,
    CreatePullRequestResult, CreateWorktreeRequest, ForgeProvider, ManagedWorktreeMetadata,
    MergeConflict, PreparePullRequestResult, PullRequestInfo, PullRequestState,
    SessionWorktreeRuntimeBinding, Worktree, WorktreeCleanupPolicy, WorktreeRuntimeKind,
    WorktreeStatus,
};
use crate::utils::error::{AppError, AppResult};
use crate::utils::paths::ensure_worktrees_dir;

use super::config::PlanningConfigService;
use super::git_ops::{GitOps, MergeResult};

const METADATA_FILE: &str = ".plan-cascade-worktree.json";

/// Manager for git worktree lifecycle operations
pub struct WorktreeManager {
    git: GitOps,
    config_service: PlanningConfigService,
    worktrees: Arc<RwLock<HashMap<String, Worktree>>>,
}

impl WorktreeManager {
    pub fn new() -> Self {
        Self {
            git: GitOps::new(),
            config_service: PlanningConfigService::new(),
            worktrees: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Default managed worktree root for a repository.
    pub fn get_worktree_base(&self, repo_path: &Path) -> PathBuf {
        let repo_root = self
            .canonical_repo_root(repo_path)
            .unwrap_or_else(|_| repo_path.to_path_buf());
        let repo_id = self
            .compute_repo_id(&repo_root)
            .unwrap_or_else(|_| self.hash_input(&repo_root.to_string_lossy()));
        ensure_worktrees_dir()
            .unwrap_or_else(|_| PathBuf::from(".").join(".plan-cascade").join("worktrees"))
            .join(repo_id)
    }

    pub async fn create_worktree(
        &self,
        repo_path: &Path,
        request: CreateWorktreeRequest,
    ) -> AppResult<Worktree> {
        let repo_root = self.canonical_repo_root(repo_path)?;
        let sanitized_name = self.sanitize_task_name(&request.task_name);
        let base_path = request
            .base_path
            .map(PathBuf::from)
            .unwrap_or_else(|| self.get_worktree_base(&repo_root));
        let worktree_path = base_path.join(&sanitized_name);
        if worktree_path.exists() {
            return Err(AppError::validation(format!(
                "Worktree directory already exists: {}",
                worktree_path.display()
            )));
        }

        let branch_name = self.derive_branch_name(&sanitized_name, None);
        self.ensure_target_branch_exists(&repo_root, &request.target_branch)?;
        if self.git.branch_exists(&repo_root, &branch_name)? {
            return Err(AppError::validation(format!(
                "Branch already exists: {}",
                branch_name
            )));
        }
        fs::create_dir_all(&base_path)?;

        self.git.add_worktree_with_new_branch(
            &repo_root,
            &worktree_path,
            &branch_name,
            &request.target_branch,
        )?;

        let mut worktree = Worktree::new(
            sanitized_name.clone(),
            request.task_name.clone(),
            worktree_path.to_string_lossy().to_string(),
            branch_name,
            request.target_branch.clone(),
        );
        let config = self.config_service.create(
            &worktree_path,
            &request.task_name,
            &request.target_branch,
            request.prd_path.as_deref(),
            &request.execution_mode,
        )?;
        worktree.root_path = Some(repo_root.to_string_lossy().to_string());
        worktree.repo_id = Some(self.compute_repo_id(&repo_root)?);
        worktree.planning_config = Some(config);
        worktree.set_status(WorktreeStatus::Active);
        worktree.runtime_kind = if self.is_managed_runtime_path(&repo_root, &worktree_path) {
            WorktreeRuntimeKind::Managed
        } else {
            WorktreeRuntimeKind::Legacy
        };

        self.cache_worktree(&worktree).await;
        Ok(worktree)
    }

    pub async fn create_managed_worktree_for_session(
        &self,
        repo_path: &Path,
        request: CreateManagedWorktreeRequest,
    ) -> AppResult<SessionWorktreeRuntimeBinding> {
        let repo_root = self.canonical_repo_root(repo_path)?;
        let repo_id = self.compute_repo_id(&repo_root)?;
        let task_slug = self.sanitize_task_name(&request.task_name);
        let worktree_id = self.derive_worktree_id(&request.session_id, &task_slug);
        let branch_name = request
            .branch_name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| self.derive_branch_name(&task_slug, Some(&request.session_id)));
        self.validate_branch_name(&branch_name)?;
        self.ensure_target_branch_exists(&repo_root, &request.target_branch)?;
        if self.git.branch_exists(&repo_root, &branch_name)? {
            return Err(AppError::validation(format!(
                "Branch already exists: {}",
                branch_name
            )));
        }

        let worktree_path = self.get_worktree_base(&repo_root).join(&worktree_id);
        if worktree_path.exists() {
            return Err(AppError::validation(format!(
                "Managed worktree directory already exists: {}",
                worktree_path.display()
            )));
        }
        if let Some(parent) = worktree_path.parent() {
            fs::create_dir_all(parent)?;
        }

        self.git.add_worktree_with_new_branch(
            &repo_root,
            &worktree_path,
            &branch_name,
            &request.target_branch,
        )?;

        let _ = self.config_service.create(
            &worktree_path,
            &request.task_name,
            &request.target_branch,
            request.prd_path.as_deref(),
            &request.execution_mode,
        )?;

        let mut metadata = ManagedWorktreeMetadata::new(
            &worktree_id,
            &repo_id,
            repo_root.to_string_lossy(),
            worktree_path.to_string_lossy(),
            &branch_name,
            &request.target_branch,
            &request.session_id,
        );
        metadata.cleanup_policy = request.cleanup_policy;
        metadata.display_label = request.display_label.clone();
        self.persist_metadata(&metadata)?;

        let worktree = self.managed_metadata_to_worktree(&metadata, None);
        self.cache_worktree(&worktree).await;

        Ok(metadata.to_runtime_binding(WorktreeRuntimeKind::Managed))
    }

    pub async fn attach_existing_worktree_to_session(
        &self,
        repo_path: &Path,
        worktree_path: &Path,
        session_id: &str,
        display_label: Option<String>,
        cleanup_policy: WorktreeCleanupPolicy,
    ) -> AppResult<SessionWorktreeRuntimeBinding> {
        let repo_root = self.canonical_repo_root(repo_path)?;
        let worktree_repo_root = self.canonical_repo_root(worktree_path)?;
        if worktree_repo_root != repo_root {
            return Err(AppError::validation(format!(
                "Worktree '{}' does not belong to repository '{}'",
                worktree_path.display(),
                repo_root.display()
            )));
        }

        if let Some(mut metadata) = self.read_metadata(worktree_path)? {
            metadata.session_id = session_id.to_string();
            metadata.display_label = display_label;
            metadata.cleanup_policy = cleanup_policy;
            metadata.touch();
            self.persist_metadata(&metadata)?;
            let worktree = self.managed_metadata_to_worktree(&metadata, None);
            self.cache_worktree(&worktree).await;
            return Ok(metadata.to_runtime_binding(WorktreeRuntimeKind::Managed));
        }

        let repo_id = self.compute_repo_id(&repo_root)?;
        let branch = self.git.get_current_branch(worktree_path)?;
        let worktree_id = worktree_path
            .file_name()
            .and_then(|value| value.to_str())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| self.hash_input(&worktree_path.to_string_lossy()));
        Ok(SessionWorktreeRuntimeBinding {
            session_id: session_id.to_string(),
            repo_id,
            root_path: repo_root.to_string_lossy().to_string(),
            runtime_path: worktree_path.to_string_lossy().to_string(),
            worktree_id,
            branch,
            target_branch: self.default_target_branch(&repo_root)?,
            runtime_kind: WorktreeRuntimeKind::Legacy,
            cleanup_policy,
            display_label,
            pr_info: None,
        })
    }

    pub async fn detach_worktree_from_session(
        &self,
        repo_path: &Path,
        worktree_id: &str,
    ) -> AppResult<()> {
        let repo_root = self.canonical_repo_root(repo_path)?;
        let Some(worktree) = self.find_worktree_by_id(&repo_root, worktree_id).await? else {
            return Ok(());
        };
        if worktree.runtime_kind != WorktreeRuntimeKind::Managed {
            return Ok(());
        }
        let wt_path = PathBuf::from(&worktree.path);
        if let Some(mut metadata) = self.read_metadata(&wt_path)? {
            metadata.session_id.clear();
            metadata.touch();
            self.persist_metadata(&metadata)?;
        }
        Ok(())
    }

    pub async fn list_worktrees(&self, repo_path: &Path) -> AppResult<Vec<Worktree>> {
        let repo_root = self.canonical_repo_root(repo_path)?;
        let repo_id = self.compute_repo_id(&repo_root)?;
        let managed_base = self.get_worktree_base(&repo_root);
        let git_worktrees = self.git.list_worktrees(&repo_root)?;
        let mut worktrees = Vec::new();

        for git_wt in git_worktrees {
            if git_wt.path == repo_root.to_string_lossy() {
                continue;
            }
            if git_wt.is_bare || git_wt.is_prunable {
                continue;
            }

            let wt_path = PathBuf::from(&git_wt.path);
            let planning_config = self.config_service.read(&wt_path).ok();
            let status = if let Some(ref config) = planning_config {
                match config.phase {
                    crate::models::worktree::PlanningPhase::Complete => WorktreeStatus::Ready,
                    crate::models::worktree::PlanningPhase::Executing => WorktreeStatus::InProgress,
                    _ => WorktreeStatus::Active,
                }
            } else {
                WorktreeStatus::Active
            };

            let (created_at, updated_at) = self.get_worktree_timestamps(&wt_path);
            let metadata = self.read_metadata(&wt_path)?;
            let task_name = git_wt
                .branch
                .strip_prefix("pc/")
                .or_else(|| git_wt.branch.strip_prefix("worktree/"))
                .unwrap_or(&git_wt.branch)
                .to_string();

            let mut worktree = if let Some(metadata) = metadata {
                self.managed_metadata_to_worktree(&metadata, planning_config)
            } else {
                Worktree {
                    id: task_name.clone(),
                    name: planning_config
                        .as_ref()
                        .map(|config| config.task_name.clone())
                        .unwrap_or_else(|| task_name.clone()),
                    path: git_wt.path.clone(),
                    branch: git_wt.branch.clone(),
                    target_branch: planning_config
                        .as_ref()
                        .map(|config| config.target_branch.clone())
                        .unwrap_or_else(|| self.default_target_branch(&repo_root).unwrap_or_else(|_| "main".to_string())),
                    status,
                    created_at,
                    updated_at,
                    error: None,
                    planning_config,
                    repo_id: Some(repo_id.clone()),
                    root_path: Some(repo_root.to_string_lossy().to_string()),
                    session_id: None,
                    runtime_kind: if wt_path.starts_with(&managed_base) {
                        WorktreeRuntimeKind::Managed
                    } else {
                        WorktreeRuntimeKind::Legacy
                    },
                    cleanup_policy: WorktreeCleanupPolicy::Manual,
                    pr_info: None,
                    display_label: None,
                }
            };
            worktree.status = status;
            if worktree.root_path.is_none() {
                worktree.root_path = Some(repo_root.to_string_lossy().to_string());
            }
            if worktree.repo_id.is_none() {
                worktree.repo_id = Some(repo_id.clone());
            }
            worktrees.push(worktree);
        }

        Ok(worktrees)
    }

    pub async fn list_managed_worktrees_for_repo(&self, repo_path: &Path) -> AppResult<Vec<Worktree>> {
        Ok(self
            .list_worktrees(repo_path)
            .await?
            .into_iter()
            .filter(|worktree| worktree.runtime_kind == WorktreeRuntimeKind::Managed)
            .collect())
    }

    pub async fn detect_legacy_worktrees(&self, repo_path: &Path) -> AppResult<Vec<Worktree>> {
        Ok(self
            .list_worktrees(repo_path)
            .await?
            .into_iter()
            .filter(|worktree| worktree.runtime_kind == WorktreeRuntimeKind::Legacy)
            .collect())
    }

    pub async fn get_worktree(&self, repo_path: &Path, worktree_id: &str) -> AppResult<Worktree> {
        let repo_root = self.canonical_repo_root(repo_path)?;
        let Some(worktree) = self.find_worktree_by_id(&repo_root, worktree_id).await? else {
            return Err(AppError::not_found(format!("Worktree not found: {}", worktree_id)));
        };
        Ok(worktree)
    }

    pub async fn get_worktree_status(
        &self,
        repo_path: &Path,
        worktree_id: &str,
    ) -> AppResult<WorktreeStatus> {
        Ok(self.get_worktree(repo_path, worktree_id).await?.status)
    }

    pub async fn remove_worktree(
        &self,
        repo_path: &Path,
        worktree_id: &str,
        force: bool,
    ) -> AppResult<()> {
        let repo_root = self.canonical_repo_root(repo_path)?;
        let worktree = self.get_worktree(&repo_root, worktree_id).await?;
        let wt_path = PathBuf::from(&worktree.path);
        self.git.remove_worktree(&repo_root, &wt_path, force)?;
        if self.git.branch_exists(&repo_root, &worktree.branch)? {
            self.git.delete_branch(&repo_root, &worktree.branch, force)?;
        }
        if worktree.runtime_kind == WorktreeRuntimeKind::Managed {
            let metadata_path = self.metadata_path(&wt_path);
            if metadata_path.exists() {
                let _ = fs::remove_file(&metadata_path);
            }
            let _ = fs::remove_dir_all(&wt_path);
        }
        {
            let mut cache = self.worktrees.write().await;
            cache.remove(worktree_id);
        }
        self.git.prune_worktrees(&repo_root)?;
        Ok(())
    }

    pub async fn cleanup_managed_worktree(
        &self,
        repo_path: &Path,
        worktree_id: &str,
        force: bool,
    ) -> AppResult<()> {
        let repo_root = self.canonical_repo_root(repo_path)?;
        let worktree = self.get_worktree(&repo_root, worktree_id).await?;
        if worktree.runtime_kind != WorktreeRuntimeKind::Managed {
            return Err(AppError::validation(format!(
                "Worktree '{}' is not managed by Plan Cascade",
                worktree_id
            )));
        }
        self.remove_worktree(&repo_root, worktree_id, force).await
    }

    pub async fn complete_worktree(
        &self,
        repo_path: &Path,
        worktree_id: &str,
        commit_message: Option<&str>,
    ) -> AppResult<CompleteWorktreeResult> {
        let repo_root = self.canonical_repo_root(repo_path)?;
        let worktree = self.get_worktree(&repo_root, worktree_id).await?;
        let wt_path = PathBuf::from(&worktree.path);
        if !worktree.status.can_complete() {
            return Ok(CompleteWorktreeResult::error(format!(
                "Worktree cannot be completed in status: {}",
                worktree.status
            )));
        }

        let status = self.git.status(&wt_path)?;
        let all_files: Vec<String> = status
            .staged
            .iter()
            .chain(status.modified.iter())
            .chain(status.untracked.iter())
            .cloned()
            .collect();
        let committable_files = self.config_service.get_committable_files(&wt_path, &all_files);
        let mut result = CompleteWorktreeResult::success(None, false, false);

        if !committable_files.is_empty() {
            self.git.reset_staging(&wt_path).ok();
            let file_refs: Vec<&str> = committable_files.iter().map(String::as_str).collect();
            self.git.add(&wt_path, &file_refs)?;
            let owned_message;
            let message = if let Some(message) = commit_message {
                message
            } else {
                owned_message = format!("Complete worktree task: {}", worktree.name);
                &owned_message
            };
            match self.git.commit(&wt_path, message) {
                Ok(sha) => {
                    result.commit_sha = Some(sha);
                }
                Err(error) => {
                    return Ok(CompleteWorktreeResult::error(format!(
                        "Failed to commit: {}",
                        error
                    )));
                }
            }
        } else {
            result = result.with_warning("No code changes to commit".to_string());
        }

        self.git.checkout(&repo_root, &worktree.target_branch)?;
        let merge_message = format!("Merge {} into {}", worktree.branch, worktree.target_branch);
        match self
            .git
            .merge(&repo_root, &worktree.branch, Some(&merge_message))?
        {
            MergeResult::Success => {
                result.merged = true;
            }
            MergeResult::Conflict(conflicts) => {
                self.git.merge_abort(&repo_root).ok();
                let conflict_info = MergeConflict::new(
                    conflicts,
                    worktree.branch.clone(),
                    worktree.target_branch.clone(),
                );
                return Ok(CompleteWorktreeResult::error(format!(
                    "Merge conflict detected in {} files. Please resolve manually.",
                    conflict_info.conflicting_files.len()
                ))
                .with_warning(format!(
                    "Conflicting files: {:?}",
                    conflict_info.conflicting_files
                )));
            }
            MergeResult::Error(message) => {
                return Ok(CompleteWorktreeResult::error(format!(
                    "Merge failed: {}",
                    message
                )));
            }
        }

        if result.merged {
            match self.remove_worktree(&repo_root, worktree_id, true).await {
                Ok(()) => result.cleaned_up = true,
                Err(error) => {
                    result = result.with_warning(format!("Failed to cleanup worktree: {}", error));
                }
            }
        }
        Ok(result)
    }

    pub async fn prepare_pull_request(
        &self,
        repo_path: &Path,
        worktree_id: &str,
    ) -> AppResult<PreparePullRequestResult> {
        let repo_root = self.canonical_repo_root(repo_path)?;
        let worktree = self.get_worktree(&repo_root, worktree_id).await?;
        let (remote_name, remote_url) = self.primary_remote(&repo_root)?;
        let parsed = self.parse_forge_remote(&remote_url)?;
        Ok(PreparePullRequestResult {
            worktree_id: worktree.id.clone(),
            repo_id: worktree.repo_id.clone().unwrap_or_else(|| self.hash_input(&repo_root.to_string_lossy())),
            forge_provider: parsed.provider,
            remote_name,
            remote_url,
            head_branch: worktree.branch.clone(),
            base_branch: worktree.target_branch.clone(),
            compare_url: self.compare_url(&parsed, &worktree.target_branch, &worktree.branch),
            create_url: self.create_url(&parsed, &worktree.target_branch, &worktree.branch),
        })
    }

    pub async fn create_pull_request(
        &self,
        repo_path: &Path,
        request: &CreatePullRequestRequest,
        token: &str,
    ) -> AppResult<CreatePullRequestResult> {
        let repo_root = self.canonical_repo_root(repo_path)?;
        let worktree = self.get_worktree(&repo_root, &request.worktree_id).await?;
        let (remote_name, remote_url) = self.primary_remote(&repo_root)?;
        let parsed = self.parse_forge_remote(&remote_url)?;
        if parsed.provider != request.provider {
            return Err(AppError::validation(format!(
                "Remote provider mismatch: remote uses {:?}, request asked for {:?}",
                parsed.provider, request.provider
            )));
        }

        let base_branch = request
            .base_branch
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&worktree.target_branch)
            .to_string();
        let client = reqwest::Client::builder()
            .user_agent("plan-cascade-desktop")
            .build()
            .map_err(|error| AppError::command(format!("Failed to build HTTP client: {}", error)))?;

        let (url, number, state) = match parsed.provider {
            ForgeProvider::Github => {
                let endpoint = format!(
                    "{}/api/v3/repos/{}/{}/pulls",
                    parsed.origin,
                    parsed.owner,
                    parsed.repo
                );
                let response = client
                    .post(endpoint)
                    .bearer_auth(token)
                    .header("Accept", "application/vnd.github+json")
                    .json(&serde_json::json!({
                        "title": request.title,
                        "body": request.body,
                        "head": worktree.branch,
                        "base": base_branch,
                        "draft": request.draft,
                    }))
                    .send()
                    .await
                    .map_err(|error| AppError::command(format!("Failed to create GitHub pull request: {}", error)))?;
                let status = response.status();
                let body: serde_json::Value = response.json().await.unwrap_or_default();
                if !status.is_success() {
                    return Err(AppError::command(format!(
                        "GitHub pull request creation failed: {}",
                        body
                    )));
                }
                (
                    body.get("html_url").and_then(|value| value.as_str()).unwrap_or_default().to_string(),
                    body.get("number").and_then(|value| value.as_u64()),
                    if body.get("draft").and_then(|value| value.as_bool()).unwrap_or(false) {
                        PullRequestState::Draft
                    } else {
                        PullRequestState::Open
                    },
                )
            }
            ForgeProvider::Gitlab => {
                let project_path = format!("{}/{}", parsed.owner, parsed.repo);
                let project = urlencoding::encode(&project_path).into_owned();
                let endpoint = format!("{}/api/v4/projects/{}/merge_requests", parsed.origin, project);
                let response = client
                    .post(endpoint)
                    .bearer_auth(token)
                    .json(&serde_json::json!({
                        "title": request.title,
                        "description": request.body,
                        "source_branch": worktree.branch,
                        "target_branch": base_branch,
                        "remove_source_branch": false,
                        "draft": request.draft,
                    }))
                    .send()
                    .await
                    .map_err(|error| AppError::command(format!("Failed to create GitLab merge request: {}", error)))?;
                let status = response.status();
                let body: serde_json::Value = response.json().await.unwrap_or_default();
                if !status.is_success() {
                    return Err(AppError::command(format!(
                        "GitLab merge request creation failed: {}",
                        body
                    )));
                }
                (
                    body.get("web_url").and_then(|value| value.as_str()).unwrap_or_default().to_string(),
                    body.get("iid").and_then(|value| value.as_u64()),
                    PullRequestState::Open,
                )
            }
            ForgeProvider::Gitea => {
                let endpoint = format!(
                    "{}/api/v1/repos/{}/{}/pulls",
                    parsed.origin,
                    parsed.owner,
                    parsed.repo
                );
                let response = client
                    .post(endpoint)
                    .bearer_auth(token)
                    .json(&serde_json::json!({
                        "title": request.title,
                        "body": request.body,
                        "head": worktree.branch,
                        "base": base_branch,
                    }))
                    .send()
                    .await
                    .map_err(|error| AppError::command(format!("Failed to create Gitea pull request: {}", error)))?;
                let status = response.status();
                let body: serde_json::Value = response.json().await.unwrap_or_default();
                if !status.is_success() {
                    return Err(AppError::command(format!(
                        "Gitea pull request creation failed: {}",
                        body
                    )));
                }
                (
                    body.get("html_url").and_then(|value| value.as_str()).unwrap_or_default().to_string(),
                    body.get("number").and_then(|value| value.as_u64()),
                    PullRequestState::Open,
                )
            }
        };

        self.update_worktree_pr_info(
            &repo_root,
            &worktree.id,
            PullRequestInfo {
                provider: Some(parsed.provider),
                remote_name: Some(remote_name),
                base_branch: Some(base_branch),
                head_branch: Some(worktree.branch.clone()),
                title: Some(request.title.clone()),
                body: Some(request.body.clone()),
                url: Some(url.clone()),
                number,
                state: Some(state),
                created_at: Some(chrono::Utc::now().to_rfc3339()),
                updated_at: Some(chrono::Utc::now().to_rfc3339()),
            },
        )
        .await?;

        Ok(CreatePullRequestResult {
            worktree_id: worktree.id,
            provider: parsed.provider,
            url,
            number,
            state,
        })
    }

    pub async fn update_worktree_pr_info(
        &self,
        repo_path: &Path,
        worktree_id: &str,
        pr_info: PullRequestInfo,
    ) -> AppResult<()> {
        let repo_root = self.canonical_repo_root(repo_path)?;
        let worktree = self.get_worktree(&repo_root, worktree_id).await?;
        if worktree.runtime_kind != WorktreeRuntimeKind::Managed {
            return Ok(());
        }
        let wt_path = PathBuf::from(&worktree.path);
        let Some(mut metadata) = self.read_metadata(&wt_path)? else {
            return Ok(());
        };
        metadata.pr_info = Some(pr_info.clone());
        metadata.touch();
        self.persist_metadata(&metadata)?;
        let mut cache = self.worktrees.write().await;
        if let Some(cached) = cache.get_mut(worktree_id) {
            cached.pr_info = Some(pr_info);
            cached.updated_at = chrono::Utc::now().to_rfc3339();
        }
        Ok(())
    }

    async fn find_worktree_by_id(
        &self,
        repo_root: &Path,
        worktree_id: &str,
    ) -> AppResult<Option<Worktree>> {
        let worktrees = self.list_worktrees(repo_root).await?;
        Ok(worktrees.into_iter().find(|worktree| worktree.id == worktree_id))
    }

    async fn cache_worktree(&self, worktree: &Worktree) {
        let mut cache = self.worktrees.write().await;
        cache.insert(worktree.id.clone(), worktree.clone());
    }

    fn canonical_repo_root(&self, repo_path: &Path) -> AppResult<PathBuf> {
        let root = self.git.get_repo_root(repo_path)?;
        let path = PathBuf::from(root);
        path.canonicalize().map_err(AppError::Io)
    }

    fn ensure_target_branch_exists(&self, repo_root: &Path, target_branch: &str) -> AppResult<()> {
        if !self.git.branch_exists(repo_root, target_branch)? {
            return Err(AppError::validation(format!(
                "Target branch does not exist: {}",
                target_branch
            )));
        }
        Ok(())
    }

    fn default_target_branch(&self, repo_root: &Path) -> AppResult<String> {
        self.git.get_current_branch(repo_root)
    }

    fn derive_worktree_id(&self, session_id: &str, task_slug: &str) -> String {
        let short = session_id.chars().take(8).collect::<String>();
        format!("session-{}-{}", short, task_slug)
    }

    fn derive_branch_name(&self, task_slug: &str, session_id: Option<&str>) -> String {
        let short = session_id
            .map(|value| value.chars().take(8).collect::<String>())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| self.hash_input(task_slug).chars().take(8).collect());
        format!("pc/{}-{}", task_slug, short)
    }

    fn validate_branch_name(&self, branch_name: &str) -> AppResult<()> {
        let normalized = branch_name.trim();
        if normalized.is_empty() || normalized != branch_name {
            return Err(AppError::validation("Branch name cannot be empty"));
        }
        if normalized.contains(char::is_whitespace)
            || normalized.contains("..")
            || normalized.starts_with('-')
        {
            return Err(AppError::validation(format!(
                "Invalid branch name: {}",
                branch_name
            )));
        }
        Ok(())
    }

    fn sanitize_task_name(&self, name: &str) -> String {
        name.chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else if c.is_whitespace() {
                    '-'
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .to_lowercase()
    }

    fn hash_input(&self, value: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(value.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn compute_repo_id(&self, repo_root: &Path) -> AppResult<String> {
        let canonical = repo_root
            .canonicalize()
            .map_err(AppError::Io)?
            .to_string_lossy()
            .to_string();
        let remotes = self
            .git
            .execute(repo_root, &["remote", "-v"])?
            .into_result()
            .unwrap_or_default();
        Ok(self.hash_input(&format!("{}::{}", canonical, remotes)))
    }

    fn managed_metadata_to_worktree(
        &self,
        metadata: &ManagedWorktreeMetadata,
        planning_config: Option<crate::models::worktree::PlanningConfig>,
    ) -> Worktree {
        let (created_at, updated_at) = self.get_worktree_timestamps(Path::new(&metadata.runtime_path));
        Worktree {
            id: metadata.worktree_id.clone(),
            name: metadata
                .display_label
                .clone()
                .unwrap_or_else(|| metadata.worktree_id.clone()),
            path: metadata.runtime_path.clone(),
            branch: metadata.branch.clone(),
            target_branch: metadata.target_branch.clone(),
            status: WorktreeStatus::Active,
            created_at: metadata.created_at.clone().max(created_at),
            updated_at: metadata.updated_at.clone().max(updated_at),
            error: None,
            planning_config,
            repo_id: Some(metadata.repo_id.clone()),
            root_path: Some(metadata.root_path.clone()),
            session_id: Some(metadata.session_id.clone()).filter(|value| !value.is_empty()),
            runtime_kind: WorktreeRuntimeKind::Managed,
            cleanup_policy: metadata.cleanup_policy,
            pr_info: metadata.pr_info.clone(),
            display_label: metadata.display_label.clone(),
        }
    }

    fn metadata_path(&self, worktree_path: &Path) -> PathBuf {
        worktree_path.join(METADATA_FILE)
    }

    fn read_metadata(&self, worktree_path: &Path) -> AppResult<Option<ManagedWorktreeMetadata>> {
        let path = self.metadata_path(worktree_path);
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path)?;
        let metadata = serde_json::from_str::<ManagedWorktreeMetadata>(&content)?;
        Ok(Some(metadata))
    }

    fn persist_metadata(&self, metadata: &ManagedWorktreeMetadata) -> AppResult<()> {
        let path = self.metadata_path(Path::new(&metadata.runtime_path));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_vec_pretty(metadata)?)?;
        Ok(())
    }

    fn is_managed_runtime_path(&self, repo_root: &Path, worktree_path: &Path) -> bool {
        worktree_path.starts_with(self.get_worktree_base(repo_root))
    }

    fn get_worktree_timestamps(&self, path: &Path) -> (String, String) {
        let default_time = chrono::Utc::now().to_rfc3339();
        let created_at = fs::metadata(path)
            .ok()
            .and_then(|metadata| metadata.created().ok())
            .map(|time| {
                let datetime: chrono::DateTime<chrono::Utc> = time.into();
                datetime.to_rfc3339()
            })
            .unwrap_or_else(|| default_time.clone());
        let updated_at = fs::metadata(path)
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .map(|time| {
                let datetime: chrono::DateTime<chrono::Utc> = time.into();
                datetime.to_rfc3339()
            })
            .unwrap_or(default_time);
        (created_at, updated_at)
    }

    fn primary_remote(&self, repo_root: &Path) -> AppResult<(String, String)> {
        let remotes = self
            .git
            .execute(repo_root, &["remote"])?
            .into_result()?
            .lines()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        let remote_name = remotes
            .iter()
            .find(|remote| remote.as_str() == "origin")
            .cloned()
            .or_else(|| remotes.first().cloned())
            .ok_or_else(|| AppError::validation("Repository has no configured remotes".to_string()))?;
        let remote_url = self
            .git
            .execute(repo_root, &["remote", "get-url", &remote_name])?
            .into_result()?
            .trim()
            .to_string();
        Ok((remote_name, remote_url))
    }

    fn parse_forge_remote(&self, remote_url: &str) -> AppResult<ParsedForgeRemote> {
        let normalized = if remote_url.starts_with("git@") {
            let without_prefix = remote_url.trim_start_matches("git@");
            let mut parts = without_prefix.splitn(2, ':');
            let host = parts.next().unwrap_or_default();
            let path = parts.next().unwrap_or_default().trim_end_matches(".git");
            format!("https://{}/{}", host, path)
        } else {
            remote_url.trim_end_matches(".git").to_string()
        };
        let url = Url::parse(&normalized)
            .map_err(|error| AppError::parse(format!("Invalid remote URL '{}': {}", remote_url, error)))?;
        let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
        let path = url.path().trim_start_matches('/').trim_end_matches(".git");
        let mut segments = path.split('/').filter(|value| !value.is_empty()).collect::<Vec<_>>();
        if segments.len() < 2 {
            return Err(AppError::parse(format!(
                "Unsupported remote URL '{}'",
                remote_url
            )));
        }
        let repo = segments.pop().unwrap_or_default().to_string();
        let owner = segments.join("/");
        let provider = if host.contains("github") {
            ForgeProvider::Github
        } else if host.contains("gitlab") {
            ForgeProvider::Gitlab
        } else {
            ForgeProvider::Gitea
        };
        Ok(ParsedForgeRemote {
            provider,
            origin: format!("{}://{}", url.scheme(), host),
            owner,
            repo,
        })
    }

    fn compare_url(&self, remote: &ParsedForgeRemote, base: &str, head: &str) -> String {
        match remote.provider {
            ForgeProvider::Github => format!(
                "{}/{}/{}/compare/{}...{}?expand=1",
                remote.origin, remote.owner, remote.repo, base, head
            ),
            ForgeProvider::Gitlab => format!(
                "{}/{}/{}/-/compare/{}...{}",
                remote.origin, remote.owner, remote.repo, base, head
            ),
            ForgeProvider::Gitea => format!(
                "{}/{}/{}/compare/{}...{}",
                remote.origin, remote.owner, remote.repo, base, head
            ),
        }
    }

    fn create_url(&self, remote: &ParsedForgeRemote, base: &str, head: &str) -> String {
        match remote.provider {
            ForgeProvider::Github => self.compare_url(remote, base, head),
            ForgeProvider::Gitlab => format!(
                "{}/{}/{}/-/merge_requests/new?merge_request[source_branch]={}&merge_request[target_branch]={}",
                remote.origin,
                remote.owner,
                remote.repo,
                urlencoding::encode(head),
                urlencoding::encode(base)
            ),
            ForgeProvider::Gitea => self.compare_url(remote, base, head),
        }
    }
}

#[derive(Debug)]
struct ParsedForgeRemote {
    provider: ForgeProvider,
    origin: String,
    owner: String,
    repo: String,
}

impl Default for WorktreeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for WorktreeManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorktreeManager").finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_task_name() {
        let manager = WorktreeManager::new();
        assert_eq!(manager.sanitize_task_name("My Feature Task"), "my-feature-task");
        assert_eq!(manager.sanitize_task_name("feature/add-button"), "feature_add-button");
        assert_eq!(
            manager.sanitize_task_name("Task with @special#chars!"),
            "task-with-_special_chars_"
        );
    }

    #[test]
    fn test_get_worktree_base_uses_plan_cascade_root() {
        let manager = WorktreeManager::new();
        let base = manager.get_worktree_base(Path::new("/tmp/repo"));
        assert!(base.to_string_lossy().contains(".plan-cascade/worktrees"));
    }

    #[test]
    fn test_derive_branch_name_uses_pc_prefix() {
        let manager = WorktreeManager::new();
        let branch = manager.derive_branch_name("feature-auth", Some("1234567890"));
        assert!(branch.starts_with("pc/feature-auth-"));
    }

    #[test]
    fn test_validate_branch_name_rejects_invalid_values() {
        let manager = WorktreeManager::new();
        assert!(manager.validate_branch_name("pc/feature-auth-1234").is_ok());
        assert!(manager.validate_branch_name(" feature").is_err());
        assert!(manager.validate_branch_name("feature branch").is_err());
        assert!(manager.validate_branch_name("feature..branch").is_err());
        assert!(manager.validate_branch_name("-feature").is_err());
    }

    #[test]
    fn test_parse_github_remote() {
        let manager = WorktreeManager::new();
        let parsed = manager
            .parse_forge_remote("git@github.com:plan-cascade/plan-cascade.git")
            .unwrap();
        assert_eq!(parsed.provider, ForgeProvider::Github);
        assert_eq!(parsed.owner, "plan-cascade");
        assert_eq!(parsed.repo, "plan-cascade");
    }
}
