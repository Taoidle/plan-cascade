//! Analytics Service
//!
//! Core service for managing analytics data with SQLite storage.
//! Provides CRUD operations, schema initialization, and connection pooling.

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use regex::Regex;
use rusqlite::params;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::models::analytics::{
    AggregationPeriod, CostBreakdown, CostStatus, DashboardFilterV2, DashboardSummary, ExportFormat,
    ExportJob, ExportJobStatus, ExportStreamingJobRequest, ModelPricing, ModelUsage, PricingRule,
    PricingRuleStatus, ProjectUsage, RecomputeCostsRequest, RecomputeCostsResult, TimeSeriesPoint,
    UsageFilter, UsageRecord, UsageRecordV2, UsageStats,
};
use crate::utils::error::{AppError, AppResult};

/// Type alias for the analytics connection pool
pub type AnalyticsPool = Pool<SqliteConnectionManager>;

/// Analytics service for managing usage data
pub struct AnalyticsService {
    pool: AnalyticsPool,
}

impl AnalyticsService {
    /// Create a new analytics service with the given connection pool
    pub fn new(pool: AnalyticsPool) -> AppResult<Self> {
        let service = Self { pool };
        service.init_schema()?;
        service.init_default_pricing()?;
        Ok(service)
    }

    /// Create analytics service from an existing database pool
    pub fn from_pool(pool: AnalyticsPool) -> AppResult<Self> {
        let service = Self { pool };
        service.init_schema()?;
        service.init_default_pricing()?;
        Ok(service)
    }

