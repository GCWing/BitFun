//! Physical schema owner for the durable Agent coordination database.
//!
//! Runtime coordination and offline legacy import both open this database, but
//! neither should carry a private copy of its versioning and repair rules.

use crate::storage_error::{StorageError as OpenBitFunError, StorageResult as OpenBitFunResult};
use rusqlite::Connection;

pub const COORDINATION_SCHEMA_VERSION: i64 = 2;

pub fn initialize_coordination_schema(connection: &Connection) -> OpenBitFunResult<()> {
    let version = connection
        .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
        .map_err(db_error)?;
    if version > COORDINATION_SCHEMA_VERSION {
        return Err(OpenBitFunError::service(format!(
            "Agent coordination database schema {version} is newer than supported schema {COORDINATION_SCHEMA_VERSION}"
        )));
    }
    if version == 0 {
        connection
            .execute_batch(
                r#"
CREATE TABLE coordination_sessions (
    parent_session_id TEXT PRIMARY KEY,
    next_auto_agent_seq INTEGER NOT NULL DEFAULT 1,
    updated_at_ms INTEGER NOT NULL
);

CREATE TABLE agents (
    agent_pk INTEGER PRIMARY KEY AUTOINCREMENT,
    parent_session_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    child_session_id TEXT,
    next_bg_seq INTEGER NOT NULL DEFAULT 1,
    state TEXT NOT NULL CHECK (state IN ('active', 'historical')),
    created_at_ms INTEGER NOT NULL,
    UNIQUE(parent_session_id, agent_id),
    UNIQUE(parent_session_id, child_session_id)
);

CREATE TABLE background_tasks (
    task_pk INTEGER PRIMARY KEY AUTOINCREMENT,
    parent_session_id TEXT NOT NULL,
    agent_pk INTEGER NOT NULL,
    bg_task_id TEXT NOT NULL,
    bg_ordinal INTEGER NOT NULL,
    parent_dialog_turn_id TEXT NOT NULL,
    parent_tool_call_id TEXT NOT NULL,
    child_dialog_turn_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK (
        status IN ('running', 'completed', 'partial_timeout', 'failed', 'cancelled', 'interrupted')
    ),
    error_code TEXT,
    error_message TEXT,
    execution_owner_token TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    terminal_at_ms INTEGER,
    delivered_at_ms INTEGER,
    delivered_parent_dialog_turn_id TEXT,
    UNIQUE(parent_session_id, bg_task_id),
    UNIQUE(agent_pk, bg_ordinal),
    FOREIGN KEY(agent_pk) REFERENCES agents(agent_pk) ON DELETE CASCADE
);

CREATE INDEX idx_background_tasks_wait
    ON background_tasks(parent_session_id, delivered_at_ms, status, task_pk);
CREATE INDEX idx_background_tasks_parent_turn
    ON background_tasks(parent_session_id, parent_dialog_turn_id);

PRAGMA user_version = 1;
            "#,
            )
            .map_err(db_error)?;
    }
    if version < 2 {
        connection
            .execute_batch(
                r#"
CREATE TABLE swarm_trees (
    root_session_id TEXT PRIMARY KEY,
    created_at_ms INTEGER NOT NULL
);

CREATE TABLE swarm_nodes (
    session_id TEXT PRIMARY KEY,
    root_session_id TEXT NOT NULL,
    parent_session_id TEXT,
    agent_type TEXT NOT NULL,
    depth INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    FOREIGN KEY(root_session_id) REFERENCES swarm_trees(root_session_id) ON DELETE CASCADE
);

CREATE INDEX idx_swarm_nodes_root ON swarm_nodes(root_session_id);
CREATE INDEX idx_swarm_nodes_parent ON swarm_nodes(parent_session_id);
PRAGMA user_version = 2;
                "#,
            )
            .map_err(db_error)?;
    }
    ensure_v2_additive_columns(connection)?;
    Ok(())
}

/// Schema v2 shipped after the background-task delivery columns were added to
/// schema v1. A retired build could nevertheless leave a database stamped as
/// v2 without those additive columns. Keep the repair keyed to the physical
/// shape so reopening such a database is safe and idempotent.
fn ensure_v2_additive_columns(connection: &Connection) -> OpenBitFunResult<()> {
    for (column, declaration) in [
        ("delivered_at_ms", "INTEGER"),
        ("delivered_parent_dialog_turn_id", "TEXT"),
    ] {
        if !coordination_table_has_column(connection, "background_tasks", column)? {
            connection
                .execute_batch(&format!(
                    "ALTER TABLE background_tasks ADD COLUMN {column} {declaration};"
                ))
                .map_err(db_error)?;
        }
    }
    connection
        .execute_batch(
            r#"
CREATE INDEX IF NOT EXISTS idx_background_tasks_wait
    ON background_tasks(parent_session_id, delivered_at_ms, status, task_pk);
CREATE INDEX IF NOT EXISTS idx_background_tasks_parent_turn
    ON background_tasks(parent_session_id, parent_dialog_turn_id);
            "#,
        )
        .map_err(db_error)?;
    Ok(())
}

pub fn coordination_table_has_column(
    connection: &Connection,
    table: &str,
    expected_column: &str,
) -> OpenBitFunResult<bool> {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(db_error)?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(db_error)?;
    for column in columns {
        if column.map_err(db_error)? == expected_column {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn validate_coordination_agent_id(agent_id: &str) -> OpenBitFunResult<()> {
    let valid = !agent_id.is_empty()
        && agent_id.len() <= 32
        && agent_id
            .bytes()
            .enumerate()
            .all(|(index, byte)| match byte {
                b'a'..=b'z' => true,
                b'0'..=b'9' | b'_' | b'-' => index > 0,
                _ => false,
            });
    if valid {
        Ok(())
    } else {
        Err(OpenBitFunError::tool(
            "agent_id must match [a-z][a-z0-9_-]{0,31}".to_string(),
        ))
    }
}

fn db_error(error: rusqlite::Error) -> OpenBitFunError {
    OpenBitFunError::io(format!("Agent coordination database error: {error}"))
}
