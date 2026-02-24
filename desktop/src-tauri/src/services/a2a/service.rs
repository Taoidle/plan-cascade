//! A2A Service
//!
//! Business logic for managing registered remote A2A agents.
//! Provides CRUD operations backed by SQLite persistence.

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::client::A2aClient;
use super::types::AgentCard;
use crate::utils::error::{AppError, AppResult};

/// Type alias for the database pool
type DbPool = Pool<SqliteConnectionManager>;

/// A registered remote A2A agent persisted in the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredRemoteAgent {
    /// Unique identifier
    pub id: String,
    /// The base URL used to discover this agent
    pub base_url: String,
    /// Agent name (from agent card)
    pub name: String,
    /// Agent description (from agent card)
    pub description: String,
    /// Agent capabilities (from agent card)
    pub capabilities: Vec<String>,
    /// The HTTP(S) endpoint URL for sending task requests
    pub endpoint: String,
    /// Protocol/agent version string
    pub version: String,
    /// Whether the agent requires authentication
    pub auth_required: bool,
    /// Supported input formats
    pub supported_inputs: Vec<String>,
    /// When registered
    pub created_at: Option<String>,
    /// When last updated
    pub updated_at: Option<String>,
}

/// Result of discovering a remote agent (before registration).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredAgent {
    /// The base URL used for discovery
    pub base_url: String,
    /// The discovered agent card
    pub agent_card: AgentCard,
}

/// Service for managing registered remote A2A agents.
#[derive(Clone)]
pub struct A2aService {
    pool: DbPool,
}

