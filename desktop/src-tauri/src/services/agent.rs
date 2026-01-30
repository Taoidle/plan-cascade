//! Agent Service
//!
//! Business logic for managing AI agents with custom behaviors.
//! Provides CRUD operations and an in-memory registry for active agents.

use std::collections::HashMap;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;

use crate::models::agent::{
    Agent, AgentCreateRequest, AgentRun, AgentRunList, AgentStats, AgentUpdateRequest,
    AgentWithStats, RunStatus,
};
use crate::utils::error::{AppError, AppResult};

/// Type alias for the database pool
type DbPool = Pool<SqliteConnectionManager>;

/// In-memory registry for caching active agents
#[derive(Debug, Default)]
pub struct AgentRegistry {
    agents: HashMap<String, Agent>,
}

impl AgentRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
        }
    }

    /// Get an agent from the cache
    pub fn get(&self, id: &str) -> Option<&Agent> {
        self.agents.get(id)
    }

    /// Add or update an agent in the cache
    pub fn insert(&mut self, agent: Agent) {
        self.agents.insert(agent.id.clone(), agent);
    }

    /// Remove an agent from the cache
    pub fn remove(&mut self, id: &str) -> Option<Agent> {
        self.agents.remove(id)
    }

    /// Clear all agents from the cache
    pub fn clear(&mut self) {
        self.agents.clear();
    }

    /// Get all cached agents
    pub fn all(&self) -> Vec<&Agent> {
        self.agents.values().collect()
    }

    /// Check if an agent is in the cache
    pub fn contains(&self, id: &str) -> bool {
        self.agents.contains_key(id)
    }
}

/// Service for managing AI agents
#[derive(Clone)]
pub struct AgentService {
    pool: DbPool,
}

