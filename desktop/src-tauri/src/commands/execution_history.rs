//! Execution History Commands
//!
//! SQLite-backed history storage used by the desktop UI.
//! This is the primary persistence path; localStorage is migration-only fallback.

use chrono::{Duration, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::models::response::CommandResponse;
use crate::state::AppState;

const DEFAULT_LIST_LIMIT: usize = 200;
const MAX_IMPORT_ITEMS: usize = 1000;
const DEFAULT_RETENTION_DAYS: i64 = 30;
const DEFAULT_MAX_HISTORY_ITEMS: usize = 200;
const DEFAULT_MAX_TOTAL_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryConversationLine {
    #[serde(rename = "type")]
    pub line_type: String,
    pub content: String,
    #[serde(default)]
    pub sub_agent_id: Option<String>,
    #[serde(default)]
    pub sub_agent_depth: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionHistoryRecord {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    pub task_description: String,
    #[serde(default)]
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub strategy: Option<String>,
    pub status: String,
    pub started_at: i64,
    #[serde(default)]
    pub completed_at: Option<i64>,
    #[serde(default)]
    pub duration: Option<i64>,
    #[serde(default)]
    pub completed_stories: Option<i64>,
    #[serde(default)]
    pub total_stories: Option<i64>,
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub conversation_content: Option<String>,
    #[serde(default)]
    pub conversation_lines: Option<Vec<HistoryConversationLine>>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub llm_backend: Option<String>,
    #[serde(default)]
    pub llm_provider: Option<String>,
    #[serde(default)]
    pub llm_model: Option<String>,
}

fn normalize_line_content(content: &str) -> String {
    content.trim().to_string()
}

fn serialize_lines(lines: Option<&Vec<HistoryConversationLine>>) -> Option<String> {
    let lines = lines?;
    if lines.is_empty() {
        return None;
    }
    serde_json::to_string(lines).ok()
}

fn parse_lines(json: Option<String>) -> Option<Vec<HistoryConversationLine>> {
    let raw = json?;
    if raw.trim().is_empty() {
        return None;
    }
    serde_json::from_str::<Vec<HistoryConversationLine>>(&raw).ok()
}

fn load_turn_lines(
    conn: &rusqlite::Connection,
    history_id: &str,
) -> Result<Option<Vec<HistoryConversationLine>>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT line_type, content, sub_agent_id, sub_agent_depth
         FROM execution_history_turns
         WHERE history_id = ?1
         ORDER BY seq ASC",
    )?;
    let rows = stmt
        .query_map(params![history_id], |row| {
            Ok(HistoryConversationLine {
                line_type: row.get(0)?,
                content: row.get(1)?,
                sub_agent_id: row.get(2)?,
                sub_agent_depth: row.get(3)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect::<Vec<_>>();

    if rows.is_empty() {
        Ok(None)
    } else {
        Ok(Some(rows))
    }
}

fn record_from_row(
    row: &rusqlite::Row<'_>,
    lines: Option<Vec<HistoryConversationLine>>,
) -> Result<ExecutionHistoryRecord, rusqlite::Error> {
    let status = row
        .get::<_, Option<String>>(5)?
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "completed".to_string());
    Ok(ExecutionHistoryRecord {
        id: row.get(0)?,
        title: row.get(1)?,
        task_description: row.get(2)?,
        workspace_path: row.get(3)?,
        strategy: row.get(4)?,
        status,
        started_at: row.get(6)?,
        completed_at: row.get(7)?,
        duration: row.get(8)?,
        completed_stories: row.get(9)?,
        total_stories: row.get(10)?,
        success: row.get::<_, i64>(11)? != 0,
        error: row.get(12)?,
        conversation_content: row.get(13)?,
        conversation_lines: lines,
        session_id: row.get(15)?,
        llm_backend: row.get(16)?,
        llm_provider: row.get(17)?,
        llm_model: row.get(18)?,
    })
}

fn upsert_record(
    conn: &rusqlite::Connection,
    item: &ExecutionHistoryRecord,
) -> Result<(), rusqlite::Error> {
    let normalized_status = if item.status.trim().is_empty() {
        "completed".to_string()
    } else {
        item.status.clone()
    };
    let conversation_lines_json = serialize_lines(item.conversation_lines.as_ref());
    conn.execute(
        "INSERT INTO execution_history_sessions (
            id, title, task_description, workspace_path, strategy, status,
            started_at, completed_at, duration_ms, completed_stories, total_stories,
            success, error_message, conversation_content, conversation_lines_json,
            session_id, llm_backend, llm_provider, llm_model, updated_at
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6,
            ?7, ?8, ?9, ?10, ?11,
            ?12, ?13, ?14, ?15,
            ?16, ?17, ?18, ?19, datetime('now')
         )
         ON CONFLICT(id) DO UPDATE SET
            title = excluded.title,
            task_description = excluded.task_description,
            workspace_path = excluded.workspace_path,
            strategy = excluded.strategy,
            status = excluded.status,
            started_at = excluded.started_at,
            completed_at = excluded.completed_at,
            duration_ms = excluded.duration_ms,
            completed_stories = excluded.completed_stories,
            total_stories = excluded.total_stories,
            success = excluded.success,
            error_message = excluded.error_message,
            conversation_content = excluded.conversation_content,
            conversation_lines_json = excluded.conversation_lines_json,
            session_id = excluded.session_id,
            llm_backend = excluded.llm_backend,
            llm_provider = excluded.llm_provider,
            llm_model = excluded.llm_model,
            updated_at = datetime('now')",
        params![
            item.id,
            item.title,
            item.task_description,
            item.workspace_path,
            item.strategy,
            normalized_status,
            item.started_at,
            item.completed_at,
            item.duration,
            item.completed_stories,
            item.total_stories,
            if item.success { 1_i64 } else { 0_i64 },
            item.error,
            item.conversation_content,
            conversation_lines_json,
            item.session_id,
            item.llm_backend,
            item.llm_provider,
            item.llm_model
        ],
    )?;

    conn.execute(
        "DELETE FROM execution_history_turns WHERE history_id = ?1",
        params![item.id],
    )?;
    if let Some(lines) = item.conversation_lines.as_ref() {
        for (seq, line) in lines.iter().enumerate() {
            conn.execute(
                "INSERT INTO execution_history_turns (
                    history_id, seq, line_type, content, sub_agent_id, sub_agent_depth
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    item.id,
                    seq as i64,
                    line.line_type,
                    normalize_line_content(&line.content),
                    line.sub_agent_id,
                    line.sub_agent_depth
                ],
            )?;
        }
    }

    Ok(())
}

fn prune_history(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    let cutoff_ms = (Utc::now() - Duration::days(DEFAULT_RETENTION_DAYS))
        .timestamp_millis()
        .max(0);
    conn.execute(
        "DELETE FROM execution_history_sessions WHERE started_at < ?1",
        params![cutoff_ms],
    )?;

    conn.execute(
        "DELETE FROM execution_history_sessions
         WHERE id IN (
            SELECT id FROM execution_history_sessions
            ORDER BY started_at DESC
            LIMIT -1 OFFSET ?1
         )",
        params![DEFAULT_MAX_HISTORY_ITEMS as i64],
    )?;

    let mut stmt = conn.prepare(
        "SELECT id,
                COALESCE(LENGTH(task_description), 0)
              + COALESCE(LENGTH(conversation_content), 0)
              + COALESCE(LENGTH(conversation_lines_json), 0)
              + COALESCE(LENGTH(error_message), 0) AS bytes
         FROM execution_history_sessions
         ORDER BY started_at DESC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1).unwrap_or(0)))
        })?
        .filter_map(|r| r.ok())
        .collect::<Vec<_>>();

    let mut running = 0usize;
    let mut overflow_ids = Vec::new();
    for (id, bytes) in rows {
        running = running.saturating_add(bytes.max(0) as usize);
        if running > DEFAULT_MAX_TOTAL_BYTES {
            overflow_ids.push(id);
        }
    }
    for id in overflow_ids {
        conn.execute(
            "DELETE FROM execution_history_sessions WHERE id = ?1",
            params![id],
        )?;
    }

    Ok(())
}

