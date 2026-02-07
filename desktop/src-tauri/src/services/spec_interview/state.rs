//! Interview State Persistence
//!
//! Persists all interview state to SQLite and supports resume after restart.
//! Uses the existing Database connection pool pattern (ADR-F002).

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::storage::database::DbPool;
use crate::utils::error::{AppError, AppResult};

/// Persisted interview state stored in SQLite
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedInterviewState {
    /// Unique interview ID
    pub id: String,
    /// Status: "in_progress", "finalized"
    pub status: String,
    /// Current interview phase
    pub phase: String,
    /// Flow level: "quick", "standard", "full"
    pub flow_level: String,
    /// Whether first-principles mode is enabled
    pub first_principles: bool,
    /// Maximum number of questions (soft cap)
    pub max_questions: i32,
    /// Number of questions answered so far
    pub question_cursor: i32,
    /// Initial project description
    pub description: String,
    /// Optional project path
    pub project_path: Option<String>,
    /// JSON-serialized spec data being built up
    pub spec_data: String,
    /// Created timestamp (ISO-8601)
    pub created_at: String,
    /// Last updated timestamp (ISO-8601)
    pub updated_at: String,
}

/// A single turn in the interview conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterviewTurn {
    /// Unique turn ID
    pub id: String,
    /// Parent interview ID
    pub interview_id: String,
    /// Turn number (1-based)
    pub turn_number: i32,
    /// Phase during this turn
    pub phase: String,
    /// The question that was asked
    pub question: String,
    /// The user's answer
    pub answer: String,
    /// The spec field this answer maps to
    pub field_name: String,
    /// Created timestamp
    pub created_at: String,
}

/// Manages interview state persistence in SQLite
#[derive(Clone)]
pub struct InterviewStateManager {
    pool: DbPool,
}

