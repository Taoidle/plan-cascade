//! Analysis Run Store
//!
//! Persists analysis plans, evidence, and synthesis artifacts to disk so
//! long-running analysis can be resumed and audited.

use crate::utils::error::{AppError, AppResult};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use uuid::Uuid;

use super::analysis_scheduler::PlannedPhase;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CoverageMetrics {
    pub observed_paths: usize,
    pub evidence_records: usize,
    pub successful_phases: usize,
    pub partial_phases: usize,
    pub failed_phases: usize,
    pub inventory_total_files: usize,
    pub inventory_indexed_files: usize,
    pub sampled_read_files: usize,
    pub test_files_total: usize,
    pub test_files_read: usize,
    pub coverage_ratio: f64,
    pub test_coverage_ratio: f64,
    pub chunk_count: usize,
    pub synthesis_rounds: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRecord {
    pub evidence_id: String,
    pub phase_id: String,
    pub sub_agent_id: String,
    pub tool_name: Option<String>,
    pub file_path: Option<String>,
    pub summary: String,
    pub success: bool,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentResultRecord {
    pub sub_agent_id: String,
    pub role: String,
    pub status: String,
    pub summary: Option<String>,
    pub usage: Value,
    pub metrics: Value,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisPhaseResultRecord {
    pub phase_id: String,
    pub title: String,
    pub status: String,
    pub summary_path: Option<String>,
    pub usage: Value,
    pub metrics: Value,
    pub warnings: Vec<String>,
    pub sub_agents: Vec<SubAgentResultRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisRunManifest {
    pub run_id: String,
    pub request: String,
    pub project_root: String,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub planned_phases: Vec<PlannedPhase>,
    pub phase_results: Vec<AnalysisPhaseResultRecord>,
    pub coverage: CoverageMetrics,
    pub observed_paths: Vec<String>,
    pub report_path: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AnalysisRunStore {
    base_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct AnalysisRunHandle {
    run_id: String,
    run_dir: PathBuf,
    manifest_path: PathBuf,
    evidence_log_path: PathBuf,
    manifest: Arc<Mutex<AnalysisRunManifest>>,
}

impl AnalysisRunStore {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    pub fn start_run(&self, request: &str, project_root: &Path) -> AppResult<AnalysisRunHandle> {
        fs::create_dir_all(&self.base_dir)?;

        let run_id = format!(
            "run-{}-{}",
            Utc::now().format("%Y%m%dT%H%M%S"),
            Uuid::new_v4().simple()
        );
        let run_dir = self.base_dir.join(&run_id);
        fs::create_dir_all(run_dir.join("phases"))?;
        fs::create_dir_all(run_dir.join("evidence"))?;
        fs::create_dir_all(run_dir.join("summaries"))?;
        fs::create_dir_all(run_dir.join("final"))?;

        let now = Utc::now().timestamp();
        let manifest = AnalysisRunManifest {
            run_id: run_id.clone(),
            request: request.to_string(),
            project_root: project_root.to_string_lossy().to_string(),
            status: "running".to_string(),
            created_at: now,
            updated_at: now,
            planned_phases: Vec::new(),
            phase_results: Vec::new(),
            coverage: CoverageMetrics::default(),
            observed_paths: Vec::new(),
            report_path: None,
            error: None,
        };

        let manifest_path = run_dir.join("manifest.json");
        let evidence_log_path = run_dir.join("evidence").join("evidence.jsonl");
        fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

        Ok(AnalysisRunHandle {
            run_id,
            run_dir,
            manifest_path,
            evidence_log_path,
            manifest: Arc::new(Mutex::new(manifest)),
        })
    }
}

impl AnalysisRunHandle {
    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    pub fn run_dir(&self) -> &Path {
        &self.run_dir
    }

    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub fn record_phase_plan(&self, phase: PlannedPhase) -> AppResult<()> {
        let mut manifest = self.lock_manifest()?;
        if let Some(existing) = manifest
            .planned_phases
            .iter_mut()
            .find(|item| item.phase_id == phase.phase_id)
        {
            *existing = phase;
        } else {
            manifest.planned_phases.push(phase);
        }
        manifest.updated_at = Utc::now().timestamp();
        self.persist_manifest_locked(&manifest)
    }

    pub fn append_evidence(&self, record: &EvidenceRecord) -> AppResult<()> {
        let serialized = serde_json::to_string(record)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.evidence_log_path)?;
        writeln!(file, "{serialized}")?;

        let mut manifest = self.lock_manifest()?;
        manifest.coverage.evidence_records += 1;
        if let Some(path) = record.file_path.as_ref() {
            if !manifest.observed_paths.iter().any(|p| p == path) {
                manifest.observed_paths.push(path.clone());
            }
            manifest.coverage.observed_paths = manifest.observed_paths.len();
        }
        manifest.updated_at = Utc::now().timestamp();
        self.persist_manifest_locked(&manifest)
    }

    pub fn update_coverage(&self, metrics: CoverageMetrics) -> AppResult<()> {
        let mut manifest = self.lock_manifest()?;
        manifest.coverage = metrics;
        manifest.updated_at = Utc::now().timestamp();
        self.persist_manifest_locked(&manifest)
    }

    pub fn write_phase_summary(&self, phase_id: &str, content: &str) -> AppResult<String> {
        let path = self
            .run_dir
            .join("summaries")
            .join(format!("{phase_id}.md"));
        fs::write(&path, content)?;
        Ok(path.to_string_lossy().to_string())
    }

    pub fn write_json_artifact<T: Serialize>(
        &self,
        relative_path: &str,
        payload: &T,
    ) -> AppResult<String> {
        let target = self.run_dir.join(relative_path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&target, serde_json::to_string_pretty(payload)?)?;
        Ok(target.to_string_lossy().to_string())
    }

    pub fn record_phase_result(&self, result: AnalysisPhaseResultRecord) -> AppResult<()> {
        let mut manifest = self.lock_manifest()?;
        if let Some(existing) = manifest
            .phase_results
            .iter_mut()
            .find(|item| item.phase_id == result.phase_id)
        {
            *existing = result;
        } else {
            manifest.phase_results.push(result);
        }
        manifest.updated_at = Utc::now().timestamp();
        self.persist_manifest_locked(&manifest)
    }

    pub fn write_final_report(&self, report: &str) -> AppResult<String> {
        let report_path = self.run_dir.join("final").join("report.md");
        fs::write(&report_path, report)?;

        let mut manifest = self.lock_manifest()?;
        manifest.report_path = Some(report_path.to_string_lossy().to_string());
        manifest.updated_at = Utc::now().timestamp();
        self.persist_manifest_locked(&manifest)?;
        Ok(report_path.to_string_lossy().to_string())
    }

    pub fn complete(&self, success: bool, error: Option<String>) -> AppResult<()> {
        let mut manifest = self.lock_manifest()?;
        manifest.status = if success {
            "completed".to_string()
        } else {
            "failed".to_string()
        };
        manifest.error = error;
        manifest.updated_at = Utc::now().timestamp();
        self.persist_manifest_locked(&manifest)
    }

    fn lock_manifest(&self) -> AppResult<MutexGuard<'_, AnalysisRunManifest>> {
        self.manifest
            .lock()
            .map_err(|_| AppError::internal("Analysis run manifest mutex poisoned"))
    }

    fn persist_manifest_locked(&self, manifest: &AnalysisRunManifest) -> AppResult<()> {
        fs::write(&self.manifest_path, serde_json::to_string_pretty(manifest)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::orchestrator::analysis_scheduler::build_phase_plan;
    use tempfile::tempdir;

    #[test]
    fn start_run_creates_manifest_and_directories() {
        let dir = tempdir().expect("temp dir");
        let store = AnalysisRunStore::new(dir.path().join("analysis-runs"));
        let handle = store
            .start_run("analyze project", dir.path())
            .expect("start run");

        assert!(handle.manifest_path().exists());
        assert!(handle.run_dir().join("evidence").exists());
        assert!(handle.run_dir().join("summaries").exists());
        assert!(handle.run_dir().join("final").exists());
    }

    #[test]
    fn records_phase_plan_and_completion() {
        let dir = tempdir().expect("temp dir");
        let store = AnalysisRunStore::new(dir.path().join("analysis-runs"));
        let handle = store
            .start_run("analyze project", dir.path())
            .expect("start run");

        let phase = build_phase_plan(
            "structure_discovery",
            "Structure Discovery",
            "Map layout",
            &["Layer 1: inventory"],
            "analyze project",
            "focus src",
            "none",
        );
        handle.record_phase_plan(phase).expect("record plan");
        handle.complete(true, None).expect("complete");

        let manifest_json = fs::read_to_string(handle.manifest_path()).expect("manifest readable");
        let manifest: AnalysisRunManifest =
            serde_json::from_str(&manifest_json).expect("manifest parse");
        assert_eq!(manifest.status, "completed");
        assert_eq!(manifest.planned_phases.len(), 1);
    }
}