impl AgentService {
    /// Create a new agent service with the given database pool
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Get a database connection from the pool
    fn get_conn(&self) -> AppResult<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))
    }

    // ========================================================================
    // Agent CRUD Operations (synchronous internally, wrapped for async)
    // ========================================================================

    /// Create a new agent
    pub async fn create_agent(&self, request: AgentCreateRequest) -> AppResult<Agent> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            Self::create_agent_sync(&pool, request)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))?
    }

    fn create_agent_sync(pool: &DbPool, request: AgentCreateRequest) -> AppResult<Agent> {
        // Validate the request
        request.validate().map_err(|e| AppError::validation(e))?;

        let conn = pool.get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        // Check for duplicate name
        let existing: Result<String, _> = conn.query_row(
            "SELECT id FROM agents WHERE name = ?1",
            params![request.name],
            |row| row.get(0),
        );

        if existing.is_ok() {
            return Err(AppError::validation(format!(
                "Agent with name '{}' already exists",
                request.name
            )));
        }

        let agent = Agent::create(&request.name, &request.system_prompt, &request.model)
            .with_allowed_tools(request.allowed_tools);

        let agent = if let Some(desc) = request.description {
            agent.with_description(desc)
        } else {
            agent
        };

        let allowed_tools_json = serde_json::to_string(&agent.allowed_tools)?;

        conn.execute(
            "INSERT INTO agents (id, name, description, system_prompt, model, allowed_tools, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                agent.id,
                agent.name,
                agent.description,
                agent.system_prompt,
                agent.model,
                allowed_tools_json,
                agent.created_at,
                agent.updated_at,
            ],
        )?;

        Ok(agent)
    }

    /// Get an agent by ID
    pub async fn get_agent(&self, id: &str) -> AppResult<Option<Agent>> {
        let pool = self.pool.clone();
        let id = id.to_string();
        tokio::task::spawn_blocking(move || {
            Self::get_agent_sync(&pool, &id)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))?
    }

    fn get_agent_sync(pool: &DbPool, id: &str) -> AppResult<Option<Agent>> {
        let conn = pool.get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let result = conn.query_row(
            "SELECT id, name, description, system_prompt, model, allowed_tools, created_at, updated_at
             FROM agents WHERE id = ?1",
            params![id],
            |row| Self::row_to_agent(row),
        );

        match result {
            Ok(agent) => Ok(Some(agent)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::database(e.to_string())),
        }
    }

    /// Get an agent by name
    pub async fn get_agent_by_name(&self, name: &str) -> AppResult<Option<Agent>> {
        let pool = self.pool.clone();
        let name = name.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()
                .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

            let result = conn.query_row(
                "SELECT id, name, description, system_prompt, model, allowed_tools, created_at, updated_at
                 FROM agents WHERE name = ?1",
                params![name],
                |row| Self::row_to_agent(row),
            );

            match result {
                Ok(agent) => Ok(Some(agent)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(AppError::database(e.to_string())),
            }
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))?
    }

    /// List all agents
    pub async fn list_agents(&self) -> AppResult<Vec<Agent>> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            Self::list_agents_sync(&pool)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))?
    }

    fn list_agents_sync(pool: &DbPool) -> AppResult<Vec<Agent>> {
        let conn = pool.get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let mut stmt = conn.prepare(
            "SELECT id, name, description, system_prompt, model, allowed_tools, created_at, updated_at
             FROM agents ORDER BY name ASC",
        )?;

        let agents = stmt
            .query_map([], |row| Self::row_to_agent(row))?
            .filter_map(|r| r.ok())
            .collect::<Vec<_>>();

        Ok(agents)
    }

    /// List all agents with their statistics
    pub async fn list_agents_with_stats(&self) -> AppResult<Vec<AgentWithStats>> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let agents = Self::list_agents_sync(&pool)?;
            let mut result = Vec::with_capacity(agents.len());

            for agent in agents {
                let stats = Self::get_agent_stats_sync(&pool, &agent.id)?;
                result.push(AgentWithStats { agent, stats });
            }

            Ok(result)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))?
    }

    /// Update an existing agent
    pub async fn update_agent(&self, id: &str, request: AgentUpdateRequest) -> AppResult<Agent> {
        let pool = self.pool.clone();
        let id = id.to_string();
        tokio::task::spawn_blocking(move || {
            Self::update_agent_sync(&pool, &id, request)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))?
    }

    fn update_agent_sync(pool: &DbPool, id: &str, request: AgentUpdateRequest) -> AppResult<Agent> {
        // Validate the request
        request.validate().map_err(|e| AppError::validation(e))?;

        if !request.has_updates() {
            return Err(AppError::validation("No fields to update"));
        }

        // Get existing agent
        let existing = Self::get_agent_sync(pool, id)?
            .ok_or_else(|| AppError::not_found(format!("Agent not found: {}", id)))?;

        // Check for name conflict if updating name
        if let Some(ref new_name) = request.name {
            if new_name != &existing.name {
                let conn = pool.get()
                    .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

                let existing_with_name: Result<String, _> = conn.query_row(
                    "SELECT id FROM agents WHERE name = ?1",
                    params![new_name],
                    |row| row.get(0),
                );

                if existing_with_name.is_ok() {
                    return Err(AppError::validation(format!(
                        "Agent with name '{}' already exists",
                        new_name
                    )));
                }
            }
        }

        let now = chrono::Utc::now().to_rfc3339();
        let updated = Agent {
            id: existing.id.clone(),
            name: request.name.unwrap_or(existing.name),
            description: request.description.or(existing.description),
            system_prompt: request.system_prompt.unwrap_or(existing.system_prompt),
            model: request.model.unwrap_or(existing.model),
            allowed_tools: request.allowed_tools.unwrap_or(existing.allowed_tools),
            created_at: existing.created_at,
            updated_at: Some(now),
        };

        let conn = pool.get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;
        let allowed_tools_json = serde_json::to_string(&updated.allowed_tools)?;

        conn.execute(
            "UPDATE agents SET name = ?2, description = ?3, system_prompt = ?4, model = ?5,
             allowed_tools = ?6, updated_at = ?7 WHERE id = ?1",
            params![
                updated.id,
                updated.name,
                updated.description,
                updated.system_prompt,
                updated.model,
                allowed_tools_json,
                updated.updated_at,
            ],
        )?;

        Ok(updated)
    }

    /// Delete an agent and all associated history
    pub async fn delete_agent(&self, id: &str) -> AppResult<()> {
        let pool = self.pool.clone();
        let id = id.to_string();
        tokio::task::spawn_blocking(move || {
            // Verify agent exists
            if Self::get_agent_sync(&pool, &id)?.is_none() {
                return Err(AppError::not_found(format!("Agent not found: {}", id)));
            }

            let conn = pool.get()
                .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

            // Delete associated runs first
            conn.execute("DELETE FROM agent_runs WHERE agent_id = ?1", params![id])?;

            // Delete the agent
            conn.execute("DELETE FROM agents WHERE id = ?1", params![id])?;

            Ok(())
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))?
    }

    // ========================================================================
    // Agent Run History Operations
    // ========================================================================

    /// Create a new agent run
    pub async fn create_run(&self, agent_id: &str, input: &str) -> AppResult<AgentRun> {
        let pool = self.pool.clone();
        let agent_id = agent_id.to_string();
        let input = input.to_string();
        tokio::task::spawn_blocking(move || {
            // Verify agent exists
            if Self::get_agent_sync(&pool, &agent_id)?.is_none() {
                return Err(AppError::not_found(format!("Agent not found: {}", agent_id)));
            }

            let run = AgentRun::new(&agent_id, &input);

            let conn = pool.get()
                .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

            conn.execute(
                "INSERT INTO agent_runs (id, agent_id, input, status, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    run.id,
                    run.agent_id,
                    run.input,
                    run.status.to_string(),
                    run.created_at,
                ],
            )?;

            Ok(run)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))?
    }

    /// Update a run's status and result
    pub async fn update_run(&self, run: &AgentRun) -> AppResult<()> {
        let pool = self.pool.clone();
        let run = run.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()
                .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

            conn.execute(
                "UPDATE agent_runs SET output = ?2, status = ?3, duration_ms = ?4, input_tokens = ?5,
                 output_tokens = ?6, error = ?7, completed_at = ?8 WHERE id = ?1",
                params![
                    run.id,
                    run.output,
                    run.status.to_string(),
                    run.duration_ms,
                    run.input_tokens,
                    run.output_tokens,
                    run.error,
                    run.completed_at,
                ],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))?
    }

    /// Get a run by ID
    pub async fn get_run(&self, id: &str) -> AppResult<Option<AgentRun>> {
        let pool = self.pool.clone();
        let id = id.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()
                .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

            let result = conn.query_row(
                "SELECT id, agent_id, input, output, status, duration_ms, input_tokens, output_tokens,
                 error, created_at, completed_at FROM agent_runs WHERE id = ?1",
                params![id],
                |row| Self::row_to_agent_run(row),
            );

            match result {
                Ok(run) => Ok(Some(run)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(AppError::database(e.to_string())),
            }
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))?
    }

    /// Get run history for an agent with pagination
    pub async fn get_run_history(
        &self,
        agent_id: &str,
        limit: u32,
        offset: u32,
    ) -> AppResult<AgentRunList> {
        let pool = self.pool.clone();
        let agent_id = agent_id.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()
                .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

            // Get total count
            let total: u32 = conn.query_row(
                "SELECT COUNT(*) FROM agent_runs WHERE agent_id = ?1",
                params![agent_id],
                |row| row.get(0),
            )?;

            // Get runs with pagination
            let mut stmt = conn.prepare(
                "SELECT id, agent_id, input, output, status, duration_ms, input_tokens, output_tokens,
                 error, created_at, completed_at FROM agent_runs
                 WHERE agent_id = ?1 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3",
            )?;

            let runs = stmt
                .query_map(params![agent_id, limit, offset], |row| {
                    Self::row_to_agent_run(row)
                })?
                .filter_map(|r| r.ok())
                .collect();

            Ok(AgentRunList {
                runs,
                total,
                offset,
                limit,
            })
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))?
    }

    /// Get statistics for an agent
    pub async fn get_agent_stats(&self, agent_id: &str) -> AppResult<AgentStats> {
        let pool = self.pool.clone();
        let agent_id = agent_id.to_string();
        tokio::task::spawn_blocking(move || {
            Self::get_agent_stats_sync(&pool, &agent_id)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))?
    }

    fn get_agent_stats_sync(pool: &DbPool, agent_id: &str) -> AppResult<AgentStats> {
        let conn = pool.get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let mut stats = AgentStats::default();

        // Get run counts by status
        let mut stmt = conn.prepare(
            "SELECT status, COUNT(*) FROM agent_runs WHERE agent_id = ?1 GROUP BY status",
        )?;

        let counts = stmt.query_map(params![agent_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?))
        })?;

        for result in counts {
            if let Ok((status_str, count)) = result {
                stats.total_runs += count;
                match status_str.as_str() {
                    "completed" => stats.completed_runs = count,
                    "failed" => stats.failed_runs = count,
                    "cancelled" => stats.cancelled_runs = count,
                    _ => {}
                }
            }
        }

        stats.calculate_success_rate();

        // Get average duration and token totals for completed runs
        let totals: (Option<f64>, Option<i64>, Option<i64>) = conn
            .query_row(
                "SELECT AVG(duration_ms), SUM(input_tokens), SUM(output_tokens)
             FROM agent_runs WHERE agent_id = ?1 AND status = 'completed'",
                params![agent_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap_or((None, None, None));

        stats.avg_duration_ms = totals.0.unwrap_or(0.0);
        stats.total_input_tokens = totals.1.unwrap_or(0) as u64;
        stats.total_output_tokens = totals.2.unwrap_or(0) as u64;

        // Get last run timestamp
        let last_run: Option<String> = conn
            .query_row(
                "SELECT created_at FROM agent_runs WHERE agent_id = ?1 ORDER BY created_at DESC LIMIT 1",
                params![agent_id],
                |row| row.get(0),
            )
            .ok();

        stats.last_run_at = last_run;

        Ok(stats)
    }

    /// Delete old runs based on retention policy
    pub async fn prune_old_runs(&self, agent_id: &str, keep_count: u32) -> AppResult<u32> {
        let pool = self.pool.clone();
        let agent_id = agent_id.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()
                .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

            // Get the created_at of the nth most recent run
            let cutoff: Option<String> = conn
                .query_row(
                    "SELECT created_at FROM agent_runs WHERE agent_id = ?1
                 ORDER BY created_at DESC LIMIT 1 OFFSET ?2",
                    params![agent_id, keep_count.saturating_sub(1)],
                    |row| row.get(0),
                )
                .ok();

            if let Some(cutoff_time) = cutoff {
                let deleted = conn.execute(
                    "DELETE FROM agent_runs WHERE agent_id = ?1 AND created_at < ?2",
                    params![agent_id, cutoff_time],
                )?;
                Ok(deleted as u32)
            } else {
                Ok(0)
            }
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))?
    }

    // ========================================================================
    // Helper Functions
    // ========================================================================

    /// Convert a database row to an Agent
    fn row_to_agent(row: &rusqlite::Row) -> rusqlite::Result<Agent> {
        let id: String = row.get(0)?;
        let name: String = row.get(1)?;
        let description: Option<String> = row.get(2)?;
        let system_prompt: String = row.get(3)?;
        let model: String = row.get(4)?;
        let allowed_tools_json: String = row.get::<_, String>(5).unwrap_or_else(|_| "[]".to_string());
        let created_at: Option<String> = row.get(6)?;
        let updated_at: Option<String> = row.get(7)?;

        let allowed_tools: Vec<String> =
            serde_json::from_str(&allowed_tools_json).unwrap_or_default();

        Ok(Agent {
            id,
            name,
            description,
            system_prompt,
            model,
            allowed_tools,
            created_at,
            updated_at,
        })
    }

    /// Convert a database row to an AgentRun
    fn row_to_agent_run(row: &rusqlite::Row) -> rusqlite::Result<AgentRun> {
        let id: String = row.get(0)?;
        let agent_id: String = row.get(1)?;
        let input: String = row.get(2)?;
        let output: Option<String> = row.get(3)?;
        let status_str: String = row.get(4)?;
        let duration_ms: Option<u64> = row.get(5)?;
        let input_tokens: Option<u32> = row.get(6)?;
        let output_tokens: Option<u32> = row.get(7)?;
        let error: Option<String> = row.get(8)?;
        let created_at: Option<String> = row.get(9)?;
        let completed_at: Option<String> = row.get(10)?;

        let status = status_str.parse().unwrap_or(RunStatus::Pending);

        Ok(AgentRun {
            id,
            agent_id,
            input,
            output,
            status,
            duration_ms,
            input_tokens,
            output_tokens,
            error,
            created_at,
            completed_at,
        })
    }
}