#[tauri::command]
pub async fn list_execution_history(
    limit: Option<usize>,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<ExecutionHistoryRecord>>, String> {
    let lim = limit.unwrap_or(DEFAULT_LIST_LIMIT).clamp(1, 1000);
    let rows = match app_state
        .with_database(|db| {
            let conn = db.get_connection()?;
            let mut stmt = conn.prepare(
                "SELECT
                    id, title, task_description, workspace_path, strategy, status,
                    started_at, completed_at, duration_ms, completed_stories, total_stories,
                    success, error_message, conversation_content, conversation_lines_json,
                    session_id, llm_backend, llm_provider, llm_model
                 FROM execution_history_sessions
                 ORDER BY started_at DESC
                 LIMIT ?1",
            )?;
            let mut items = stmt
                .query_map(params![lim as i64], |row| {
                    let lines_json: Option<String> = row.get(14)?;
                    let lines = parse_lines(lines_json);
                    record_from_row(row, lines)
                })?
                .filter_map(|r| r.ok())
                .collect::<Vec<_>>();
            drop(stmt);
            for item in &mut items {
                if item.conversation_lines.is_none() {
                    item.conversation_lines = load_turn_lines(&conn, &item.id).ok().flatten();
                }
            }
            Ok(items)
        })
        .await
    {
        Ok(items) => items,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };
    Ok(CommandResponse::ok(rows))
}

#[tauri::command]
pub async fn upsert_execution_history(
    item: ExecutionHistoryRecord,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<ExecutionHistoryRecord>, String> {
    if item.id.trim().is_empty() {
        return Ok(CommandResponse::err("history id is required"));
    }
    if item.task_description.trim().is_empty() {
        return Ok(CommandResponse::err("taskDescription is required"));
    }
    if item.status.trim().is_empty() {
        return Ok(CommandResponse::err("status is required"));
    }

    let saved = item.clone();
    let persisted = app_state
        .with_database(|db| {
            let conn = db.get_connection()?;
            upsert_record(&conn, &saved)?;
            prune_history(&conn)?;
            Ok(())
        })
        .await;

    match persisted {
        Ok(_) => Ok(CommandResponse::ok(item)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub async fn import_execution_history(
    items: Vec<ExecutionHistoryRecord>,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<usize>, String> {
    if items.is_empty() {
        return Ok(CommandResponse::ok(0));
    }

    let capped = items.into_iter().take(MAX_IMPORT_ITEMS).collect::<Vec<_>>();
    let count = capped.len();
    let persisted = app_state
        .with_database(|db| {
            let conn = db.get_connection()?;
            for item in &capped {
                if item.id.trim().is_empty() || item.task_description.trim().is_empty() {
                    continue;
                }
                upsert_record(&conn, item)?;
            }
            prune_history(&conn)?;
            Ok(())
        })
        .await;

    match persisted {
        Ok(_) => Ok(CommandResponse::ok(count)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub async fn rename_execution_history(
    history_id: String,
    title: Option<String>,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<bool>, String> {
    let history_id = history_id.trim().to_string();
    if history_id.is_empty() {
        return Ok(CommandResponse::err("history id is required"));
    }

    let update = app_state
        .with_database(|db| {
            let conn = db.get_connection()?;
            let rows = conn.execute(
                "UPDATE execution_history_sessions
                 SET title = ?2, updated_at = datetime('now')
                 WHERE id = ?1",
                params![history_id, title],
            )?;
            Ok(rows > 0)
        })
        .await;

    match update {
        Ok(updated) => Ok(CommandResponse::ok(updated)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub async fn delete_execution_history(
    history_id: String,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<bool>, String> {
    let history_id = history_id.trim().to_string();
    if history_id.is_empty() {
        return Ok(CommandResponse::err("history id is required"));
    }

    let deleted = app_state
        .with_database(|db| {
            let conn = db.get_connection()?;
            let rows = conn.execute(
                "DELETE FROM execution_history_sessions WHERE id = ?1",
                params![history_id],
            )?;
            Ok(rows > 0)
        })
        .await;

    match deleted {
        Ok(v) => Ok(CommandResponse::ok(v)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub async fn clear_execution_history(
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<bool>, String> {
    let cleared = app_state
        .with_database(|db| {
            let conn = db.get_connection()?;
            conn.execute("DELETE FROM execution_history_sessions", [])?;
            Ok(true)
        })
        .await;

    match cleared {
        Ok(v) => Ok(CommandResponse::ok(v)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}
