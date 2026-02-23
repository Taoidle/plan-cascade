//! Analytics Service
//!
//! Core service for managing analytics data with SQLite storage.
//! Provides CRUD operations, schema initialization, and connection pooling.

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;

use crate::models::analytics::{ModelPricing, UsageFilter, UsageRecord, UsageStats};
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

        Ok(())
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
        let conn = self.get_connection()?;

        conn.execute(
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

        let id = conn.last_insert_rowid();
        Ok(id)
    }

    /// Insert multiple usage records in a batch
    pub fn insert_usage_records_batch(&self, records: &[UsageRecord]) -> AppResult<Vec<i64>> {
        let conn = self.get_connection()?;
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
    // Helper Methods
    // ========================================================================

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
}