impl A2aService {
    /// Create a new A2A service with the given database pool.
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Get a database connection from the pool.
    fn get_conn(&self) -> AppResult<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))
    }

    /// Discover a remote agent at the given base URL.
    ///
    /// Fetches the agent card from `{base_url}/.well-known/agent.json` and
    /// validates it. Does NOT register the agent -- call `register` for that.
    pub async fn discover(&self, base_url: &str) -> AppResult<DiscoveredAgent> {
        let client = A2aClient::new()
            .map_err(|e| AppError::internal(format!("Failed to create A2A client: {}", e)))?;

        let agent_card = client
            .discover(base_url)
            .await
            .map_err(|e| AppError::internal(format!("A2A discovery failed: {}", e)))?;

        Ok(DiscoveredAgent {
            base_url: base_url.trim_end_matches('/').to_string(),
            agent_card,
        })
    }

    /// Register a remote agent in the database.
    ///
    /// If an agent with the same base_url already exists, it is updated.
    pub async fn register(
        &self,
        base_url: &str,
        card: &AgentCard,
    ) -> AppResult<RegisteredRemoteAgent> {
        let normalized_url = base_url.trim_end_matches('/').to_string();
        let id = uuid::Uuid::new_v4().to_string();
        let capabilities_json =
            serde_json::to_string(&card.capabilities).unwrap_or_else(|_| "[]".to_string());
        let supported_inputs_json =
            serde_json::to_string(&card.supported_inputs).unwrap_or_else(|_| "[]".to_string());

        // Scope the connection so it is returned to the pool before get_by_url
        {
            let conn = self.get_conn()?;

            // Upsert: insert or replace on base_url conflict
            conn.execute(
                "INSERT INTO remote_agents (id, base_url, name, description, capabilities, endpoint, version, auth_required, supported_inputs, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                 ON CONFLICT(base_url) DO UPDATE SET
                    name = excluded.name,
                    description = excluded.description,
                    capabilities = excluded.capabilities,
                    endpoint = excluded.endpoint,
                    version = excluded.version,
                    auth_required = excluded.auth_required,
                    supported_inputs = excluded.supported_inputs,
                    updated_at = CURRENT_TIMESTAMP",
                params![
                    id,
                    normalized_url,
                    card.name,
                    card.description,
                    capabilities_json,
                    card.endpoint,
                    card.version,
                    card.auth_required as i32,
                    supported_inputs_json,
                ],
            )?;
        }

        // Fetch the inserted/updated row (connection returned to pool above)
        self.get_by_url(&normalized_url).and_then(|opt| {
            opt.ok_or_else(|| AppError::database("Failed to fetch registered agent after insert"))
        })
    }

    /// List all registered remote agents.
    pub fn list(&self) -> AppResult<Vec<RegisteredRemoteAgent>> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, base_url, name, description, capabilities, endpoint, version, auth_required, supported_inputs, created_at, updated_at
             FROM remote_agents
             ORDER BY created_at DESC"
        )?;

        let agents = stmt
            .query_map([], |row| {
                let capabilities_str: String = row.get(4)?;
                let supported_inputs_str: String = row.get(8)?;
                Ok(RegisteredRemoteAgent {
                    id: row.get(0)?,
                    base_url: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    capabilities: serde_json::from_str(&capabilities_str).unwrap_or_default(),
                    endpoint: row.get(5)?,
                    version: row.get(6)?,
                    auth_required: row.get::<_, i32>(7)? != 0,
                    supported_inputs: serde_json::from_str(&supported_inputs_str)
                        .unwrap_or_default(),
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(agents)
    }

    /// Get a registered remote agent by base URL.
    pub fn get_by_url(&self, base_url: &str) -> AppResult<Option<RegisteredRemoteAgent>> {
        let conn = self.get_conn()?;
        let normalized = base_url.trim_end_matches('/');
        let mut stmt = conn.prepare(
            "SELECT id, base_url, name, description, capabilities, endpoint, version, auth_required, supported_inputs, created_at, updated_at
             FROM remote_agents
             WHERE base_url = ?1"
        )?;

        let mut rows = stmt.query_map(params![normalized], |row| {
            let capabilities_str: String = row.get(4)?;
            let supported_inputs_str: String = row.get(8)?;
            Ok(RegisteredRemoteAgent {
                id: row.get(0)?,
                base_url: row.get(1)?,
                name: row.get(2)?,
                description: row.get(3)?,
                capabilities: serde_json::from_str(&capabilities_str).unwrap_or_default(),
                endpoint: row.get(5)?,
                version: row.get(6)?,
                auth_required: row.get::<_, i32>(7)? != 0,
                supported_inputs: serde_json::from_str(&supported_inputs_str).unwrap_or_default(),
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?;

        match rows.next() {
            Some(Ok(agent)) => Ok(Some(agent)),
            Some(Err(e)) => Err(AppError::database(format!("Failed to fetch agent: {}", e))),
            None => Ok(None),
        }
    }

    /// Remove a registered remote agent by its ID.
    pub fn remove(&self, id: &str) -> AppResult<bool> {
        let conn = self.get_conn()?;
        let deleted = conn.execute("DELETE FROM remote_agents WHERE id = ?1", params![id])?;
        Ok(deleted > 0)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Database;

    fn test_pool() -> DbPool {
        let db = Database::new_in_memory().unwrap();
        db.pool().clone()
    }

    fn test_card() -> AgentCard {
        AgentCard {
            name: "test-agent".to_string(),
            description: "A test remote agent".to_string(),
            capabilities: vec!["code_review".to_string(), "testing".to_string()],
            endpoint: "https://agent.example.com/tasks".to_string(),
            version: "1.0.0".to_string(),
            auth_required: false,
            supported_inputs: vec!["text/plain".to_string()],
        }
    }

    #[tokio::test]
    async fn test_register_and_list() {
        let service = A2aService::new(test_pool());
        let card = test_card();

        let registered = service
            .register("https://agent.example.com", &card)
            .await
            .unwrap();
        assert_eq!(registered.name, "test-agent");
        assert_eq!(registered.base_url, "https://agent.example.com");
        assert_eq!(registered.capabilities, vec!["code_review", "testing"]);

        let agents = service.list().unwrap();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].name, "test-agent");
    }

    #[tokio::test]
    async fn test_register_upsert() {
        let service = A2aService::new(test_pool());

        let card1 = test_card();
        service
            .register("https://agent.example.com", &card1)
            .await
            .unwrap();

        // Register with same URL but different name
        let mut card2 = test_card();
        card2.name = "updated-agent".to_string();
        let updated = service
            .register("https://agent.example.com", &card2)
            .await
            .unwrap();
        assert_eq!(updated.name, "updated-agent");

        // Should still be only one agent
        let agents = service.list().unwrap();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].name, "updated-agent");
    }

    #[tokio::test]
    async fn test_remove() {
        let service = A2aService::new(test_pool());
        let card = test_card();

        let registered = service
            .register("https://agent.example.com", &card)
            .await
            .unwrap();
        assert!(service.remove(&registered.id).unwrap());

        let agents = service.list().unwrap();
        assert!(agents.is_empty());
    }

    #[test]
    fn test_remove_nonexistent() {
        let service = A2aService::new(test_pool());
        assert!(!service.remove("nonexistent-id").unwrap());
    }

    #[test]
    fn test_get_by_url_not_found() {
        let service = A2aService::new(test_pool());
        let result = service
            .get_by_url("https://nonexistent.example.com")
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_by_url_found() {
        let service = A2aService::new(test_pool());
        let card = test_card();

        service
            .register("https://agent.example.com/", &card)
            .await
            .unwrap();

        // Trailing slash should be normalized
        let result = service.get_by_url("https://agent.example.com").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "test-agent");
    }

    #[tokio::test]
    async fn test_multiple_agents() {
        let service = A2aService::new(test_pool());

        let card1 = AgentCard {
            name: "agent-one".to_string(),
            description: "First agent".to_string(),
            capabilities: vec!["review".to_string()],
            endpoint: "https://one.example.com/tasks".to_string(),
            version: "1.0".to_string(),
            auth_required: false,
            supported_inputs: vec![],
        };

        let card2 = AgentCard {
            name: "agent-two".to_string(),
            description: "Second agent".to_string(),
            capabilities: vec!["testing".to_string()],
            endpoint: "https://two.example.com/tasks".to_string(),
            version: "2.0".to_string(),
            auth_required: true,
            supported_inputs: vec!["application/json".to_string()],
        };

        service
            .register("https://one.example.com", &card1)
            .await
            .unwrap();
        service
            .register("https://two.example.com", &card2)
            .await
            .unwrap();

        let agents = service.list().unwrap();
        assert_eq!(agents.len(), 2);
    }

    #[test]
    fn test_list_empty() {
        let service = A2aService::new(test_pool());
        let agents = service.list().unwrap();
        assert!(agents.is_empty());
    }
}
