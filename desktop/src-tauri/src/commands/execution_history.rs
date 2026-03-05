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
const CARD_LINE_PREFIX: &str = "[Card] ";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryConversationLine {
    #[serde(rename = "type")]
    pub line_type: String,
    pub content: String,
    #[serde(default)]
    pub card_payload: Option<serde_json::Value>,
    #[serde(default)]
    pub sub_agent_id: Option<String>,
    #[serde(default)]
    pub sub_agent_depth: Option<i32>,
    #[serde(default)]
    pub turn_id: Option<i64>,
    #[serde(default)]
    pub turn_boundary: Option<String>,
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

fn normalize_line_type(line_type: &str) -> String {
    line_type.trim().to_lowercase()
}

fn parse_card_payload_json(raw: Option<String>) -> Option<serde_json::Value> {
    let text = raw?;
    if text.trim().is_empty() {
        return None;
    }
    serde_json::from_str::<serde_json::Value>(&text).ok()
}

fn sanitize_conversation_content(content: Option<String>) -> (Option<String>, bool) {
    let Some(content) = content else {
        return (None, false);
    };
    if content.trim().is_empty() {
        return (None, !content.is_empty());
    }
    let filtered = content
        .lines()
        .filter(|line| !line.starts_with(CARD_LINE_PREFIX))
        .collect::<Vec<_>>()
        .join("\n");
    if filtered == content {
        (Some(content), false)
    } else if filtered.is_empty() {
        (None, true)
    } else {
        (Some(filtered), true)
    }
}

fn sanitize_lines(
    lines: Option<Vec<HistoryConversationLine>>,
) -> (Option<Vec<HistoryConversationLine>>, bool) {
    let Some(lines) = lines else {
        return (None, false);
    };
    if lines.is_empty() {
        return (None, false);
    }

    let original_len = lines.len();
    let mut sanitized = Vec::with_capacity(lines.len());
    let mut changed = false;

    for mut line in lines {
        let normalized_type = normalize_line_type(&line.line_type);
        if normalized_type != line.line_type {
            changed = true;
            line.line_type = normalized_type.clone();
        }

        let normalized_content = normalize_line_content(&line.content);
        if normalized_content != line.content {
            changed = true;
            line.content = normalized_content;
        }

        if normalized_type == "card" {
            if line.card_payload.is_none() {
                changed = true;
                continue;
            }
            if line.content.is_empty() {
                if let Some(payload) = line.card_payload.as_ref() {
                    if let Ok(serialized) = serde_json::to_string(payload) {
                        line.content = serialized;
                        changed = true;
                    }
                }
            }
        } else if line.card_payload.take().is_some() {
            changed = true;
        }

        sanitized.push(line);
    }

    if sanitized.len() != original_len {
        changed = true;
    }

    if sanitized.is_empty() {
        (None, changed)
    } else {
        (Some(sanitized), changed)
    }
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
        "SELECT line_type, content, card_payload_json, sub_agent_id, sub_agent_depth, turn_id, turn_boundary
         FROM execution_history_turns
         WHERE history_id = ?1
         ORDER BY seq ASC",
    )?;
    let rows = stmt
        .query_map(params![history_id], |row| {
            Ok(HistoryConversationLine {
                line_type: row.get(0)?,
                content: row.get(1)?,
                card_payload: parse_card_payload_json(row.get(2)?),
                sub_agent_id: row.get(3)?,
                sub_agent_depth: row.get(4)?,
                turn_id: row.get(5)?,
                turn_boundary: row.get(6)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect::<Vec<_>>();

    Ok(sanitize_lines(if rows.is_empty() { None } else { Some(rows) }).0)
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
    let (sanitized_lines, _) = sanitize_lines(item.conversation_lines.clone());
    let (sanitized_content, _) = sanitize_conversation_content(item.conversation_content.clone());
    let normalized_status = if item.status.trim().is_empty() {
        "completed".to_string()
    } else {
        item.status.clone()
    };
    let conversation_lines_json = serialize_lines(sanitized_lines.as_ref());
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
            sanitized_content,
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
    if let Some(lines) = sanitized_lines.as_ref() {
        for (seq, line) in lines.iter().enumerate() {
            let card_payload_json = line
                .card_payload
                .as_ref()
                .and_then(|payload| serde_json::to_string(payload).ok());
            conn.execute(
                "INSERT INTO execution_history_turns (
                    history_id, seq, line_type, content, card_payload_json, sub_agent_id, sub_agent_depth, turn_id, turn_boundary
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    item.id,
                    seq as i64,
                    line.line_type,
                    normalize_line_content(&line.content),
                    card_payload_json,
                    line.sub_agent_id,
                    line.sub_agent_depth,
                    line.turn_id,
                    line.turn_boundary
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
                let (sanitized_lines, lines_changed) =
                    sanitize_lines(item.conversation_lines.take());
                item.conversation_lines = sanitized_lines;
                let (sanitized_content, content_changed) =
                    sanitize_conversation_content(item.conversation_content.take());
                item.conversation_content = sanitized_content;
                if lines_changed || content_changed {
                    upsert_record(&conn, item)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn create_test_tables(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE execution_history_sessions (
                id TEXT PRIMARY KEY,
                title TEXT,
                task_description TEXT NOT NULL,
                workspace_path TEXT,
                strategy TEXT,
                status TEXT,
                started_at INTEGER NOT NULL,
                completed_at INTEGER,
                duration_ms INTEGER,
                completed_stories INTEGER NOT NULL DEFAULT 0,
                total_stories INTEGER NOT NULL DEFAULT 0,
                success INTEGER NOT NULL DEFAULT 0,
                error_message TEXT,
                conversation_content TEXT,
                conversation_lines_json TEXT,
                session_id TEXT,
                llm_backend TEXT,
                llm_provider TEXT,
                llm_model TEXT,
                updated_at TEXT
            );
            CREATE TABLE execution_history_turns (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                history_id TEXT NOT NULL,
                seq INTEGER NOT NULL,
                line_type TEXT NOT NULL,
                content TEXT NOT NULL,
                card_payload_json TEXT,
                sub_agent_id TEXT,
                sub_agent_depth INTEGER,
                turn_id INTEGER,
                turn_boundary TEXT,
                UNIQUE(history_id, seq)
            );",
        )
        .expect("create test tables");
    }

    #[test]
    fn sanitize_lines_drops_legacy_card_without_payload() {
        let lines = vec![
            HistoryConversationLine {
                line_type: "card".to_string(),
                content: "{\"cardType\":\"workflow_info\"}".to_string(),
                card_payload: None,
                sub_agent_id: None,
                sub_agent_depth: None,
                turn_id: None,
                turn_boundary: None,
            },
            HistoryConversationLine {
                line_type: "text".to_string(),
                content: "hello".to_string(),
                card_payload: Some(serde_json::json!({"unexpected": true})),
                sub_agent_id: None,
                sub_agent_depth: None,
                turn_id: None,
                turn_boundary: None,
            },
        ];

        let (sanitized, changed) = sanitize_lines(Some(lines));
        let sanitized = sanitized.expect("sanitized lines");

        assert!(changed);
        assert_eq!(sanitized.len(), 1);
        assert_eq!(sanitized[0].line_type, "text");
        assert!(sanitized[0].card_payload.is_none());
    }

    #[test]
    fn upsert_record_persists_card_payload_and_drops_legacy_card() {
        let conn = Connection::open_in_memory().expect("open in-memory sqlite");
        create_test_tables(&conn);

        let record = ExecutionHistoryRecord {
            id: "hist-1".to_string(),
            title: None,
            task_description: "Task".to_string(),
            workspace_path: None,
            strategy: None,
            status: "completed".to_string(),
            started_at: 1,
            completed_at: Some(2),
            duration: Some(1),
            completed_stories: Some(1),
            total_stories: Some(1),
            success: true,
            error: None,
            conversation_content: Some(
                "[User] hello\n[Card] {\"cardType\":\"workflow_info\"}\n[Assistant] world"
                    .to_string(),
            ),
            conversation_lines: Some(vec![
                HistoryConversationLine {
                    line_type: "card".to_string(),
                    content: "{\"cardType\":\"workflow_info\"}".to_string(),
                    card_payload: None,
                    sub_agent_id: None,
                    sub_agent_depth: None,
                    turn_id: None,
                    turn_boundary: None,
                },
                HistoryConversationLine {
                    line_type: "card".to_string(),
                    content: "".to_string(),
                    card_payload: Some(serde_json::json!({
                        "cardType": "workflow_info",
                        "cardId": "card-1",
                        "interactive": false,
                        "data": { "message": "ok" }
                    })),
                    sub_agent_id: None,
                    sub_agent_depth: None,
                    turn_id: Some(2),
                    turn_boundary: Some("assistant".to_string()),
                },
            ]),
            session_id: Some("standalone:s-1".to_string()),
            llm_backend: Some("openai".to_string()),
            llm_provider: Some("openai".to_string()),
            llm_model: Some("gpt-4o".to_string()),
        };

        upsert_record(&conn, &record).expect("upsert history record");

        let mut stmt = conn
            .prepare(
                "SELECT line_type, card_payload_json, turn_id, turn_boundary
                 FROM execution_history_turns
                 WHERE history_id = ?1
                 ORDER BY seq ASC",
            )
            .expect("prepare query");
        let rows = stmt
            .query_map(params!["hist-1"], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })
            .expect("query rows")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect rows");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "card");
        assert!(rows[0].1.is_some());
        assert_eq!(rows[0].2, Some(2));
        assert_eq!(rows[0].3.as_deref(), Some("assistant"));

        let conversation_content: Option<String> = conn
            .query_row(
                "SELECT conversation_content FROM execution_history_sessions WHERE id = ?1",
                params!["hist-1"],
                |row| row.get(0),
            )
            .expect("query conversation_content");
        assert_eq!(
            conversation_content.as_deref(),
            Some("[User] hello\n[Assistant] world")
        );
    }

    #[test]
    fn load_turn_lines_parses_card_payload_json() {
        let conn = Connection::open_in_memory().expect("open in-memory sqlite");
        create_test_tables(&conn);

        conn.execute(
            "INSERT INTO execution_history_turns (
                history_id, seq, line_type, content, card_payload_json, sub_agent_id, sub_agent_depth, turn_id, turn_boundary
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                "hist-2",
                0_i64,
                "card",
                "{\"cardType\":\"workflow_info\"}",
                "{\"cardType\":\"workflow_info\",\"cardId\":\"card-2\",\"interactive\":false,\"data\":{\"message\":\"ok\"}}",
                Option::<String>::None,
                Option::<i32>::None,
                Option::<i64>::None,
                Option::<String>::None
            ],
        )
        .expect("insert turn");

        let lines = load_turn_lines(&conn, "hist-2")
            .expect("load lines")
            .expect("some lines");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].line_type, "card");
        assert!(lines[0].card_payload.is_some());
    }
}