    /// Initialize the analytics database schema
    fn init_schema(&self) -> AppResult<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        // Create usage_records table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS usage_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT,
                project_id TEXT,
                model_name TEXT NOT NULL,
                provider TEXT NOT NULL,
                input_tokens INTEGER NOT NULL DEFAULT 0,
                output_tokens INTEGER NOT NULL DEFAULT 0,
                cost_microdollars INTEGER NOT NULL DEFAULT 0,
                timestamp INTEGER NOT NULL,
                metadata TEXT
            )",
            [],
        )?;

        // Create indexes for query performance
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_usage_records_timestamp ON usage_records(timestamp)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_usage_records_session ON usage_records(session_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_usage_records_project ON usage_records(project_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_usage_records_model ON usage_records(model_name)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_usage_records_provider ON usage_records(provider)",
            [],
        )?;

        // Create model_pricing table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS model_pricing (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                model_name TEXT NOT NULL,
                provider TEXT NOT NULL,
                input_price_per_million INTEGER NOT NULL,
                output_price_per_million INTEGER NOT NULL,
                is_custom INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL,
                UNIQUE(model_name, provider)
            )",
            [],
        )?;

        // Create schema_version table for migrations
        conn.execute(
            "CREATE TABLE IF NOT EXISTS analytics_schema_version (
                version INTEGER PRIMARY KEY,
                applied_at INTEGER NOT NULL
            )",
            [],
        )?;

        // Create v2 pricing rules table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS pricing_rules (
                id TEXT PRIMARY KEY,
                provider TEXT NOT NULL,
                model_pattern TEXT NOT NULL,
                currency TEXT NOT NULL DEFAULT 'USD',
                input_per_million INTEGER NOT NULL DEFAULT 0,
                output_per_million INTEGER NOT NULL DEFAULT 0,
                cache_read_per_million INTEGER NOT NULL DEFAULT 0,
                cache_write_per_million INTEGER NOT NULL DEFAULT 0,
                thinking_per_million INTEGER NOT NULL DEFAULT 0,
                effective_from INTEGER NOT NULL,
                effective_to INTEGER,
                status TEXT NOT NULL DEFAULT 'active',
                note TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_pricing_rules_lookup
             ON pricing_rules(provider, effective_from, effective_to, status)",
            [],
        )?;

        // Create v2 usage event table (dual-write target)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS usage_events (
                event_id TEXT PRIMARY KEY,
                session_id TEXT,
                project_id TEXT,
                provider TEXT NOT NULL,
                model TEXT NOT NULL,
                input_tokens INTEGER NOT NULL DEFAULT 0,
                output_tokens INTEGER NOT NULL DEFAULT 0,
                thinking_tokens INTEGER NOT NULL DEFAULT 0,
                cache_read_tokens INTEGER NOT NULL DEFAULT 0,
                cache_write_tokens INTEGER NOT NULL DEFAULT 0,
                timestamp_utc INTEGER NOT NULL,
                metadata_json TEXT,
                ingest_status TEXT NOT NULL DEFAULT 'ingested'
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_usage_events_time ON usage_events(timestamp_utc)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_usage_events_provider_model ON usage_events(provider, model)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_usage_events_project ON usage_events(project_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_usage_events_session ON usage_events(session_id)",
            [],
        )?;

        // Create v2 usage costs table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS usage_costs (
                event_id TEXT PRIMARY KEY,
                rule_id TEXT,
                cost_total INTEGER NOT NULL DEFAULT 0,
                currency TEXT NOT NULL DEFAULT 'USD',
                cost_status TEXT NOT NULL DEFAULT 'missing',
                cost_breakdown_json TEXT,
                computed_at INTEGER NOT NULL,
                FOREIGN KEY(event_id) REFERENCES usage_events(event_id) ON DELETE CASCADE
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_usage_costs_status ON usage_costs(cost_status)",
            [],
        )?;

        // Daily rollup table for fast dashboard queries
        conn.execute(
            "CREATE TABLE IF NOT EXISTS analytics_rollup_daily (
                date_utc TEXT NOT NULL,
                provider TEXT NOT NULL,
                model TEXT NOT NULL,
                project_id TEXT NOT NULL DEFAULT '',
                request_count INTEGER NOT NULL DEFAULT 0,
                tokens_total INTEGER NOT NULL DEFAULT 0,
                input_tokens_total INTEGER NOT NULL DEFAULT 0,
                output_tokens_total INTEGER NOT NULL DEFAULT 0,
                cost_total INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY(date_utc, provider, model, project_id)
            )",
            [],
        )?;

        // Apply migrations
        self.apply_migrations(&conn)?;

        Ok(())
    }

    /// Apply database migrations
    fn apply_migrations(&self, conn: &rusqlite::Connection) -> AppResult<()> {
        let current_version: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM analytics_schema_version",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Migration 1: Initial schema (already applied above)
        if current_version < 1 {
            conn.execute(
                "INSERT INTO analytics_schema_version (version, applied_at) VALUES (1, ?1)",
                params![chrono::Utc::now().timestamp()],
            )?;
        }

        // Migration 2: Add extended token columns
        if current_version < 2 {
            conn.execute(
                "ALTER TABLE usage_records ADD COLUMN thinking_tokens INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
            conn.execute(
                "ALTER TABLE usage_records ADD COLUMN cache_read_tokens INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
            conn.execute(
                "ALTER TABLE usage_records ADD COLUMN cache_creation_tokens INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
            conn.execute(
                "INSERT INTO analytics_schema_version (version, applied_at) VALUES (2, ?1)",
                params![chrono::Utc::now().timestamp()],
            )?;
        }

        // Migration 3: Analytics v2 tables were introduced.
        // The CREATE TABLE IF NOT EXISTS calls in init_schema already ensure
        // table availability; this row marks migration completion.
        if current_version < 3 {
            conn.execute(
                "INSERT INTO analytics_schema_version (version, applied_at) VALUES (3, ?1)",
                params![chrono::Utc::now().timestamp()],
            )?;
        }

        // Migration 4: enrich rollup table with split input/output tokens.
        if current_version < 4 {
            if !Self::column_exists(conn, "analytics_rollup_daily", "input_tokens_total")? {
                conn.execute(
                    "ALTER TABLE analytics_rollup_daily ADD COLUMN input_tokens_total INTEGER NOT NULL DEFAULT 0",
                    [],
                )?;
            }
            if !Self::column_exists(conn, "analytics_rollup_daily", "output_tokens_total")? {
                conn.execute(
                    "ALTER TABLE analytics_rollup_daily ADD COLUMN output_tokens_total INTEGER NOT NULL DEFAULT 0",
                    [],
                )?;
            }
            conn.execute(
                "INSERT INTO analytics_schema_version (version, applied_at) VALUES (4, ?1)",
                params![chrono::Utc::now().timestamp()],
            )?;
        }

        Ok(())
    }

    fn column_exists(
        conn: &rusqlite::Connection,
        table: &str,
        column: &str,
    ) -> AppResult<bool> {
        let pragma = format!("PRAGMA table_info({})", table);
        let mut stmt = conn.prepare(&pragma)?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let name: String = row.get(1)?;
            if name == column {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Initialize default model pricing
    fn init_default_pricing(&self) -> AppResult<()> {
        let default_pricing = vec![
            // Anthropic models (prices in microdollars per million tokens)
            (
                "claude-3-5-sonnet-20241022",
                "anthropic",
                3_000_000,
                15_000_000,
            ),
            (
                "claude-3-5-sonnet-latest",
                "anthropic",
                3_000_000,
                15_000_000,
            ),
            (
                "claude-3-5-haiku-20241022",
                "anthropic",
                1_000_000,
                5_000_000,
            ),
            (
                "claude-3-opus-20240229",
                "anthropic",
                15_000_000,
                75_000_000,
            ),
            (
                "claude-3-sonnet-20240229",
                "anthropic",
                3_000_000,
                15_000_000,
            ),
            ("claude-3-haiku-20240307", "anthropic", 250_000, 1_250_000),
            (
                "claude-opus-4-20250514",
                "anthropic",
                15_000_000,
                75_000_000,
            ),
            (
                "claude-sonnet-4-20250514",
                "anthropic",
                3_000_000,
                15_000_000,
            ),
            // OpenAI models
            ("gpt-4-turbo", "openai", 10_000_000, 30_000_000),
            ("gpt-4o", "openai", 5_000_000, 15_000_000),
            ("gpt-4o-mini", "openai", 150_000, 600_000),
            ("gpt-4", "openai", 30_000_000, 60_000_000),
            ("gpt-3.5-turbo", "openai", 500_000, 1_500_000),
            // DeepSeek models
            ("deepseek-chat", "deepseek", 140_000, 280_000),
            ("deepseek-coder", "deepseek", 140_000, 280_000),
            // Local/Free models
            ("llama3", "ollama", 0, 0),
            ("codellama", "ollama", 0, 0),
            ("mistral", "ollama", 0, 0),
        ];

        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        for (model, provider, input_price, output_price) in default_pricing {
            conn.execute(
                "INSERT OR IGNORE INTO model_pricing
                 (model_name, provider, input_price_per_million, output_price_per_million, is_custom, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 0, ?5)",
                params![
                    model,
                    provider,
                    input_price,
                    output_price,
                    chrono::Utc::now().timestamp()
                ],
            )?;
        }

        Ok(())
    }

    /// Get a connection from the pool
    pub fn get_connection(&self) -> AppResult<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))
    }

    // ========================================================================
    // Usage Records CRUD
    // ========================================================================

    /// Insert a new usage record
    pub fn insert_usage_record(&self, record: &UsageRecord) -> AppResult<i64> {
        let mut conn = self.get_connection()?;
        let tx = conn.unchecked_transaction()?;

        tx.execute(
            "INSERT INTO usage_records
             (session_id, project_id, model_name, provider, input_tokens, output_tokens,
              thinking_tokens, cache_read_tokens, cache_creation_tokens,
              cost_microdollars, timestamp, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                record.session_id,
                record.project_id,
                record.model_name,
                record.provider,
                record.input_tokens,
                record.output_tokens,
                record.thinking_tokens,
                record.cache_read_tokens,
                record.cache_creation_tokens,
                record.cost_microdollars,
                record.timestamp,
                record.metadata,
            ],
        )?;

        let id = tx.last_insert_rowid();
        Self::dual_write_v2_tx(&tx, record)?;
        tx.commit()?;
        Ok(id)
    }

    /// Insert multiple usage records in a batch
    pub fn insert_usage_records_batch(&self, records: &[UsageRecord]) -> AppResult<Vec<i64>> {
        let mut conn = self.get_connection()?;
        let mut ids = Vec::with_capacity(records.len());

        let tx = conn.unchecked_transaction()?;

        for record in records {
            tx.execute(
                "INSERT INTO usage_records
                 (session_id, project_id, model_name, provider, input_tokens, output_tokens,
                  thinking_tokens, cache_read_tokens, cache_creation_tokens,
                  cost_microdollars, timestamp, metadata)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    record.session_id,
                    record.project_id,
                    record.model_name,
                    record.provider,
                    record.input_tokens,
                    record.output_tokens,
                    record.thinking_tokens,
                    record.cache_read_tokens,
                    record.cache_creation_tokens,
                    record.cost_microdollars,
                    record.timestamp,
                    record.metadata,
                ],
            )?;
            ids.push(tx.last_insert_rowid());
            Self::dual_write_v2_tx(&tx, record)?;
        }

        tx.commit()?;
        Ok(ids)
    }

    /// Get a usage record by ID
    pub fn get_usage_record(&self, id: i64) -> AppResult<Option<UsageRecord>> {
        let conn = self.get_connection()?;

        let result = conn.query_row(
            "SELECT id, session_id, project_id, model_name, provider, input_tokens,
                    output_tokens, thinking_tokens, cache_read_tokens, cache_creation_tokens,
                    cost_microdollars, timestamp, metadata
             FROM usage_records WHERE id = ?1",
            params![id],
            |row| Self::row_to_usage_record(row),
        );

        match result {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::database(e.to_string())),
        }
    }

    /// List usage records with optional filtering
    pub fn list_usage_records(
        &self,
        filter: &UsageFilter,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> AppResult<Vec<UsageRecord>> {
        let conn = self.get_connection()?;

        let mut sql = String::from(
            "SELECT id, session_id, project_id, model_name, provider, input_tokens,
                    output_tokens, thinking_tokens, cache_read_tokens, cache_creation_tokens,
                    cost_microdollars, timestamp, metadata
             FROM usage_records WHERE 1=1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref start) = filter.start_timestamp {
            sql.push_str(" AND timestamp >= ?");
            params_vec.push(Box::new(*start));
        }
        if let Some(ref end) = filter.end_timestamp {
            sql.push_str(" AND timestamp < ?");
            params_vec.push(Box::new(*end));
        }
        if let Some(ref model) = filter.model_name {
            sql.push_str(" AND model_name = ?");
            params_vec.push(Box::new(model.clone()));
        }
        if let Some(ref provider) = filter.provider {
            sql.push_str(" AND provider = ?");
            params_vec.push(Box::new(provider.clone()));
        }
        if let Some(ref session) = filter.session_id {
            sql.push_str(" AND session_id = ?");
            params_vec.push(Box::new(session.clone()));
        }
        if let Some(ref project) = filter.project_id {
            sql.push_str(" AND project_id = ?");
            params_vec.push(Box::new(project.clone()));
        }

        sql.push_str(" ORDER BY timestamp DESC");

        if let Some(lim) = limit {
            sql.push_str(&format!(" LIMIT {}", lim));
        }
        if let Some(off) = offset {
            sql.push_str(&format!(" OFFSET {}", off));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let records = stmt
            .query_map(params_refs.as_slice(), |row| Self::row_to_usage_record(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(records)
    }

    /// Delete a usage record by ID
    pub fn delete_usage_record(&self, id: i64) -> AppResult<bool> {
        let conn = self.get_connection()?;
        let rows = conn.execute("DELETE FROM usage_records WHERE id = ?1", params![id])?;
        Ok(rows > 0)
    }

    /// Delete usage records by filter criteria
    pub fn delete_usage_records(&self, filter: &UsageFilter) -> AppResult<i64> {
        let conn = self.get_connection()?;

        let mut sql = String::from("DELETE FROM usage_records WHERE 1=1");
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref start) = filter.start_timestamp {
            sql.push_str(" AND timestamp >= ?");
            params_vec.push(Box::new(*start));
        }
        if let Some(ref end) = filter.end_timestamp {
            sql.push_str(" AND timestamp < ?");
            params_vec.push(Box::new(*end));
        }
        if let Some(ref model) = filter.model_name {
            sql.push_str(" AND model_name = ?");
            params_vec.push(Box::new(model.clone()));
        }
        if let Some(ref provider) = filter.provider {
            sql.push_str(" AND provider = ?");
            params_vec.push(Box::new(provider.clone()));
        }
        if let Some(ref session) = filter.session_id {
            sql.push_str(" AND session_id = ?");
            params_vec.push(Box::new(session.clone()));
        }
        if let Some(ref project) = filter.project_id {
            sql.push_str(" AND project_id = ?");
            params_vec.push(Box::new(project.clone()));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let rows = conn.execute(&sql, params_refs.as_slice())?;
        Ok(rows as i64)
    }

    // ========================================================================
    // Model Pricing CRUD
    // ========================================================================

    /// Get pricing for a specific model
    pub fn get_model_pricing(
        &self,
        model_name: &str,
        provider: &str,
    ) -> AppResult<Option<ModelPricing>> {
        let conn = self.get_connection()?;

        let result = conn.query_row(
            "SELECT id, model_name, provider, input_price_per_million,
                    output_price_per_million, is_custom, updated_at
             FROM model_pricing WHERE model_name = ?1 AND provider = ?2",
            params![model_name, provider],
            |row| Self::row_to_model_pricing(row),
        );

        match result {
            Ok(pricing) => Ok(Some(pricing)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::database(e.to_string())),
        }
    }

    /// List all model pricing
    pub fn list_model_pricing(&self) -> AppResult<Vec<ModelPricing>> {
        let conn = self.get_connection()?;

        let mut stmt = conn.prepare(
            "SELECT id, model_name, provider, input_price_per_million,
                    output_price_per_million, is_custom, updated_at
             FROM model_pricing ORDER BY provider, model_name",
        )?;

        let pricing = stmt
            .query_map([], |row| Self::row_to_model_pricing(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(pricing)
    }

    /// Update or insert model pricing
    pub fn upsert_model_pricing(&self, pricing: &ModelPricing) -> AppResult<()> {
        let conn = self.get_connection()?;

        conn.execute(
            "INSERT INTO model_pricing
             (model_name, provider, input_price_per_million, output_price_per_million, is_custom, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(model_name, provider) DO UPDATE SET
             input_price_per_million = ?3, output_price_per_million = ?4, is_custom = ?5, updated_at = ?6",
            params![
                pricing.model_name,
                pricing.provider,
                pricing.input_price_per_million,
                pricing.output_price_per_million,
                pricing.is_custom as i32,
                chrono::Utc::now().timestamp(),
            ],
        )?;

        Ok(())
    }

    /// Delete custom model pricing (reset to default)
    pub fn delete_model_pricing(&self, model_name: &str, provider: &str) -> AppResult<bool> {
        let conn = self.get_connection()?;
        let rows = conn.execute(
            "DELETE FROM model_pricing WHERE model_name = ?1 AND provider = ?2 AND is_custom = 1",
            params![model_name, provider],
        )?;
        Ok(rows > 0)
    }

    // ========================================================================
    // Statistics
    // ========================================================================

    /// Get overall usage statistics with optional filtering
    pub fn get_usage_stats(&self, filter: &UsageFilter) -> AppResult<UsageStats> {
        let conn = self.get_connection()?;

        let mut sql = String::from(
            "SELECT COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(cost_microdollars), 0), COUNT(*)
             FROM usage_records WHERE 1=1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref start) = filter.start_timestamp {
            sql.push_str(" AND timestamp >= ?");
            params_vec.push(Box::new(*start));
        }
        if let Some(ref end) = filter.end_timestamp {
            sql.push_str(" AND timestamp < ?");
            params_vec.push(Box::new(*end));
        }
        if let Some(ref model) = filter.model_name {
            sql.push_str(" AND model_name = ?");
            params_vec.push(Box::new(model.clone()));
        }
        if let Some(ref provider) = filter.provider {
            sql.push_str(" AND provider = ?");
            params_vec.push(Box::new(provider.clone()));
        }
        if let Some(ref session) = filter.session_id {
            sql.push_str(" AND session_id = ?");
            params_vec.push(Box::new(session.clone()));
        }
        if let Some(ref project) = filter.project_id {
            sql.push_str(" AND project_id = ?");
            params_vec.push(Box::new(project.clone()));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let stats = conn.query_row(&sql, params_refs.as_slice(), |row| {
            let total_input: i64 = row.get(0)?;
            let total_output: i64 = row.get(1)?;
            let total_cost: i64 = row.get(2)?;
            let count: i64 = row.get(3)?;

            let avg_tokens = if count > 0 {
                (total_input + total_output) as f64 / count as f64
            } else {
                0.0
            };
            let avg_cost = if count > 0 {
                total_cost as f64 / count as f64
            } else {
                0.0
            };

            Ok(UsageStats {
                total_input_tokens: total_input,
                total_output_tokens: total_output,
                total_cost_microdollars: total_cost,
                request_count: count,
                avg_tokens_per_request: avg_tokens,
                avg_cost_per_request: avg_cost,
            })
        })?;

        Ok(stats)
    }

    /// Get the count of usage records with optional filtering
    pub fn count_usage_records(&self, filter: &UsageFilter) -> AppResult<i64> {
        let conn = self.get_connection()?;

        let mut sql = String::from("SELECT COUNT(*) FROM usage_records WHERE 1=1");
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref start) = filter.start_timestamp {
            sql.push_str(" AND timestamp >= ?");
            params_vec.push(Box::new(*start));
        }
        if let Some(ref end) = filter.end_timestamp {
            sql.push_str(" AND timestamp < ?");
            params_vec.push(Box::new(*end));
        }
        if let Some(ref model) = filter.model_name {
            sql.push_str(" AND model_name = ?");
            params_vec.push(Box::new(model.clone()));
        }
        if let Some(ref provider) = filter.provider {
            sql.push_str(" AND provider = ?");
            params_vec.push(Box::new(provider.clone()));
        }
        if let Some(ref session) = filter.session_id {
            sql.push_str(" AND session_id = ?");
            params_vec.push(Box::new(session.clone()));
        }
        if let Some(ref project) = filter.project_id {
            sql.push_str(" AND project_id = ?");
            params_vec.push(Box::new(project.clone()));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let count: i64 = conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0))?;
        Ok(count)
    }

    // ========================================================================
    // Analytics v2 Public APIs
    // ========================================================================

    /// List manual pricing rules ordered by provider/model/effective window.
    pub fn list_pricing_rules(&self) -> AppResult<Vec<PricingRule>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, provider, model_pattern, currency, input_per_million, output_per_million,
                    cache_read_per_million, cache_write_per_million, thinking_per_million,
                    effective_from, effective_to, status, created_at, updated_at, note
             FROM pricing_rules
             ORDER BY provider ASC, model_pattern ASC, effective_from DESC, updated_at DESC",
        )?;

        let rules = stmt
            .query_map([], |row| Self::row_to_pricing_rule(row))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rules)
    }

    /// Upsert one pricing rule with overlap validation.
    pub fn upsert_pricing_rule(&self, rule: &PricingRule) -> AppResult<PricingRule> {
        let conn = self.get_connection()?;
        let mut normalized = rule.clone();

        normalized.provider = normalized.provider.trim().to_string();
        normalized.model_pattern = normalized.model_pattern.trim().to_string();
        normalized.currency = normalized.currency.trim().to_string();

        if normalized.id.trim().is_empty() {
            normalized.id = uuid::Uuid::new_v4().to_string();
        }
        if normalized.provider.is_empty() {
            return Err(AppError::validation("provider must not be empty"));
        }
        if normalized.model_pattern.is_empty() {
            return Err(AppError::validation("model_pattern must not be empty"));
        }
        if normalized.currency.is_empty() {
            normalized.currency = "USD".to_string();
        }
        if let Some(end) = normalized.effective_to {
            if end <= normalized.effective_from {
                return Err(AppError::validation(
                    "effective_to must be greater than effective_from",
                ));
            }
        }
        for price in [
            normalized.input_per_million,
            normalized.output_per_million,
            normalized.cache_read_per_million,
            normalized.cache_write_per_million,
            normalized.thinking_per_million,
        ] {
            if price < 0 {
                return Err(AppError::validation("price values must be non-negative"));
            }
        }

        let mut existing_stmt = conn.prepare("SELECT created_at FROM pricing_rules WHERE id = ?1")?;
        let existing_created = existing_stmt
            .query_row(params![normalized.id], |row| row.get::<_, i64>(0))
            .ok();

        let now = chrono::Utc::now().timestamp();
        normalized.created_at = existing_created.unwrap_or(now);
        normalized.updated_at = now;

        if normalized.status == PricingRuleStatus::Active {
            let max_ts = i64::MAX;
            let effective_to = normalized.effective_to.unwrap_or(max_ts);
            let conflict_count: i64 = conn.query_row(
                "SELECT COUNT(*)
                 FROM pricing_rules
                 WHERE id != ?1
                   AND provider = ?2
                   AND model_pattern = ?3
                   AND status = ?4
                   AND effective_from < ?5
                   AND COALESCE(effective_to, 9223372036854775807) > ?6",
                params![
                    normalized.id,
                    normalized.provider,
                    normalized.model_pattern,
                    PricingRuleStatus::Active.as_str(),
                    effective_to,
                    normalized.effective_from,
                ],
                |row| row.get(0),
            )?;
            if conflict_count > 0 {
                return Err(AppError::validation(
                    "overlapping active pricing rule exists for provider + model pattern",
                ));
            }
        }

        conn.execute(
            "INSERT INTO pricing_rules
             (id, provider, model_pattern, currency, input_per_million, output_per_million,
              cache_read_per_million, cache_write_per_million, thinking_per_million,
              effective_from, effective_to, status, note, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
             ON CONFLICT(id) DO UPDATE SET
                provider = excluded.provider,
                model_pattern = excluded.model_pattern,
                currency = excluded.currency,
                input_per_million = excluded.input_per_million,
                output_per_million = excluded.output_per_million,
                cache_read_per_million = excluded.cache_read_per_million,
                cache_write_per_million = excluded.cache_write_per_million,
                thinking_per_million = excluded.thinking_per_million,
                effective_from = excluded.effective_from,
                effective_to = excluded.effective_to,
                status = excluded.status,
                note = excluded.note,
                updated_at = excluded.updated_at",
            params![
                normalized.id,
                normalized.provider,
                normalized.model_pattern,
                normalized.currency,
                normalized.input_per_million,
                normalized.output_per_million,
                normalized.cache_read_per_million,
                normalized.cache_write_per_million,
                normalized.thinking_per_million,
                normalized.effective_from,
                normalized.effective_to,
                normalized.status.as_str(),
                normalized.note,
                normalized.created_at,
                normalized.updated_at,
            ],
        )?;

        Ok(normalized)
    }

    /// Delete pricing rule by ID.
    pub fn delete_pricing_rule(&self, rule_id: &str) -> AppResult<bool> {
        let conn = self.get_connection()?;
        let rows = conn.execute("DELETE FROM pricing_rules WHERE id = ?1", params![rule_id])?;
        Ok(rows > 0)
    }

    /// Query usage records from v2 event/cost tables.
    pub fn list_usage_records_v2(
        &self,
        filter: &DashboardFilterV2,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> AppResult<Vec<UsageRecordV2>> {
        let conn = self.get_connection()?;
        let mut sql = String::from(
            "SELECT ue.event_id, ue.session_id, ue.project_id, ue.model, ue.provider,
                    ue.input_tokens, ue.output_tokens, ue.thinking_tokens,
                    ue.cache_read_tokens, ue.cache_write_tokens,
                    COALESCE(uc.cost_total, 0) AS cost_total,
                    ue.timestamp_utc, ue.metadata_json, uc.rule_id,
                    COALESCE(uc.currency, 'USD') AS currency,
                    COALESCE(uc.cost_status, 'missing') AS cost_status,
                    uc.cost_breakdown_json
             FROM usage_events ue
             LEFT JOIN usage_costs uc ON uc.event_id = ue.event_id
             WHERE 1=1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        Self::append_v2_filter_clauses(&mut sql, &mut params_vec, filter, "ue", "uc");
        sql.push_str(" ORDER BY ue.timestamp_utc DESC");

        if let Some(lim) = limit {
            sql.push_str(&format!(" LIMIT {}", lim.max(0)));
        }
        if let Some(off) = offset {
            sql.push_str(&format!(" OFFSET {}", off.max(0)));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params_refs.as_slice(), |row| Self::row_to_usage_record_v2(row))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    /// Count usage records from v2 event/cost tables.
    pub fn count_usage_records_v2(&self, filter: &DashboardFilterV2) -> AppResult<i64> {
        let conn = self.get_connection()?;
        let mut sql = String::from(
            "SELECT COUNT(*)
             FROM usage_events ue
             LEFT JOIN usage_costs uc ON uc.event_id = ue.event_id
             WHERE 1=1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        Self::append_v2_filter_clauses(&mut sql, &mut params_vec, filter, "ue", "uc");
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let count = conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0))?;
        Ok(count)
    }

    /// v2 dashboard summary with cost-status filtering and rollup acceleration.
    pub fn get_dashboard_summary_v2(
        &self,
        filter: &DashboardFilterV2,
        period: AggregationPeriod,
    ) -> AppResult<DashboardSummary> {
        let current_stats = self.get_usage_stats_v2(filter)?;
        let previous_filter = Self::calculate_previous_period_filter_v2(filter);
        let previous_stats = self.get_usage_stats_v2(&previous_filter)?;

        let cost_change = DashboardSummary::calculate_change(
            current_stats.total_cost_microdollars as f64,
            previous_stats.total_cost_microdollars as f64,
        );
        let tokens_change = DashboardSummary::calculate_change(
            current_stats.total_tokens() as f64,
            previous_stats.total_tokens() as f64,
        );
        let requests_change = DashboardSummary::calculate_change(
            current_stats.request_count as f64,
            previous_stats.request_count as f64,
        );

        let by_model = self.aggregate_by_model_v2(filter)?;
        let by_project = self.aggregate_by_project_v2(filter)?;
        let time_series = self.get_time_series_v2(filter, period)?;

        Ok(DashboardSummary {
            current_period: current_stats,
            previous_period: previous_stats,
            cost_change_percent: cost_change,
            tokens_change_percent: tokens_change,
            requests_change_percent: requests_change,
            by_model,
            by_project,
            time_series,
        })
    }

    /// Recompute usage costs with current pricing rules for matching records.
    pub fn recompute_costs(&self, request: &RecomputeCostsRequest) -> AppResult<RecomputeCostsResult> {
        let mut conn = self.get_connection()?;
        let tx = conn.unchecked_transaction()?;
        let filter = &request.filter;

        let mut sql = String::from(
            "SELECT ue.event_id, ue.session_id, ue.project_id, ue.provider, ue.model,
                    ue.input_tokens, ue.output_tokens, ue.thinking_tokens,
                    ue.cache_read_tokens, ue.cache_write_tokens, ue.timestamp_utc,
                    COALESCE(uc.cost_total, 0) AS existing_cost,
                    ue.metadata_json
             FROM usage_events ue
             LEFT JOIN usage_costs uc ON uc.event_id = ue.event_id
             WHERE 1=1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        Self::append_v2_filter_clauses(&mut sql, &mut params_vec, filter, "ue", "uc");
        sql.push_str(" ORDER BY ue.timestamp_utc ASC");

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let mut stmt = tx.prepare(&sql)?;
        let event_rows = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, i64>(9)?,
                    row.get::<_, i64>(10)?,
                    row.get::<_, i64>(11)?,
                    row.get::<_, Option<String>>(12)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);

        let mut result = RecomputeCostsResult::default();
        for (
            event_id,
            session_id,
            project_id,
            provider,
            model,
            input_tokens,
            output_tokens,
            thinking_tokens,
            cache_read_tokens,
            cache_write_tokens,
            timestamp_utc,
            existing_cost,
            metadata_json,
        ) in event_rows
        {
            result.scanned_records += 1;
            let synthetic = UsageRecord {
                id: 0,
                session_id,
                project_id,
                model_name: model,
                provider,
                input_tokens,
                output_tokens,
                thinking_tokens,
                cache_read_tokens,
                cache_creation_tokens: cache_write_tokens,
                cost_microdollars: existing_cost,
                timestamp: timestamp_utc,
                metadata: metadata_json,
            };
            let (rule_id, cost_total, cost_status, currency, breakdown_json) =
                Self::resolve_cost_for_record_tx(&tx, &synthetic)?;

            tx.execute(
                "INSERT INTO usage_costs
                 (event_id, rule_id, cost_total, currency, cost_status, cost_breakdown_json, computed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(event_id) DO UPDATE SET
                    rule_id = excluded.rule_id,
                    cost_total = excluded.cost_total,
                    currency = excluded.currency,
                    cost_status = excluded.cost_status,
                    cost_breakdown_json = excluded.cost_breakdown_json,
                    computed_at = excluded.computed_at",
                params![
                    event_id,
                    rule_id,
                    cost_total,
                    currency,
                    cost_status.as_str(),
                    breakdown_json,
                    chrono::Utc::now().timestamp(),
                ],
            )?;

            result.recomputed_records += 1;
            match cost_status {
                CostStatus::Exact => result.exact_records += 1,
                CostStatus::Estimated => result.estimated_records += 1,
                CostStatus::Missing => result.missing_records += 1,
            }
        }

        Self::rebuild_rollup_daily_tx(&tx)?;
        tx.commit()?;
        Ok(result)
    }

    /// Export v2 usage records to a local file in streaming mode.
    pub fn export_usage_streaming_job(
        &self,
        request: &ExportStreamingJobRequest,
    ) -> AppResult<ExportJob> {
        let job_id = uuid::Uuid::new_v4().to_string();
        let extension = match request.format {
            ExportFormat::Csv => "csv",
            ExportFormat::Json => "json",
        };
        let file_path = request
            .file_path
            .clone()
            .filter(|p| !p.trim().is_empty())
            .unwrap_or_else(|| Self::default_export_path(extension));
        let path = PathBuf::from(file_path.clone());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = File::create(&path)?;
        let mut writer = BufWriter::new(file);
        let mut total_exported = 0_i64;
        let chunk_size = 2_000_i64;

        match request.format {
            ExportFormat::Csv => {
                writeln!(
                    writer,
                    "event_id,session_id,project_id,provider,model,input_tokens,output_tokens,thinking_tokens,cache_read_tokens,cache_write_tokens,cost_microdollars,currency,cost_status,timestamp,timestamp_formatted,rule_id,cost_breakdown_json,metadata_json"
                )?;

                let mut offset = 0_i64;
                loop {
                    let batch =
                        self.list_usage_records_v2(&request.filter, Some(chunk_size), Some(offset))?;
                    if batch.is_empty() {
                        break;
                    }

                    for row in &batch {
                        let formatted = chrono::DateTime::from_timestamp(row.timestamp, 0)
                            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                            .unwrap_or_default();
                        writeln!(
                            writer,
                            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},\"{}\",{},{},{}",
                            Self::csv_escape_local(&row.event_id),
                            Self::csv_escape_local(row.session_id.as_deref().unwrap_or("")),
                            Self::csv_escape_local(row.project_id.as_deref().unwrap_or("")),
                            Self::csv_escape_local(&row.provider),
                            Self::csv_escape_local(&row.model_name),
                            row.input_tokens,
                            row.output_tokens,
                            row.thinking_tokens,
                            row.cache_read_tokens,
                            row.cache_creation_tokens,
                            row.cost_microdollars,
                            Self::csv_escape_local(&row.currency),
                            row.cost_status.as_str(),
                            row.timestamp,
                            formatted,
                            Self::csv_escape_local(row.pricing_rule_id.as_deref().unwrap_or("")),
                            Self::csv_escape_local(row.cost_breakdown_json.as_deref().unwrap_or("")),
                            Self::csv_escape_local(row.metadata.as_deref().unwrap_or("")),
                        )?;
                    }

                    total_exported += batch.len() as i64;
                    offset += chunk_size;
                    if batch.len() < chunk_size as usize {
                        break;
                    }
                }

                if request.include_summary {
                    let summary = self.get_usage_stats_v2(&request.filter)?;
                    writeln!(writer, "\n# summary")?;
                    writeln!(writer, "# request_count={}", summary.request_count)?;
                    writeln!(writer, "# total_input_tokens={}", summary.total_input_tokens)?;
                    writeln!(writer, "# total_output_tokens={}", summary.total_output_tokens)?;
                    writeln!(
                        writer,
                        "# total_cost_microdollars={}",
                        summary.total_cost_microdollars
                    )?;
                }
            }
            ExportFormat::Json => {
                writeln!(writer, "{{")?;
                writeln!(
                    writer,
                    "  \"exported_at\": \"{}\",",
                    chrono::Utc::now().to_rfc3339()
                )?;
                if request.include_summary {
                    let summary = self.get_usage_stats_v2(&request.filter)?;
                    let summary_json = serde_json::to_string_pretty(&summary)?;
                    writeln!(writer, "  \"summary\": {},", summary_json)?;
                }
                writeln!(writer, "  \"records\": [")?;

                let mut first = true;
                let mut offset = 0_i64;
                loop {
                    let batch =
                        self.list_usage_records_v2(&request.filter, Some(chunk_size), Some(offset))?;
                    if batch.is_empty() {
                        break;
                    }
                    for row in &batch {
                        if !first {
                            writeln!(writer, ",")?;
                        }
                        first = false;
                        let line = serde_json::to_string(row)?;
                        write!(writer, "    {}", line)?;
                    }
                    total_exported += batch.len() as i64;
                    offset += chunk_size;
                    if batch.len() < chunk_size as usize {
                        break;
                    }
                }
                writeln!(writer)?;
                writeln!(writer, "  ]")?;
                writeln!(writer, "}}")?;
            }
        }

        writer.flush()?;
        Ok(ExportJob {
            id: job_id,
            status: ExportJobStatus::Completed,
            file_path: Some(path.to_string_lossy().to_string()),
            record_count: total_exported,
            error: None,
        })
    }

    fn get_usage_stats_v2(&self, filter: &DashboardFilterV2) -> AppResult<UsageStats> {
        if Self::is_rollup_eligible(filter) {
            return self.get_usage_stats_v2_rollup(filter);
        }

        let conn = self.get_connection()?;
        let mut sql = String::from(
            "SELECT COALESCE(SUM(ue.input_tokens), 0) AS total_input,
                    COALESCE(SUM(ue.output_tokens), 0) AS total_output,
                    COALESCE(SUM(COALESCE(uc.cost_total, 0)), 0) AS total_cost,
                    COUNT(*) AS request_count
             FROM usage_events ue
             LEFT JOIN usage_costs uc ON uc.event_id = ue.event_id
             WHERE 1=1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        Self::append_v2_filter_clauses(&mut sql, &mut params_vec, filter, "ue", "uc");
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let (total_input, total_output, total_cost, request_count): (i64, i64, i64, i64) =
            conn.query_row(&sql, params_refs.as_slice(), |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?;

        Ok(Self::usage_stats_from_totals(
            total_input,
            total_output,
            total_cost,
            request_count,
        ))
    }

    fn get_usage_stats_v2_rollup(&self, filter: &DashboardFilterV2) -> AppResult<UsageStats> {
        let conn = self.get_connection()?;
        let mut sql = String::from(
            "SELECT COALESCE(SUM(input_tokens_total), 0) AS total_input,
                    COALESCE(SUM(output_tokens_total), 0) AS total_output,
                    COALESCE(SUM(cost_total), 0) AS total_cost,
                    COALESCE(SUM(request_count), 0) AS request_count
             FROM analytics_rollup_daily
             WHERE 1=1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        Self::append_rollup_filter_clauses(&mut sql, &mut params_vec, filter);
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let (total_input, total_output, total_cost, request_count): (i64, i64, i64, i64) =
            conn.query_row(&sql, params_refs.as_slice(), |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?;

        Ok(Self::usage_stats_from_totals(
            total_input,
            total_output,
            total_cost,
            request_count,
        ))
    }

    fn aggregate_by_model_v2(&self, filter: &DashboardFilterV2) -> AppResult<Vec<ModelUsage>> {
        if Self::is_rollup_eligible(filter) {
            let conn = self.get_connection()?;
            let mut sql = String::from(
                "SELECT model, provider,
                        COALESCE(SUM(input_tokens_total), 0) AS total_input,
                        COALESCE(SUM(output_tokens_total), 0) AS total_output,
                        COALESCE(SUM(cost_total), 0) AS total_cost,
                        COALESCE(SUM(request_count), 0) AS request_count
                 FROM analytics_rollup_daily
                 WHERE 1=1",
            );
            let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
            Self::append_rollup_filter_clauses(&mut sql, &mut params_vec, filter);
            sql.push_str(" GROUP BY model, provider ORDER BY total_cost DESC, request_count DESC");
            let params_refs: Vec<&dyn rusqlite::ToSql> =
                params_vec.iter().map(|p| p.as_ref()).collect();
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(params_refs.as_slice(), |row| {
                    let total_input: i64 = row.get(2)?;
                    let total_output: i64 = row.get(3)?;
                    let total_cost: i64 = row.get(4)?;
                    let request_count: i64 = row.get(5)?;
                    Ok(ModelUsage {
                        model_name: row.get(0)?,
                        provider: row.get(1)?,
                        stats: Self::usage_stats_from_totals(
                            total_input,
                            total_output,
                            total_cost,
                            request_count,
                        ),
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();
            return Ok(rows);
        }

        let conn = self.get_connection()?;
        let mut sql = String::from(
            "SELECT ue.model, ue.provider,
                    COALESCE(SUM(ue.input_tokens), 0) AS total_input,
                    COALESCE(SUM(ue.output_tokens), 0) AS total_output,
                    COALESCE(SUM(COALESCE(uc.cost_total, 0)), 0) AS total_cost,
                    COUNT(*) AS request_count
             FROM usage_events ue
             LEFT JOIN usage_costs uc ON uc.event_id = ue.event_id
             WHERE 1=1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        Self::append_v2_filter_clauses(&mut sql, &mut params_vec, filter, "ue", "uc");
        sql.push_str(" GROUP BY ue.model, ue.provider ORDER BY total_cost DESC, request_count DESC");
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params_refs.as_slice(), |row| {
                let total_input: i64 = row.get(2)?;
                let total_output: i64 = row.get(3)?;
                let total_cost: i64 = row.get(4)?;
                let request_count: i64 = row.get(5)?;
                Ok(ModelUsage {
                    model_name: row.get(0)?,
                    provider: row.get(1)?,
                    stats: Self::usage_stats_from_totals(
                        total_input,
                        total_output,
                        total_cost,
                        request_count,
                    ),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    fn aggregate_by_project_v2(&self, filter: &DashboardFilterV2) -> AppResult<Vec<ProjectUsage>> {
        if Self::is_rollup_eligible(filter) {
            let conn = self.get_connection()?;
            let mut sql = String::from(
                "SELECT project_id,
                        COALESCE(SUM(input_tokens_total), 0) AS total_input,
                        COALESCE(SUM(output_tokens_total), 0) AS total_output,
                        COALESCE(SUM(cost_total), 0) AS total_cost,
                        COALESCE(SUM(request_count), 0) AS request_count
                 FROM analytics_rollup_daily
                 WHERE project_id != ''",
            );
            let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
            Self::append_rollup_filter_clauses(&mut sql, &mut params_vec, filter);
            sql.push_str(" GROUP BY project_id ORDER BY total_cost DESC, request_count DESC");
            let params_refs: Vec<&dyn rusqlite::ToSql> =
                params_vec.iter().map(|p| p.as_ref()).collect();
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(params_refs.as_slice(), |row| {
                    let total_input: i64 = row.get(1)?;
                    let total_output: i64 = row.get(2)?;
                    let total_cost: i64 = row.get(3)?;
                    let request_count: i64 = row.get(4)?;
                    Ok(ProjectUsage {
                        project_id: row.get(0)?,
                        project_name: None,
                        stats: Self::usage_stats_from_totals(
                            total_input,
                            total_output,
                            total_cost,
                            request_count,
                        ),
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();
            return Ok(rows);
        }

        let conn = self.get_connection()?;
        let mut sql = String::from(
            "SELECT COALESCE(ue.project_id, '') AS project_id,
                    COALESCE(SUM(ue.input_tokens), 0) AS total_input,
                    COALESCE(SUM(ue.output_tokens), 0) AS total_output,
                    COALESCE(SUM(COALESCE(uc.cost_total, 0)), 0) AS total_cost,
                    COUNT(*) AS request_count
             FROM usage_events ue
             LEFT JOIN usage_costs uc ON uc.event_id = ue.event_id
             WHERE COALESCE(ue.project_id, '') != ''",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        Self::append_v2_filter_clauses(&mut sql, &mut params_vec, filter, "ue", "uc");
        sql.push_str(" GROUP BY COALESCE(ue.project_id, '') ORDER BY total_cost DESC, request_count DESC");
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params_refs.as_slice(), |row| {
                let total_input: i64 = row.get(1)?;
                let total_output: i64 = row.get(2)?;
                let total_cost: i64 = row.get(3)?;
                let request_count: i64 = row.get(4)?;
                Ok(ProjectUsage {
                    project_id: row.get(0)?,
                    project_name: None,
                    stats: Self::usage_stats_from_totals(
                        total_input,
                        total_output,
                        total_cost,
                        request_count,
                    ),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    fn get_time_series_v2(
        &self,
        filter: &DashboardFilterV2,
        period: AggregationPeriod,
    ) -> AppResult<Vec<TimeSeriesPoint>> {
        if Self::is_rollup_eligible(filter) && period == AggregationPeriod::Daily {
            let conn = self.get_connection()?;
            let mut sql = String::from(
                "SELECT date_utc AS period,
                        MIN(strftime('%s', date_utc || ' 00:00:00')) AS period_start,
                        COALESCE(SUM(input_tokens_total), 0) AS total_input,
                        COALESCE(SUM(output_tokens_total), 0) AS total_output,
                        COALESCE(SUM(cost_total), 0) AS total_cost,
                        COALESCE(SUM(request_count), 0) AS request_count
                 FROM analytics_rollup_daily
                 WHERE 1=1",
            );
            let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
            Self::append_rollup_filter_clauses(&mut sql, &mut params_vec, filter);
            sql.push_str(" GROUP BY date_utc ORDER BY date_utc ASC");
            let params_refs: Vec<&dyn rusqlite::ToSql> =
                params_vec.iter().map(|p| p.as_ref()).collect();

            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(params_refs.as_slice(), |row| {
                    let total_input: i64 = row.get(2)?;
                    let total_output: i64 = row.get(3)?;
                    let total_cost: i64 = row.get(4)?;
                    let request_count: i64 = row.get(5)?;
                    Ok(TimeSeriesPoint {
                        timestamp: row.get(1)?,
                        timestamp_formatted: row.get(0)?,
                        stats: Self::usage_stats_from_totals(
                            total_input,
                            total_output,
                            total_cost,
                            request_count,
                        ),
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();
            return Ok(rows);
        }

        let conn = self.get_connection()?;
        let mut sql = format!(
            "SELECT strftime('{}', datetime(ue.timestamp_utc, 'unixepoch')) AS period,
                    MIN(ue.timestamp_utc) AS period_start,
                    COALESCE(SUM(ue.input_tokens), 0) AS total_input,
                    COALESCE(SUM(ue.output_tokens), 0) AS total_output,
                    COALESCE(SUM(COALESCE(uc.cost_total, 0)), 0) AS total_cost,
                    COUNT(*) AS request_count
             FROM usage_events ue
             LEFT JOIN usage_costs uc ON uc.event_id = ue.event_id
             WHERE 1=1",
            period.sql_format()
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        Self::append_v2_filter_clauses(&mut sql, &mut params_vec, filter, "ue", "uc");
        sql.push_str(" GROUP BY period ORDER BY period_start ASC");
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params_refs.as_slice(), |row| {
                let total_input: i64 = row.get(2)?;
                let total_output: i64 = row.get(3)?;
                let total_cost: i64 = row.get(4)?;
                let request_count: i64 = row.get(5)?;
                Ok(TimeSeriesPoint {
                    timestamp: row.get(1)?,
                    timestamp_formatted: row.get(0)?,
                    stats: Self::usage_stats_from_totals(
                        total_input,
                        total_output,
                        total_cost,
                        request_count,
                    ),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    fn rebuild_rollup_daily_tx(tx: &rusqlite::Transaction<'_>) -> AppResult<()> {
        tx.execute("DELETE FROM analytics_rollup_daily", [])?;
        tx.execute(
            "INSERT INTO analytics_rollup_daily
             (date_utc, provider, model, project_id, request_count, tokens_total, input_tokens_total, output_tokens_total, cost_total)
             SELECT strftime('%Y-%m-%d', datetime(ue.timestamp_utc, 'unixepoch')) AS date_utc,
                    ue.provider,
                    ue.model,
                    COALESCE(ue.project_id, '') AS project_id,
                    COUNT(*) AS request_count,
                    COALESCE(SUM(ue.input_tokens + ue.output_tokens), 0) AS tokens_total,
                    COALESCE(SUM(ue.input_tokens), 0) AS input_tokens_total,
                    COALESCE(SUM(ue.output_tokens), 0) AS output_tokens_total,
                    COALESCE(SUM(COALESCE(uc.cost_total, 0)), 0) AS cost_total
             FROM usage_events ue
             LEFT JOIN usage_costs uc ON uc.event_id = ue.event_id
             GROUP BY date_utc, ue.provider, ue.model, COALESCE(ue.project_id, '')",
            [],
        )?;
        Ok(())
    }

    fn append_v2_filter_clauses(
        sql: &mut String,
        params_vec: &mut Vec<Box<dyn rusqlite::ToSql>>,
        filter: &DashboardFilterV2,
        event_alias: &str,
        cost_alias: &str,
    ) {
        if let Some(start) = filter.start_timestamp {
            sql.push_str(&format!(" AND {}.timestamp_utc >= ?", event_alias));
            params_vec.push(Box::new(start));
        }
        if let Some(end) = filter.end_timestamp {
            sql.push_str(&format!(" AND {}.timestamp_utc < ?", event_alias));
            params_vec.push(Box::new(end));
        }
        if let Some(ref model) = filter.model_name {
            sql.push_str(&format!(" AND {}.model = ?", event_alias));
            params_vec.push(Box::new(model.clone()));
        }
        if let Some(ref provider) = filter.provider {
            sql.push_str(&format!(" AND {}.provider = ?", event_alias));
            params_vec.push(Box::new(provider.clone()));
        }
        if let Some(ref session_id) = filter.session_id {
            sql.push_str(&format!(" AND {}.session_id = ?", event_alias));
            params_vec.push(Box::new(session_id.clone()));
        }
        if let Some(ref project_id) = filter.project_id {
            sql.push_str(&format!(" AND COALESCE({}.project_id, '') = ?", event_alias));
            params_vec.push(Box::new(project_id.clone()));
        }
        if let Some(cost_status) = &filter.cost_status {
            sql.push_str(&format!(
                " AND COALESCE({}.cost_status, 'missing') = ?",
                cost_alias
            ));
            params_vec.push(Box::new(cost_status.as_str().to_string()));
        }
    }

    fn append_rollup_filter_clauses(
        sql: &mut String,
        params_vec: &mut Vec<Box<dyn rusqlite::ToSql>>,
        filter: &DashboardFilterV2,
    ) {
        if let Some(start) = filter.start_timestamp {
            sql.push_str(" AND date_utc >= ?");
            params_vec.push(Box::new(Self::timestamp_to_date_utc(start)));
        }
        if let Some(end) = filter.end_timestamp {
            sql.push_str(" AND date_utc < ?");
            params_vec.push(Box::new(Self::timestamp_to_date_utc(end)));
        }
        if let Some(ref model) = filter.model_name {
            sql.push_str(" AND model = ?");
            params_vec.push(Box::new(model.clone()));
        }
        if let Some(ref provider) = filter.provider {
            sql.push_str(" AND provider = ?");
            params_vec.push(Box::new(provider.clone()));
        }
        if let Some(ref project_id) = filter.project_id {
            sql.push_str(" AND project_id = ?");
            params_vec.push(Box::new(project_id.clone()));
        }
    }

    fn calculate_previous_period_filter_v2(filter: &DashboardFilterV2) -> DashboardFilterV2 {
        let mut prev = filter.clone();
        if let (Some(start), Some(end)) = (filter.start_timestamp, filter.end_timestamp) {
            let duration = end - start;
            prev.start_timestamp = Some(start - duration);
            prev.end_timestamp = Some(start);
        }
        prev
    }

    fn is_rollup_eligible(filter: &DashboardFilterV2) -> bool {
        if filter.session_id.is_some() || filter.cost_status.is_some() {
            return false;
        }
        if let Some(start) = filter.start_timestamp {
            if start % 86_400 != 0 {
                return false;
            }
        }
        if let Some(end) = filter.end_timestamp {
            if end % 86_400 != 0 {
                return false;
            }
        }
        true
    }

    fn usage_stats_from_totals(
        total_input: i64,
        total_output: i64,
        total_cost: i64,
        request_count: i64,
    ) -> UsageStats {
        let avg_tokens = if request_count > 0 {
            (total_input + total_output) as f64 / request_count as f64
        } else {
            0.0
        };
        let avg_cost = if request_count > 0 {
            total_cost as f64 / request_count as f64
        } else {
            0.0
        };

        UsageStats {
            total_input_tokens: total_input,
            total_output_tokens: total_output,
            total_cost_microdollars: total_cost,
            request_count,
            avg_tokens_per_request: avg_tokens,
            avg_cost_per_request: avg_cost,
        }
    }

    fn timestamp_to_date_utc(ts: i64) -> String {
        chrono::DateTime::from_timestamp(ts, 0)
            .map(|dt| dt.date_naive().format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "1970-01-01".to_string())
    }

    fn default_export_path(extension: &str) -> String {
        let file_name = format!(
            "analytics_export_{}.{}",
            chrono::Utc::now().format("%Y%m%d_%H%M%S"),
            extension
        );
        let base = std::env::var("HOME")
            .ok()
            .map(|home| Path::new(&home).join("Downloads"))
            .unwrap_or_else(std::env::temp_dir);
        base.join(file_name).to_string_lossy().to_string()
    }

    fn csv_escape_local(value: &str) -> String {
        if value.contains(',') || value.contains('"') || value.contains('\n') {
            format!("\"{}\"", value.replace('"', "\"\""))
        } else {
            value.to_string()
        }
    }

    // ========================================================================
    // Analytics v2 dual-write helpers
    // ========================================================================

    fn dual_write_v2_tx(tx: &rusqlite::Transaction<'_>, record: &UsageRecord) -> AppResult<()> {
        let event_id = uuid::Uuid::new_v4().to_string();
        let project_norm = record.project_id.clone().unwrap_or_default();

        tx.execute(
            "INSERT INTO usage_events
             (event_id, session_id, project_id, provider, model, input_tokens, output_tokens,
              thinking_tokens, cache_read_tokens, cache_write_tokens,
              timestamp_utc, metadata_json, ingest_status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 'ingested')",
            params![
                event_id,
                record.session_id,
                record.project_id,
                record.provider,
                record.model_name,
                record.input_tokens,
                record.output_tokens,
                record.thinking_tokens,
                record.cache_read_tokens,
                record.cache_creation_tokens,
                record.timestamp,
                record.metadata,
            ],
        )?;

        let (rule_id, cost_total, cost_status, currency, breakdown_json) =
            Self::resolve_cost_for_record_tx(tx, record)?;

        tx.execute(
            "INSERT INTO usage_costs
             (event_id, rule_id, cost_total, currency, cost_status, cost_breakdown_json, computed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(event_id) DO UPDATE SET
                rule_id = excluded.rule_id,
                cost_total = excluded.cost_total,
                currency = excluded.currency,
                cost_status = excluded.cost_status,
                cost_breakdown_json = excluded.cost_breakdown_json,
                computed_at = excluded.computed_at",
            params![
                event_id,
                rule_id,
                cost_total,
                currency,
                cost_status.as_str(),
                breakdown_json,
                chrono::Utc::now().timestamp(),
            ],
        )?;

        let date_utc = chrono::DateTime::from_timestamp(record.timestamp, 0)
            .map(|dt| dt.date_naive().format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "1970-01-01".to_string());

        tx.execute(
            "INSERT INTO analytics_rollup_daily
             (date_utc, provider, model, project_id, request_count, tokens_total, input_tokens_total, output_tokens_total, cost_total)
             VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?7, ?8)
             ON CONFLICT(date_utc, provider, model, project_id) DO UPDATE SET
                request_count = request_count + 1,
                tokens_total = tokens_total + excluded.tokens_total,
                input_tokens_total = input_tokens_total + excluded.input_tokens_total,
                output_tokens_total = output_tokens_total + excluded.output_tokens_total,
                cost_total = cost_total + excluded.cost_total",
            params![
                date_utc,
                record.provider,
                record.model_name,
                project_norm,
                record.total_tokens(),
                record.input_tokens,
                record.output_tokens,
                cost_total
            ],
        )?;

        Ok(())
    }

    fn resolve_cost_for_record_tx(
        tx: &rusqlite::Transaction<'_>,
        record: &UsageRecord,
    ) -> AppResult<(Option<String>, i64, CostStatus, String, Option<String>)> {
        let mut stmt = tx.prepare(
            "SELECT id, model_pattern, currency, input_per_million, output_per_million,
                    cache_read_per_million, cache_write_per_million, thinking_per_million
             FROM pricing_rules
             WHERE provider = ?1
               AND status = ?2
               AND effective_from <= ?3
               AND (effective_to IS NULL OR effective_to > ?3)
             ORDER BY effective_from DESC, updated_at DESC",
        )?;

        let rows = stmt.query_map(
            params![
                &record.provider,
                PricingRuleStatus::Active.as_str(),
                record.timestamp
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                ))
            },
        )?;

        for item in rows {
            let (
                rule_id,
                model_pattern,
                currency,
                input_per_million,
                output_per_million,
                cache_read_per_million,
                cache_write_per_million,
                thinking_per_million,
            ) = item?;

            if !Self::wildcard_match(&model_pattern, &record.model_name) {
                continue;
            }

            let input_cost = (record.input_tokens * input_per_million) / 1_000_000;
            let output_cost = (record.output_tokens * output_per_million) / 1_000_000;
            let thinking_cost = (record.thinking_tokens * thinking_per_million) / 1_000_000;
            let cache_read_cost = (record.cache_read_tokens * cache_read_per_million) / 1_000_000;
            let cache_write_cost =
                (record.cache_creation_tokens * cache_write_per_million) / 1_000_000;

            let breakdown = CostBreakdown {
                input_cost_microdollars: input_cost,
                output_cost_microdollars: output_cost,
                thinking_cost_microdollars: thinking_cost,
                cache_read_cost_microdollars: cache_read_cost,
                cache_write_cost_microdollars: cache_write_cost,
                total_cost_microdollars: input_cost
                    + output_cost
                    + thinking_cost
                    + cache_read_cost
                    + cache_write_cost,
            };

            return Ok((
                Some(rule_id),
                breakdown.total_cost_microdollars,
                CostStatus::Exact,
                currency,
                Some(serde_json::to_string(&breakdown)?),
            ));
        }

        if record.cost_microdollars > 0 {
            let breakdown = serde_json::json!({
                "legacy_total_cost_microdollars": record.cost_microdollars,
            });
            return Ok((
                None,
                record.cost_microdollars,
                CostStatus::Estimated,
                "USD".to_string(),
                Some(breakdown.to_string()),
            ));
        }

        Ok((None, 0, CostStatus::Missing, "USD".to_string(), None))
    }

    pub(crate) fn wildcard_match(pattern: &str, value: &str) -> bool {
        if pattern == "*" || pattern == "%" {
            return true;
        }

        let escaped = regex::escape(pattern);
        let wildcard = escaped.replace("\\*", ".*").replace("\\%", ".*");
        let regex = format!("^{}$", wildcard);
        match Regex::new(&regex) {
            Ok(re) => re.is_match(value),
            Err(_) => false,
        }
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    fn row_to_pricing_rule(row: &rusqlite::Row) -> rusqlite::Result<PricingRule> {
        let status_raw: String = row.get(11)?;
        Ok(PricingRule {
            id: row.get(0)?,
            provider: row.get(1)?,
            model_pattern: row.get(2)?,
            currency: row.get(3)?,
            input_per_million: row.get(4)?,
            output_per_million: row.get(5)?,
            cache_read_per_million: row.get(6)?,
            cache_write_per_million: row.get(7)?,
            thinking_per_million: row.get(8)?,
            effective_from: row.get(9)?,
            effective_to: row.get(10)?,
            status: PricingRuleStatus::from_str(&status_raw),
            created_at: row.get(12)?,
            updated_at: row.get(13)?,
            note: row.get(14)?,
        })
    }

    fn row_to_usage_record_v2(row: &rusqlite::Row) -> rusqlite::Result<UsageRecordV2> {
        let status_raw: String = row.get(15)?;
        Ok(UsageRecordV2 {
            event_id: row.get(0)?,
            session_id: row.get(1)?,
            project_id: row.get(2)?,
            model_name: row.get(3)?,
            provider: row.get(4)?,
            input_tokens: row.get(5)?,
            output_tokens: row.get(6)?,
            thinking_tokens: row.get(7)?,
            cache_read_tokens: row.get(8)?,
            cache_creation_tokens: row.get(9)?,
            cost_microdollars: row.get(10)?,
            timestamp: row.get(11)?,
            metadata: row.get(12)?,
            pricing_rule_id: row.get(13)?,
            currency: row.get(14)?,
            cost_status: CostStatus::from_str(&status_raw),
            cost_breakdown_json: row.get(16)?,
        })
    }

    /// Convert a database row to UsageRecord
    fn row_to_usage_record(row: &rusqlite::Row) -> rusqlite::Result<UsageRecord> {
        Ok(UsageRecord {
            id: row.get(0)?,
            session_id: row.get(1)?,
            project_id: row.get(2)?,
            model_name: row.get(3)?,
            provider: row.get(4)?,
            input_tokens: row.get(5)?,
            output_tokens: row.get(6)?,
            thinking_tokens: row.get(7)?,
            cache_read_tokens: row.get(8)?,
            cache_creation_tokens: row.get(9)?,
            cost_microdollars: row.get(10)?,
            timestamp: row.get(11)?,
            metadata: row.get(12)?,
        })
    }

    /// Convert a database row to ModelPricing
    fn row_to_model_pricing(row: &rusqlite::Row) -> rusqlite::Result<ModelPricing> {
        let is_custom_int: i32 = row.get(5)?;
        Ok(ModelPricing {
            id: row.get(0)?,
            model_name: row.get(1)?,
            provider: row.get(2)?,
            input_price_per_million: row.get(3)?,
            output_price_per_million: row.get(4)?,
            is_custom: is_custom_int != 0,
            updated_at: row.get(6)?,
        })
    }

    /// Check if the service is healthy
    pub fn is_healthy(&self) -> bool {
        if let Ok(conn) = self.pool.get() {
            conn.query_row("SELECT 1", [], |_| Ok(())).is_ok()
        } else {
            false
        }
    }
}

impl std::fmt::Debug for AnalyticsService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnalyticsService")
            .field("pool_size", &self.pool.state().connections)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_service() -> AppResult<AnalyticsService> {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::builder()
            .max_size(1)
            .build(manager)
            .map_err(|e| AppError::database(e.to_string()))?;
        AnalyticsService::from_pool(pool)
    }

    #[test]
    fn test_service_creation() {
        let service = create_test_service().unwrap();
        assert!(service.is_healthy());
    }

    #[test]
    fn test_insert_and_get_usage_record() {
        let service = create_test_service().unwrap();

        let record = UsageRecord::new("claude-3-5-sonnet", "anthropic", 1000, 500)
            .with_session("sess-123")
            .with_cost(5000);

        let id = service.insert_usage_record(&record).unwrap();
        assert!(id > 0);

        let fetched = service.get_usage_record(id).unwrap().unwrap();
        assert_eq!(fetched.model_name, "claude-3-5-sonnet");
        assert_eq!(fetched.input_tokens, 1000);
        assert_eq!(fetched.output_tokens, 500);
        assert_eq!(fetched.cost_microdollars, 5000);
    }

    #[test]
    fn test_list_usage_records_with_filter() {
        let service = create_test_service().unwrap();

        // Insert records for different models
        let record1 = UsageRecord::new("claude-3-5-sonnet", "anthropic", 1000, 500);
        let record2 = UsageRecord::new("gpt-4", "openai", 2000, 1000);

        service.insert_usage_record(&record1).unwrap();
        service.insert_usage_record(&record2).unwrap();

        // Filter by model
        let filter = UsageFilter::default().with_model("claude-3-5-sonnet");
        let records = service.list_usage_records(&filter, None, None).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].model_name, "claude-3-5-sonnet");
    }

    #[test]
    fn test_batch_insert() {
        let service = create_test_service().unwrap();

        let records = vec![
            UsageRecord::new("model1", "provider1", 100, 50),
            UsageRecord::new("model2", "provider2", 200, 100),
            UsageRecord::new("model3", "provider3", 300, 150),
        ];

        let ids = service.insert_usage_records_batch(&records).unwrap();
        assert_eq!(ids.len(), 3);

        let filter = UsageFilter::default();
        let all = service.list_usage_records(&filter, None, None).unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_usage_stats() {
        let service = create_test_service().unwrap();

        service
            .insert_usage_record(&UsageRecord::new("m1", "p1", 1000, 500).with_cost(100))
            .unwrap();
        service
            .insert_usage_record(&UsageRecord::new("m1", "p1", 2000, 1000).with_cost(200))
            .unwrap();

        let stats = service.get_usage_stats(&UsageFilter::default()).unwrap();
        assert_eq!(stats.total_input_tokens, 3000);
        assert_eq!(stats.total_output_tokens, 1500);
        assert_eq!(stats.total_cost_microdollars, 300);
        assert_eq!(stats.request_count, 2);
    }

    #[test]
    fn test_model_pricing_crud() {
        let service = create_test_service().unwrap();

        // Default pricing should be loaded
        let pricing = service
            .get_model_pricing("claude-3-5-sonnet-20241022", "anthropic")
            .unwrap();
        assert!(pricing.is_some());
        let p = pricing.unwrap();
        assert_eq!(p.input_price_per_million, 3_000_000);
        assert_eq!(p.output_price_per_million, 15_000_000);

        // Custom pricing
        let custom = ModelPricing {
            id: 0,
            model_name: "custom-model".to_string(),
            provider: "custom".to_string(),
            input_price_per_million: 1_000_000,
            output_price_per_million: 2_000_000,
            is_custom: true,
            updated_at: chrono::Utc::now().timestamp(),
        };
        service.upsert_model_pricing(&custom).unwrap();

        let fetched = service
            .get_model_pricing("custom-model", "custom")
            .unwrap()
            .unwrap();
        assert_eq!(fetched.input_price_per_million, 1_000_000);
        assert!(fetched.is_custom);
    }

    #[test]
    fn test_delete_usage_record() {
        let service = create_test_service().unwrap();

        let record = UsageRecord::new("m1", "p1", 100, 50);
        let id = service.insert_usage_record(&record).unwrap();

        assert!(service.get_usage_record(id).unwrap().is_some());

        let deleted = service.delete_usage_record(id).unwrap();
        assert!(deleted);

        assert!(service.get_usage_record(id).unwrap().is_none());
    }

    #[test]
    fn test_v2_pricing_rule_and_record_query() {
        let service = create_test_service().unwrap();

        let mut rule = PricingRule::new("anthropic", "claude-*");
        rule.input_per_million = 2_000_000;
        rule.output_per_million = 10_000_000;
        rule.effective_from = 0;
        let saved = service.upsert_pricing_rule(&rule).unwrap();
        assert!(!saved.id.is_empty());

        let record = UsageRecord::new("claude-sonnet-4-20250514", "anthropic", 1_000, 500);
        service.insert_usage_record(&record).unwrap();

        let filter = DashboardFilterV2::default();
        let rows = service.list_usage_records_v2(&filter, Some(10), Some(0)).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].provider, "anthropic");
        assert_eq!(rows[0].cost_status, CostStatus::Exact);

        let count = service.count_usage_records_v2(&filter).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_recompute_costs_missing_path() {
        let service = create_test_service().unwrap();
        service
            .insert_usage_record(&UsageRecord::new("unknown-model", "unknown-provider", 100, 50))
            .unwrap();

        let result = service
            .recompute_costs(&RecomputeCostsRequest {
                filter: DashboardFilterV2::default(),
            })
            .unwrap();

        assert_eq!(result.scanned_records, 1);
        assert_eq!(result.recomputed_records, 1);
        assert_eq!(result.missing_records, 1);
    }
}