impl std::fmt::Debug for AgentService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentService")
            .field("pool_size", &self.pool.state().connections)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_pool() -> DbPool {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::builder().max_size(1).build(manager).unwrap();

        // Initialize schema
        let conn = pool.get().unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS agents (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                description TEXT,
                system_prompt TEXT NOT NULL,
                model TEXT NOT NULL DEFAULT 'claude-sonnet-4-20250514',
                allowed_tools TEXT DEFAULT '[]',
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )
        .unwrap();

        conn.execute(
            "CREATE TABLE IF NOT EXISTS agent_runs (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                input TEXT NOT NULL,
                output TEXT,
                status TEXT NOT NULL DEFAULT 'pending',
                duration_ms INTEGER,
                input_tokens INTEGER,
                output_tokens INTEGER,
                error TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                completed_at TEXT,
                FOREIGN KEY (agent_id) REFERENCES agents(id) ON DELETE CASCADE
            )",
            [],
        )
        .unwrap();

        pool
    }

    #[tokio::test]
    async fn test_create_agent() {
        let pool = create_test_pool();
        let service = AgentService::new(pool);

        let request = AgentCreateRequest {
            name: "Test Agent".to_string(),
            description: Some("A test agent".to_string()),
            system_prompt: "You are a helpful assistant.".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            allowed_tools: vec!["read_file".to_string()],
        };

        let agent = service.create_agent(request).await.unwrap();
        assert_eq!(agent.name, "Test Agent");
        assert!(!agent.id.is_empty());
    }

    #[tokio::test]
    async fn test_duplicate_name_rejected() {
        let pool = create_test_pool();
        let service = AgentService::new(pool);

        let request = AgentCreateRequest {
            name: "Test Agent".to_string(),
            description: None,
            system_prompt: "Prompt".to_string(),
            model: "model".to_string(),
            allowed_tools: vec![],
        };

        service.create_agent(request.clone()).await.unwrap();
        let result = service.create_agent(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_agents() {
        let pool = create_test_pool();
        let service = AgentService::new(pool);

        // Create two agents
        for i in 1..=2 {
            let request = AgentCreateRequest {
                name: format!("Agent {}", i),
                description: None,
                system_prompt: "Prompt".to_string(),
                model: "model".to_string(),
                allowed_tools: vec![],
            };
            service.create_agent(request).await.unwrap();
        }

        let agents = service.list_agents().await.unwrap();
        assert_eq!(agents.len(), 2);
    }

    #[tokio::test]
    async fn test_update_agent() {
        let pool = create_test_pool();
        let service = AgentService::new(pool);

        let request = AgentCreateRequest {
            name: "Original".to_string(),
            description: None,
            system_prompt: "Prompt".to_string(),
            model: "model".to_string(),
            allowed_tools: vec![],
        };

        let agent = service.create_agent(request).await.unwrap();

        let update = AgentUpdateRequest {
            name: Some("Updated".to_string()),
            description: Some("New description".to_string()),
            system_prompt: None,
            model: None,
            allowed_tools: None,
        };

        let updated = service.update_agent(&agent.id, update).await.unwrap();
        assert_eq!(updated.name, "Updated");
        assert_eq!(updated.description, Some("New description".to_string()));
    }

    #[tokio::test]
    async fn test_delete_agent() {
        let pool = create_test_pool();
        let service = AgentService::new(pool);

        let request = AgentCreateRequest {
            name: "To Delete".to_string(),
            description: None,
            system_prompt: "Prompt".to_string(),
            model: "model".to_string(),
            allowed_tools: vec![],
        };

        let agent = service.create_agent(request).await.unwrap();
        service.delete_agent(&agent.id).await.unwrap();

        let result = service.get_agent(&agent.id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_agent_runs() {
        let pool = create_test_pool();
        let service = AgentService::new(pool);

        let request = AgentCreateRequest {
            name: "Runner".to_string(),
            description: None,
            system_prompt: "Prompt".to_string(),
            model: "model".to_string(),
            allowed_tools: vec![],
        };

        let agent = service.create_agent(request).await.unwrap();
        let mut run = service.create_run(&agent.id, "Hello").await.unwrap();

        assert_eq!(run.status, RunStatus::Pending);

        run.complete("World".to_string(), 1000, 10, 5);
        service.update_run(&run).await.unwrap();

        let fetched = service.get_run(&run.id).await.unwrap().unwrap();
        assert_eq!(fetched.status, RunStatus::Completed);
    }

    #[tokio::test]
    async fn test_agent_stats() {
        let pool = create_test_pool();
        let service = AgentService::new(pool);

        let request = AgentCreateRequest {
            name: "Stats Agent".to_string(),
            description: None,
            system_prompt: "Prompt".to_string(),
            model: "model".to_string(),
            allowed_tools: vec![],
        };

        let agent = service.create_agent(request).await.unwrap();

        // Create some runs
        let mut run1 = service.create_run(&agent.id, "Input 1").await.unwrap();
        run1.complete("Output 1".to_string(), 1000, 10, 5);
        service.update_run(&run1).await.unwrap();

        let mut run2 = service.create_run(&agent.id, "Input 2").await.unwrap();
        run2.fail("Error".to_string(), 500);
        service.update_run(&run2).await.unwrap();

        let stats = service.get_agent_stats(&agent.id).await.unwrap();
        assert_eq!(stats.total_runs, 2);
        assert_eq!(stats.completed_runs, 1);
        assert_eq!(stats.failed_runs, 1);
        assert!((stats.success_rate - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_registry() {
        let mut registry = AgentRegistry::new();
        let agent = Agent::create("Test", "Prompt", "model");

        registry.insert(agent.clone());
        assert!(registry.contains(&agent.id));

        let fetched = registry.get(&agent.id).unwrap();
        assert_eq!(fetched.name, "Test");

        registry.remove(&agent.id);
        assert!(!registry.contains(&agent.id));
    }
}
