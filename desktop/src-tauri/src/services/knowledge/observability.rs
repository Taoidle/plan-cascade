//! Knowledge Observability Metrics
//!
//! Lightweight persisted counters and rates for post-release monitoring of
//! knowledge retrieval behavior.

use serde::{Deserialize, Serialize};

use crate::storage::database::Database;
use crate::utils::error::AppResult;

const METRIC_PREFIX: &str = "kb.metrics.";
const QUERY_SCOPE_CHECKS_KEY: &str = "query_run_scope_checks_total";
const QUERY_SCOPE_HITS_KEY: &str = "query_run_scope_hits_total";
const INGEST_CROSSTALK_ALERT_KEY: &str = "ingest_crosstalk_alert_total";
const PICKER_SEARCH_TOTAL_KEY: &str = "picker_search_total";
const PICKER_SEARCH_EMPTY_KEY: &str = "picker_search_empty_total";
const PLAN_KNOWLEDGE_ATTEMPT_KEY: &str = "plan_knowledge_attempt_total";
const PLAN_KNOWLEDGE_HIT_KEY: &str = "plan_knowledge_hit_total";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeObservabilityMetrics {
    pub query_run_scope_checks_total: i64,
    pub query_run_scope_hits_total: i64,
    pub query_run_scope_hit_rate: f64,
    pub ingest_crosstalk_alert_total: i64,
    pub picker_search_total: i64,
    pub picker_search_empty_total: i64,
    pub picker_search_empty_rate: f64,
    pub plan_knowledge_attempt_total: i64,
    pub plan_knowledge_hit_total: i64,
    pub plan_knowledge_hit_rate: f64,
}

pub fn record_query_run_scope_check(db: &Database, checks: i64, hits: i64) -> AppResult<()> {
    if checks > 0 {
        increment_counter(db, QUERY_SCOPE_CHECKS_KEY, checks)?;
    }
    if hits > 0 {
        increment_counter(db, QUERY_SCOPE_HITS_KEY, hits)?;
    }
    Ok(())
}

pub fn record_ingest_crosstalk_alert(db: &Database) -> AppResult<()> {
    increment_counter(db, INGEST_CROSSTALK_ALERT_KEY, 1)
}

pub fn record_picker_search(db: &Database, empty: bool) -> AppResult<()> {
    increment_counter(db, PICKER_SEARCH_TOTAL_KEY, 1)?;
    if empty {
        increment_counter(db, PICKER_SEARCH_EMPTY_KEY, 1)?;
    }
    Ok(())
}

pub fn record_plan_knowledge(db: &Database, hit: bool) -> AppResult<()> {
    increment_counter(db, PLAN_KNOWLEDGE_ATTEMPT_KEY, 1)?;
    if hit {
        increment_counter(db, PLAN_KNOWLEDGE_HIT_KEY, 1)?;
    }
    Ok(())
}

pub fn read_metrics_snapshot(db: &Database) -> AppResult<KnowledgeObservabilityMetrics> {
    let query_run_scope_checks_total = read_counter(db, QUERY_SCOPE_CHECKS_KEY)?;
    let query_run_scope_hits_total = read_counter(db, QUERY_SCOPE_HITS_KEY)?;
    let ingest_crosstalk_alert_total = read_counter(db, INGEST_CROSSTALK_ALERT_KEY)?;
    let picker_search_total = read_counter(db, PICKER_SEARCH_TOTAL_KEY)?;
    let picker_search_empty_total = read_counter(db, PICKER_SEARCH_EMPTY_KEY)?;
    let plan_knowledge_attempt_total = read_counter(db, PLAN_KNOWLEDGE_ATTEMPT_KEY)?;
    let plan_knowledge_hit_total = read_counter(db, PLAN_KNOWLEDGE_HIT_KEY)?;

    Ok(KnowledgeObservabilityMetrics {
        query_run_scope_checks_total,
        query_run_scope_hits_total,
        query_run_scope_hit_rate: ratio(query_run_scope_hits_total, query_run_scope_checks_total),
        ingest_crosstalk_alert_total,
        picker_search_total,
        picker_search_empty_total,
        picker_search_empty_rate: ratio(picker_search_empty_total, picker_search_total),
        plan_knowledge_attempt_total,
        plan_knowledge_hit_total,
        plan_knowledge_hit_rate: ratio(plan_knowledge_hit_total, plan_knowledge_attempt_total),
    })
}

fn ratio(num: i64, denom: i64) -> f64 {
    if denom <= 0 {
        return 0.0;
    }
    num as f64 / denom as f64
}

fn metric_key(counter_key: &str) -> String {
    format!("{METRIC_PREFIX}{counter_key}")
}

fn read_counter(db: &Database, counter_key: &str) -> AppResult<i64> {
    let key = metric_key(counter_key);
    let value = db.get_setting(&key)?;
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
    let key = metric_key(counter_key);
    let current = read_counter(db, counter_key)?;
    let next = current.saturating_add(delta);
    db.set_setting(&key, &next.to_string())
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
    fn snapshot_reports_expected_rates() {
        let db = create_test_db();

        record_query_run_scope_check(&db, 3, 2).expect("record query scope");
        record_ingest_crosstalk_alert(&db).expect("record ingest crosstalk");
        record_picker_search(&db, false).expect("record picker non-empty");
        record_picker_search(&db, true).expect("record picker empty");
        record_plan_knowledge(&db, true).expect("record plan hit");
        record_plan_knowledge(&db, false).expect("record plan miss");

        let metrics = read_metrics_snapshot(&db).expect("metrics snapshot");
        assert_eq!(metrics.query_run_scope_checks_total, 3);
        assert_eq!(metrics.query_run_scope_hits_total, 2);
        assert!((metrics.query_run_scope_hit_rate - 2.0 / 3.0).abs() < f64::EPSILON);
        assert_eq!(metrics.ingest_crosstalk_alert_total, 1);
        assert_eq!(metrics.picker_search_total, 2);
        assert_eq!(metrics.picker_search_empty_total, 1);
        assert!((metrics.picker_search_empty_rate - 0.5).abs() < f64::EPSILON);
        assert_eq!(metrics.plan_knowledge_attempt_total, 2);
        assert_eq!(metrics.plan_knowledge_hit_total, 1);
        assert!((metrics.plan_knowledge_hit_rate - 0.5).abs() < f64::EPSILON);
    }
}