impl InterviewStateManager {
    /// Create a new state manager with the given connection pool
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Initialize the interview tables (called during database setup)
    pub fn init_schema(&self) -> AppResult<()> {
        let conn = self.pool.get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS interviews (
                id TEXT PRIMARY KEY,
                status TEXT NOT NULL DEFAULT 'in_progress',
                phase TEXT NOT NULL DEFAULT 'overview',
                flow_level TEXT NOT NULL DEFAULT 'standard',
                first_principles INTEGER NOT NULL DEFAULT 0,
                max_questions INTEGER NOT NULL DEFAULT 18,
                question_cursor INTEGER NOT NULL DEFAULT 0,
                description TEXT NOT NULL DEFAULT '',
                project_path TEXT,
                spec_data TEXT NOT NULL DEFAULT '{}',
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS interview_turns (
                id TEXT PRIMARY KEY,
                interview_id TEXT NOT NULL,
                turn_number INTEGER NOT NULL,
                phase TEXT NOT NULL,
                question TEXT NOT NULL,
                answer TEXT NOT NULL DEFAULT '',
                field_name TEXT NOT NULL DEFAULT '',
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (interview_id) REFERENCES interviews(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Indexes for efficient queries
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_interview_turns_interview_id
             ON interview_turns(interview_id)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_interview_turns_turn_number
             ON interview_turns(interview_id, turn_number)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_interviews_status
             ON interviews(status)",
            [],
        )?;

        Ok(())
    }

    /// Create a new interview record
    pub fn create_interview(&self, state: &PersistedInterviewState) -> AppResult<()> {
        let conn = self.pool.get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        conn.execute(
            "INSERT INTO interviews (id, status, phase, flow_level, first_principles, max_questions,
             question_cursor, description, project_path, spec_data, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                state.id,
                state.status,
                state.phase,
                state.flow_level,
                state.first_principles as i32,
                state.max_questions,
                state.question_cursor,
                state.description,
                state.project_path,
                state.spec_data,
                state.created_at,
                state.updated_at,
            ],
        )?;

        Ok(())
    }

    /// Get an interview by ID
    pub fn get_interview(&self, id: &str) -> AppResult<Option<PersistedInterviewState>> {
        let conn = self.pool.get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let result = conn.query_row(
            "SELECT id, status, phase, flow_level, first_principles, max_questions,
             question_cursor, description, project_path, spec_data, created_at, updated_at
             FROM interviews WHERE id = ?1",
            params![id],
            |row| {
                Ok(PersistedInterviewState {
                    id: row.get(0)?,
                    status: row.get(1)?,
                    phase: row.get(2)?,
                    flow_level: row.get(3)?,
                    first_principles: {
                        let v: i32 = row.get(4)?;
                        v != 0
                    },
                    max_questions: row.get(5)?,
                    question_cursor: row.get(6)?,
                    description: row.get(7)?,
                    project_path: row.get(8)?,
                    spec_data: row.get(9)?,
                    created_at: row.get(10)?,
                    updated_at: row.get(11)?,
                })
            },
        );

        match result {
            Ok(state) => Ok(Some(state)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::database(e.to_string())),
        }
    }

    /// Update an existing interview
    pub fn update_interview(&self, state: &PersistedInterviewState) -> AppResult<()> {
        let conn = self.pool.get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        conn.execute(
            "UPDATE interviews SET status = ?2, phase = ?3, flow_level = ?4,
             first_principles = ?5, max_questions = ?6, question_cursor = ?7,
             description = ?8, project_path = ?9, spec_data = ?10, updated_at = ?11
             WHERE id = ?1",
            params![
                state.id,
                state.status,
                state.phase,
                state.flow_level,
                state.first_principles as i32,
                state.max_questions,
                state.question_cursor,
                state.description,
                state.project_path,
                state.spec_data,
                state.updated_at,
            ],
        )?;

        Ok(())
    }

    /// List all interviews, optionally filtered by status
    pub fn list_interviews(&self, status_filter: Option<&str>) -> AppResult<Vec<PersistedInterviewState>> {
        let conn = self.pool.get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let (sql, filter_params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(status) = status_filter {
            (
                "SELECT id, status, phase, flow_level, first_principles, max_questions,
                 question_cursor, description, project_path, spec_data, created_at, updated_at
                 FROM interviews WHERE status = ?1 ORDER BY updated_at DESC".to_string(),
                vec![Box::new(status.to_string()) as Box<dyn rusqlite::types::ToSql>],
            )
        } else {
            (
                "SELECT id, status, phase, flow_level, first_principles, max_questions,
                 question_cursor, description, project_path, spec_data, created_at, updated_at
                 FROM interviews ORDER BY updated_at DESC".to_string(),
                vec![],
            )
        };

        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> = filter_params.iter().map(|p| p.as_ref()).collect();
        let interviews = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(PersistedInterviewState {
                id: row.get(0)?,
                status: row.get(1)?,
                phase: row.get(2)?,
                flow_level: row.get(3)?,
                first_principles: {
                    let v: i32 = row.get(4)?;
                    v != 0
                },
                max_questions: row.get(5)?,
                question_cursor: row.get(6)?,
                description: row.get(7)?,
                project_path: row.get(8)?,
                spec_data: row.get(9)?,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

        Ok(interviews)
    }

    /// Delete an interview and all its turns
    pub fn delete_interview(&self, id: &str) -> AppResult<()> {
        let conn = self.pool.get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        conn.execute("DELETE FROM interview_turns WHERE interview_id = ?1", params![id])?;
        conn.execute("DELETE FROM interviews WHERE id = ?1", params![id])?;

        Ok(())
    }

    // ========================================================================
    // Turn operations
    // ========================================================================

    /// Add a new turn to an interview
    pub fn add_turn(&self, turn: &InterviewTurn) -> AppResult<()> {
        let conn = self.pool.get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        conn.execute(
            "INSERT INTO interview_turns (id, interview_id, turn_number, phase, question, answer, field_name, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                turn.id,
                turn.interview_id,
                turn.turn_number,
                turn.phase,
                turn.question,
                turn.answer,
                turn.field_name,
                turn.created_at,
            ],
        )?;

        Ok(())
    }

    /// Get all turns for an interview, ordered by turn number
    pub fn get_turns(&self, interview_id: &str) -> AppResult<Vec<InterviewTurn>> {
        let conn = self.pool.get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let mut stmt = conn.prepare(
            "SELECT id, interview_id, turn_number, phase, question, answer, field_name, created_at
             FROM interview_turns WHERE interview_id = ?1 ORDER BY turn_number ASC",
        )?;

        let turns = stmt.query_map(params![interview_id], |row| {
            Ok(InterviewTurn {
                id: row.get(0)?,
                interview_id: row.get(1)?,
                turn_number: row.get(2)?,
                phase: row.get(3)?,
                question: row.get(4)?,
                answer: row.get(5)?,
                field_name: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

        Ok(turns)
    }

    /// Count turns for an interview
    pub fn count_turns(&self, interview_id: &str) -> AppResult<i32> {
        let conn = self.pool.get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM interview_turns WHERE interview_id = ?1",
            params![interview_id],
            |row| row.get(0),
        )?;

        Ok(count)
    }
}

impl std::fmt::Debug for InterviewStateManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InterviewStateManager").finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use r2d2::Pool;
    use r2d2_sqlite::SqliteConnectionManager;

    fn create_test_pool() -> DbPool {
        let manager = SqliteConnectionManager::memory();
        Pool::builder().max_size(1).build(manager).unwrap()
    }

    #[test]
    fn test_init_schema() {
        let pool = create_test_pool();
        let mgr = InterviewStateManager::new(pool);
        mgr.init_schema().unwrap();
    }

    #[test]
    fn test_create_and_get_interview() {
        let pool = create_test_pool();
        let mgr = InterviewStateManager::new(pool);
        mgr.init_schema().unwrap();

        let state = PersistedInterviewState {
            id: "test-001".to_string(),
            status: "in_progress".to_string(),
            phase: "overview".to_string(),
            flow_level: "standard".to_string(),
            first_principles: false,
            max_questions: 18,
            question_cursor: 0,
            description: "Test project".to_string(),
            project_path: Some("/tmp/test".to_string()),
            spec_data: "{}".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };

        mgr.create_interview(&state).unwrap();

        let retrieved = mgr.get_interview("test-001").unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, "test-001");
        assert_eq!(retrieved.description, "Test project");
        assert!(!retrieved.first_principles);
    }

    #[test]
    fn test_update_interview() {
        let pool = create_test_pool();
        let mgr = InterviewStateManager::new(pool);
        mgr.init_schema().unwrap();

        let mut state = PersistedInterviewState {
            id: "test-002".to_string(),
            status: "in_progress".to_string(),
            phase: "overview".to_string(),
            flow_level: "standard".to_string(),
            first_principles: false,
            max_questions: 18,
            question_cursor: 0,
            description: "Test".to_string(),
            project_path: None,
            spec_data: "{}".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };

        mgr.create_interview(&state).unwrap();

        state.phase = "scope".to_string();
        state.question_cursor = 5;
        mgr.update_interview(&state).unwrap();

        let retrieved = mgr.get_interview("test-002").unwrap().unwrap();
        assert_eq!(retrieved.phase, "scope");
        assert_eq!(retrieved.question_cursor, 5);
    }

    #[test]
    fn test_add_and_get_turns() {
        let pool = create_test_pool();
        let mgr = InterviewStateManager::new(pool);
        mgr.init_schema().unwrap();

        let state = PersistedInterviewState {
            id: "test-003".to_string(),
            status: "in_progress".to_string(),
            phase: "overview".to_string(),
            flow_level: "standard".to_string(),
            first_principles: false,
            max_questions: 18,
            question_cursor: 0,
            description: "Test".to_string(),
            project_path: None,
            spec_data: "{}".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };
        mgr.create_interview(&state).unwrap();

        let turn1 = InterviewTurn {
            id: "turn-001".to_string(),
            interview_id: "test-003".to_string(),
            turn_number: 1,
            phase: "overview".to_string(),
            question: "What is the title?".to_string(),
            answer: "My Project".to_string(),
            field_name: "title".to_string(),
            created_at: "2024-01-01T00:00:01Z".to_string(),
        };

        let turn2 = InterviewTurn {
            id: "turn-002".to_string(),
            interview_id: "test-003".to_string(),
            turn_number: 2,
            phase: "overview".to_string(),
            question: "What is the goal?".to_string(),
            answer: "Build something great".to_string(),
            field_name: "goal".to_string(),
            created_at: "2024-01-01T00:00:02Z".to_string(),
        };

        mgr.add_turn(&turn1).unwrap();
        mgr.add_turn(&turn2).unwrap();

        let turns = mgr.get_turns("test-003").unwrap();
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].question, "What is the title?");
        assert_eq!(turns[1].answer, "Build something great");

        let count = mgr.count_turns("test-003").unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_list_interviews() {
        let pool = create_test_pool();
        let mgr = InterviewStateManager::new(pool);
        mgr.init_schema().unwrap();

        for i in 1..=3 {
            let status = if i == 3 { "finalized" } else { "in_progress" };
            let state = PersistedInterviewState {
                id: format!("test-{:03}", i),
                status: status.to_string(),
                phase: "overview".to_string(),
                flow_level: "standard".to_string(),
                first_principles: false,
                max_questions: 18,
                question_cursor: 0,
                description: format!("Test {}", i),
                project_path: None,
                spec_data: "{}".to_string(),
                created_at: format!("2024-01-01T00:00:{:02}Z", i),
                updated_at: format!("2024-01-01T00:00:{:02}Z", i),
            };
            mgr.create_interview(&state).unwrap();
        }

        let all = mgr.list_interviews(None).unwrap();
        assert_eq!(all.len(), 3);

        let in_progress = mgr.list_interviews(Some("in_progress")).unwrap();
        assert_eq!(in_progress.len(), 2);

        let finalized = mgr.list_interviews(Some("finalized")).unwrap();
        assert_eq!(finalized.len(), 1);
    }

    #[test]
    fn test_delete_interview() {
        let pool = create_test_pool();
        let mgr = InterviewStateManager::new(pool);
        mgr.init_schema().unwrap();

        let state = PersistedInterviewState {
            id: "test-del".to_string(),
            status: "in_progress".to_string(),
            phase: "overview".to_string(),
            flow_level: "standard".to_string(),
            first_principles: false,
            max_questions: 18,
            question_cursor: 0,
            description: "Test".to_string(),
            project_path: None,
            spec_data: "{}".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };
        mgr.create_interview(&state).unwrap();

        let turn = InterviewTurn {
            id: "turn-del".to_string(),
            interview_id: "test-del".to_string(),
            turn_number: 1,
            phase: "overview".to_string(),
            question: "Q?".to_string(),
            answer: "A.".to_string(),
            field_name: "title".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        mgr.add_turn(&turn).unwrap();

        mgr.delete_interview("test-del").unwrap();

        assert!(mgr.get_interview("test-del").unwrap().is_none());
        assert_eq!(mgr.get_turns("test-del").unwrap().len(), 0);
    }
}
