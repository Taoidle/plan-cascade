//! Workflow Observability Metrics
//!
//! Lightweight persisted counters for Simple Plan/Task reliability tracking.

use serde::{Deserialize, Serialize};

use crate::storage::database::Database;
use crate::utils::error::AppResult;

const METRIC_PREFIX: &str = "workflow.metrics.";
const LINK_REHYDRATE_TOTAL_KEY: &str = "workflow_link_rehydrate_total";
const LINK_REHYDRATE_SUCCESS_KEY: &str = "workflow_link_rehydrate_success";
const LINK_REHYDRATE_FAILURE_KEY: &str = "workflow_link_rehydrate_failure";
const INTERACTIVE_ACTION_FAIL_TOTAL_KEY: &str = "interactive_action_fail_total";
const INTERACTIVE_ACTION_FAIL_INDEX_KEY: &str = "interactive_action_fail_index";
const PRD_FEEDBACK_APPLY_TOTAL_KEY: &str = "prd_feedback_apply_total";
const PRD_FEEDBACK_APPLY_SUCCESS_KEY: &str = "prd_feedback_apply_success";
const PRD_FEEDBACK_APPLY_FAILURE_KEY: &str = "prd_feedback_apply_failure";
const LATEST_FAILURE_KEY: &str = "latest_failure_summary";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowObservabilityMetrics {
    pub workflow_link_rehydrate_total: i64,
    pub workflow_link_rehydrate_success: i64,
    pub workflow_link_rehydrate_failure: i64,
    pub interactive_action_fail_total: i64,
    pub prd_feedback_apply_total: i64,
    pub prd_feedback_apply_success: i64,
    pub prd_feedback_apply_failure: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InteractiveActionFailureLabel {
    pub card: String,
    pub action: String,
    pub error_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InteractiveActionFailureMetric {
    pub card: String,
    pub action: String,
    pub error_code: String,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowFailureSummary {
    pub timestamp: String,
    pub action: String,
    pub card: Option<String>,
    pub mode: Option<String>,
    pub kernel_session_id: Option<String>,
    pub mode_session_id: Option<String>,
    pub phase_before: Option<String>,
    pub phase_after: Option<String>,
    pub error_code: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowObservabilitySnapshot {
    pub metrics: WorkflowObservabilityMetrics,
    pub interactive_action_fail_breakdown: Vec<InteractiveActionFailureMetric>,
    pub latest_failure: Option<WorkflowFailureSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowFailureRecordInput {
    pub action: String,
    pub card: Option<String>,
    pub mode: Option<String>,
    pub kernel_session_id: Option<String>,
    pub mode_session_id: Option<String>,
    pub phase_before: Option<String>,
    pub phase_after: Option<String>,
    pub error_code: Option<String>,
    pub message: Option<String>,
    pub timestamp: Option<String>,
}

pub fn record_link_rehydrate(
    db: &Database,
    record: &WorkflowFailureRecordInput,
    success: bool,
) -> AppResult<()> {
    increment_counter(db, LINK_REHYDRATE_TOTAL_KEY, 1)?;
    if success {
        increment_counter(db, LINK_REHYDRATE_SUCCESS_KEY, 1)
    } else {
        increment_counter(db, LINK_REHYDRATE_FAILURE_KEY, 1)?;
        write_latest_failure(db, record)
    }
}

pub fn record_prd_feedback_apply(
    db: &Database,
    record: &WorkflowFailureRecordInput,
    success: bool,
) -> AppResult<()> {
    increment_counter(db, PRD_FEEDBACK_APPLY_TOTAL_KEY, 1)?;
    if success {
        increment_counter(db, PRD_FEEDBACK_APPLY_SUCCESS_KEY, 1)
    } else {
        increment_counter(db, PRD_FEEDBACK_APPLY_FAILURE_KEY, 1)?;
        write_latest_failure(db, record)
    }
}

pub fn record_interactive_action_failure(
    db: &Database,
    record: &WorkflowFailureRecordInput,
) -> AppResult<()> {
    increment_counter(db, INTERACTIVE_ACTION_FAIL_TOTAL_KEY, 1)?;

    let card = normalize_label_value(record.card.as_deref(), "unknown_card");
    let action = normalize_label_value(Some(record.action.as_str()), "unknown_action");
    let error_code = normalize_label_value(record.error_code.as_deref(), "unknown_error");
    let suffix = interactive_failure_suffix(&card, &action, &error_code);
    increment_counter(
        db,
        &format!("{}.{}", INTERACTIVE_ACTION_FAIL_TOTAL_KEY, suffix),
        1,
    )?;
    upsert_interactive_action_index(db, &InteractiveActionFailureLabel {
        card,
        action,
        error_code,
    })?;
    write_latest_failure(db, record)
}

pub fn read_metrics_snapshot(db: &Database) -> AppResult<WorkflowObservabilitySnapshot> {
    let metrics = WorkflowObservabilityMetrics {
        workflow_link_rehydrate_total: read_counter(db, LINK_REHYDRATE_TOTAL_KEY)?,
        workflow_link_rehydrate_success: read_counter(db, LINK_REHYDRATE_SUCCESS_KEY)?,
        workflow_link_rehydrate_failure: read_counter(db, LINK_REHYDRATE_FAILURE_KEY)?,
        interactive_action_fail_total: read_counter(db, INTERACTIVE_ACTION_FAIL_TOTAL_KEY)?,
        prd_feedback_apply_total: read_counter(db, PRD_FEEDBACK_APPLY_TOTAL_KEY)?,
        prd_feedback_apply_success: read_counter(db, PRD_FEEDBACK_APPLY_SUCCESS_KEY)?,
        prd_feedback_apply_failure: read_counter(db, PRD_FEEDBACK_APPLY_FAILURE_KEY)?,
    };

    let index = read_interactive_action_index(db)?;
    let mut interactive_action_fail_breakdown = Vec::new();
    for label in index {
        let suffix = interactive_failure_suffix(&label.card, &label.action, &label.error_code);
        let total = read_counter(
            db,
            &format!("{}.{}", INTERACTIVE_ACTION_FAIL_TOTAL_KEY, suffix),
        )?;
        interactive_action_fail_breakdown.push(InteractiveActionFailureMetric {
            card: label.card,
            action: label.action,
            error_code: label.error_code,
            total,
        });
    }
    interactive_action_fail_breakdown.sort_by(|a, b| b.total.cmp(&a.total));

    let latest_failure = read_latest_failure(db)?;
    Ok(WorkflowObservabilitySnapshot {
        metrics,
        interactive_action_fail_breakdown,
        latest_failure,
    })
}

fn read_latest_failure(db: &Database) -> AppResult<Option<WorkflowFailureSummary>> {
    let value = db.get_setting(&metric_key(LATEST_FAILURE_KEY))?;
    let parsed = value
        .as_deref()
        .and_then(|raw| serde_json::from_str::<WorkflowFailureSummary>(raw).ok());
    Ok(parsed)
}

fn write_latest_failure(db: &Database, record: &WorkflowFailureRecordInput) -> AppResult<()> {
    let summary = WorkflowFailureSummary {
        timestamp: record
            .timestamp
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
        action: if record.action.trim().is_empty() {
            "unknown_action".to_string()
        } else {
            record.action.trim().to_string()
        },
        card: normalize_optional_text(record.card.as_deref()),
        mode: normalize_optional_text(record.mode.as_deref()),
        kernel_session_id: normalize_optional_text(record.kernel_session_id.as_deref()),
        mode_session_id: normalize_optional_text(record.mode_session_id.as_deref()),
        phase_before: normalize_optional_text(record.phase_before.as_deref()),
        phase_after: normalize_optional_text(record.phase_after.as_deref()),
        error_code: normalize_optional_text(record.error_code.as_deref()),
        message: normalize_optional_text(record.message.as_deref()),
    };
    let json = serde_json::to_string(&summary).unwrap_or_else(|_| "{}".to_string());
    db.set_setting(&metric_key(LATEST_FAILURE_KEY), &json)
}

fn normalize_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn read_interactive_action_index(db: &Database) -> AppResult<Vec<InteractiveActionFailureLabel>> {
    let key = metric_key(INTERACTIVE_ACTION_FAIL_INDEX_KEY);
    let value = db.get_setting(&key)?;
    let parsed = value
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Vec<InteractiveActionFailureLabel>>(raw).ok())
        .unwrap_or_default();
    Ok(parsed)
}

fn upsert_interactive_action_index(
    db: &Database,
    label: &InteractiveActionFailureLabel,
) -> AppResult<()> {
    let mut labels = read_interactive_action_index(db)?;
    if labels
        .iter()
        .any(|item| item.card == label.card && item.action == label.action && item.error_code == label.error_code)
    {
        return Ok(());
    }
    labels.push(label.clone());
    let json = serde_json::to_string(&labels).unwrap_or_else(|_| "[]".to_string());
    db.set_setting(&metric_key(INTERACTIVE_ACTION_FAIL_INDEX_KEY), &json)
}

fn normalize_label_value(value: Option<&str>, fallback: &str) -> String {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| fallback.to_string())
}

fn sanitize_label_segment(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            output.push(ch.to_ascii_lowercase());
        } else {
            output.push('_');
        }
    }
    let normalized = output.trim_matches('_').to_string();
    if normalized.is_empty() {
        "unknown".to_string()
    } else {
        normalized
    }
}

fn interactive_failure_suffix(card: &str, action: &str, error_code: &str) -> String {
    format!(
        "{}.{}.{}",
        sanitize_label_segment(card),
        sanitize_label_segment(action),
        sanitize_label_segment(error_code)
    )
}

fn metric_key(counter_key: &str) -> String {
    format!("{METRIC_PREFIX}{counter_key}")
}

fn read_counter(db: &Database, counter_key: &str) -> AppResult<i64> {
    let value = db.get_setting(&metric_key(counter_key))?;
    Ok(value
        .as_deref()
        .map(str::trim)
        .and_then(|raw| raw.parse::<i64>().ok())
        .unwrap_or(0))
}

fn increment_counter(db: &Database, counter_key: &str, delta: i64) -> AppResult<()> {
    if delta == 0 {
        return Ok(());
    }
    let current = read_counter(db, counter_key)?;
    let next = current.saturating_add(delta);
    db.set_setting(&metric_key(counter_key), &next.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::database::Database;

    fn create_test_db() -> Database {
        let manager = r2d2_sqlite::SqliteConnectionManager::memory();
        let pool = r2d2::Pool::builder()
            .max_size(1)
            .build(manager)
            .expect("pool");
        {
            let conn = pool.get().expect("conn");
            conn.execute(
                "CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL, created_at TEXT DEFAULT CURRENT_TIMESTAMP, updated_at TEXT DEFAULT CURRENT_TIMESTAMP)",
                [],
            )
            .expect("create settings");
        }
        Database::from_pool_for_test(pool)
    }

    #[test]
    fn snapshot_reports_expected_counters_and_latest_failure() {
        let db = create_test_db();
        let base = WorkflowFailureRecordInput {
            action: "workflow_link_mode_session".to_string(),
            card: Some("plan_card".to_string()),
            mode: Some("plan".to_string()),
            kernel_session_id: Some("kernel-1".to_string()),
            mode_session_id: Some("plan-1".to_string()),
            phase_before: Some("reviewing_plan".to_string()),
            phase_after: Some("reviewing_plan".to_string()),
            error_code: Some("mode_session_link_failed".to_string()),
            message: Some("failed".to_string()),
            timestamp: Some("2026-03-05T00:00:00Z".to_string()),
        };

        record_link_rehydrate(&db, &base, false).expect("link failure");
        record_link_rehydrate(&db, &base, true).expect("link success");
        record_prd_feedback_apply(&db, &base, true).expect("prd success");
        record_prd_feedback_apply(&db, &base, false).expect("prd failure");
        record_interactive_action_failure(
            &db,
            &WorkflowFailureRecordInput {
                action: "approve_prd".to_string(),
                card: Some("prd_card".to_string()),
                error_code: Some("backend_error".to_string()),
                ..base.clone()
            },
        )
        .expect("interactive action failure");

        let snapshot = read_metrics_snapshot(&db).expect("snapshot");
        assert_eq!(snapshot.metrics.workflow_link_rehydrate_total, 2);
        assert_eq!(snapshot.metrics.workflow_link_rehydrate_success, 1);
        assert_eq!(snapshot.metrics.workflow_link_rehydrate_failure, 1);
        assert_eq!(snapshot.metrics.prd_feedback_apply_total, 2);
        assert_eq!(snapshot.metrics.prd_feedback_apply_success, 1);
        assert_eq!(snapshot.metrics.prd_feedback_apply_failure, 1);
        assert_eq!(snapshot.metrics.interactive_action_fail_total, 1);
        assert_eq!(snapshot.interactive_action_fail_breakdown.len(), 1);
        assert_eq!(
            snapshot.latest_failure.as_ref().map(|failure| failure.action.clone()),
            Some("approve_prd".to_string())
        );
    }
}
