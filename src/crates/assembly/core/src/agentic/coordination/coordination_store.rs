use crate::util::errors::{BitFunError, BitFunResult};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::OnceCell;
use tokio::task;
use uuid::Uuid;

const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BackgroundTaskStatus {
    Running,
    Completed,
    PartialTimeout,
    Failed,
    Cancelled,
    Interrupted,
}

impl BackgroundTaskStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::PartialTimeout => "partial_timeout",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }

    pub(crate) fn is_terminal(self) -> bool {
        self != Self::Running
    }

    fn parse(value: &str) -> BitFunResult<Self> {
        match value {
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "partial_timeout" => Ok(Self::PartialTimeout),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "interrupted" => Ok(Self::Interrupted),
            _ => Err(BitFunError::service(format!(
                "Invalid background task status in coordination database: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BackgroundTaskRegistration {
    pub parent_session_id: String,
    pub requested_agent_id: Option<String>,
    pub child_session_id: String,
    pub parent_dialog_turn_id: String,
    pub parent_tool_call_id: String,
    pub child_dialog_turn_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegisteredBackgroundTask {
    pub task_pk: i64,
    pub agent_id: String,
    pub bg_task_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BackgroundTaskRecord {
    pub task_pk: i64,
    pub parent_session_id: String,
    pub agent_id: String,
    pub bg_task_id: String,
    pub child_session_id: String,
    pub parent_dialog_turn_id: String,
    pub parent_tool_call_id: String,
    pub child_dialog_turn_id: String,
    pub status: BackgroundTaskStatus,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub execution_owner_token: String,
    pub delivered_at_ms: Option<u64>,
}

pub(crate) struct CoordinationStore {
    db_path: PathBuf,
    connection: OnceCell<Arc<Mutex<Connection>>>,
    execution_owner_token: String,
}

impl CoordinationStore {
    pub(crate) fn new(db_path: PathBuf) -> Self {
        Self {
            db_path,
            connection: OnceCell::new(),
            execution_owner_token: Uuid::new_v4().to_string(),
        }
    }

    async fn connection(&self) -> BitFunResult<Arc<Mutex<Connection>>> {
        let db_path = self.db_path.clone();
        self.connection
            .get_or_try_init(|| async move {
                task::spawn_blocking(move || open_connection(db_path))
                    .await
                    .map_err(|error| {
                        BitFunError::service(format!(
                            "Agent coordination database initialization task failed: {error}"
                        ))
                    })?
            })
            .await
            .cloned()
    }

    async fn with_connection<T, F>(&self, operation: F) -> BitFunResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> BitFunResult<T> + Send + 'static,
    {
        let connection = self.connection().await?;
        task::spawn_blocking(move || {
            let mut connection = connection.lock().map_err(|_| {
                BitFunError::service("Agent coordination database lock was poisoned".to_string())
            })?;
            operation(&mut connection)
        })
        .await
        .map_err(|error| {
            BitFunError::service(format!("Agent coordination database task failed: {error}"))
        })?
    }

    pub(crate) async fn agent_id_for_session(
        &self,
        parent_session_id: &str,
        child_session_id: &str,
    ) -> BitFunResult<String> {
        let parent_session_id = parent_session_id.to_string();
        let child_session_id = child_session_id.to_string();
        self.with_connection(move |connection| {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(db_error)?;
            let (_, agent_id) =
                get_or_create_agent(&transaction, &parent_session_id, &child_session_id, None)?;
            transaction.commit().map_err(db_error)?;
            Ok(agent_id)
        })
        .await
    }

    pub(crate) async fn resolve_agent_id(
        &self,
        parent_session_id: &str,
        agent_id: &str,
    ) -> BitFunResult<String> {
        let parent_session_id = parent_session_id.to_string();
        let agent_id = agent_id.to_string();
        self.with_connection(move |connection| {
            connection
                .query_row(
                    "SELECT child_session_id FROM agents WHERE parent_session_id = ?1 AND agent_id = ?2 AND state = 'active'",
                    params![parent_session_id, agent_id],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()
                .map_err(db_error)?
                .flatten()
                .ok_or_else(|| BitFunError::tool(format!("Agent was not found: {agent_id}")))
        })
        .await
    }

    pub(crate) async fn register_background_task(
        &self,
        registration: BackgroundTaskRegistration,
    ) -> BitFunResult<RegisteredBackgroundTask> {
        let execution_owner_token = self.execution_owner_token.clone();
        self.with_connection(move |connection| {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(db_error)?;
            let (agent_pk, agent_id) = get_or_create_agent(
                &transaction,
                &registration.parent_session_id,
                &registration.child_session_id,
                registration.requested_agent_id.as_deref(),
            )?;
            let next_bg_seq = transaction
                .query_row(
                    "SELECT next_bg_seq FROM agents WHERE agent_pk = ?1",
                    params![agent_pk],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(db_error)?;
            let bg_task_id = format!("{agent_id}_bg{next_bg_seq}");
            transaction
                .execute(
                    "UPDATE agents SET next_bg_seq = ?1 WHERE agent_pk = ?2",
                    params![next_bg_seq.saturating_add(1), agent_pk],
                )
                .map_err(db_error)?;
            transaction
                .execute(
                    r#"
INSERT INTO background_tasks (
    parent_session_id, agent_pk, bg_task_id, bg_ordinal,
    parent_dialog_turn_id, parent_tool_call_id, child_dialog_turn_id,
    status, execution_owner_token, created_at_ms
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'running', ?8, ?9)
                    "#,
                    params![
                        registration.parent_session_id,
                        agent_pk,
                        bg_task_id,
                        next_bg_seq,
                        registration.parent_dialog_turn_id,
                        registration.parent_tool_call_id,
                        registration.child_dialog_turn_id,
                        execution_owner_token,
                        unix_time_ms() as i64,
                    ],
                )
                .map_err(db_error)?;
            let task_pk = transaction.last_insert_rowid();
            transaction.commit().map_err(db_error)?;
            Ok(RegisteredBackgroundTask {
                task_pk,
                agent_id,
                bg_task_id,
            })
        })
        .await
    }

    pub(crate) async fn update_task_status(
        &self,
        task_pk: i64,
        status: BackgroundTaskStatus,
        error_code: Option<String>,
        error_message: Option<String>,
    ) -> BitFunResult<bool> {
        self.with_connection(move |connection| {
            let changed = connection
                .execute(
                    r#"
UPDATE background_tasks
SET status = ?1, error_code = ?2, error_message = ?3, terminal_at_ms = ?4
WHERE task_pk = ?5 AND status = 'running'
                    "#,
                    params![
                        status.as_str(),
                        error_code,
                        error_message,
                        status.is_terminal().then(|| unix_time_ms() as i64),
                        task_pk,
                    ],
                )
                .map_err(db_error)?;
            Ok(changed > 0)
        })
        .await
    }

    pub(crate) async fn delete_background_task(&self, task_pk: i64) -> BitFunResult<()> {
        self.with_connection(move |connection| {
            connection
                .execute(
                    "DELETE FROM background_tasks WHERE task_pk = ?1",
                    params![task_pk],
                )
                .map_err(db_error)?;
            Ok(())
        })
        .await
    }

    pub(crate) async fn wait_candidates(
        &self,
        parent_session_id: &str,
        requested_bg_task_ids: &[String],
    ) -> BitFunResult<Vec<BackgroundTaskRecord>> {
        let parent_session_id = parent_session_id.to_string();
        let requested_bg_task_ids = requested_bg_task_ids.to_vec();
        self.with_connection(move |connection| {
            if requested_bg_task_ids.is_empty() {
                let mut statement = connection
                    .prepare(&format!(
                        "{} WHERE tasks.parent_session_id = ?1 AND tasks.delivered_at_ms IS NULL ORDER BY tasks.task_pk",
                        BACKGROUND_TASK_SELECT
                    ))
                    .map_err(db_error)?;
                let rows = statement
                    .query_map(params![parent_session_id], background_task_from_row)
                    .map_err(db_error)?;
                return collect_rows(rows);
            }

            let mut records = Vec::with_capacity(requested_bg_task_ids.len());
            for bg_task_id in requested_bg_task_ids {
                let record = connection
                    .query_row(
                        &format!(
                            "{} WHERE tasks.parent_session_id = ?1 AND tasks.bg_task_id = ?2",
                            BACKGROUND_TASK_SELECT
                        ),
                        params![parent_session_id, bg_task_id],
                        background_task_from_row,
                    )
                    .optional()
                    .map_err(db_error)?
                    .ok_or_else(|| {
                        BitFunError::tool(format!("Background task was not found: {bg_task_id}"))
                    })?;
                if record.delivered_at_ms.is_none() {
                    records.push(record);
                }
            }
            Ok(records)
        })
        .await
    }

    pub(crate) async fn records_by_task_pks(
        &self,
        task_pks: &[i64],
    ) -> BitFunResult<Vec<BackgroundTaskRecord>> {
        let task_pks = task_pks.to_vec();
        self.with_connection(move |connection| {
            let mut records = Vec::with_capacity(task_pks.len());
            for task_pk in task_pks {
                if let Some(record) = connection
                    .query_row(
                        &format!("{} WHERE tasks.task_pk = ?1", BACKGROUND_TASK_SELECT),
                        params![task_pk],
                        background_task_from_row,
                    )
                    .optional()
                    .map_err(db_error)?
                {
                    records.push(record);
                }
            }
            Ok(records)
        })
        .await
    }

    pub(crate) async fn claim_terminal_tasks(
        &self,
        parent_session_id: &str,
        task_pks: &[i64],
        delivered_parent_dialog_turn_id: &str,
    ) -> BitFunResult<Vec<BackgroundTaskRecord>> {
        let parent_session_id = parent_session_id.to_string();
        let task_pks = task_pks.to_vec();
        let delivered_parent_dialog_turn_id = delivered_parent_dialog_turn_id.to_string();
        self.with_connection(move |connection| {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(db_error)?;
            let mut claimed = Vec::new();
            for task_pk in task_pks {
                let changed = transaction
                    .execute(
                        r#"
UPDATE background_tasks
SET delivered_at_ms = ?1, delivered_parent_dialog_turn_id = ?2
WHERE task_pk = ?3
  AND parent_session_id = ?4
  AND status != 'running'
  AND delivered_at_ms IS NULL
                        "#,
                        params![
                            unix_time_ms() as i64,
                            delivered_parent_dialog_turn_id,
                            task_pk,
                            parent_session_id,
                        ],
                    )
                    .map_err(db_error)?;
                if changed == 0 {
                    continue;
                }
                claimed.push(
                    transaction
                        .query_row(
                            &format!("{} WHERE tasks.task_pk = ?1", BACKGROUND_TASK_SELECT),
                            params![task_pk],
                            background_task_from_row,
                        )
                        .map_err(db_error)?,
                );
            }
            transaction.commit().map_err(db_error)?;
            Ok(claimed)
        })
        .await
    }

    pub(crate) async fn stale_running_tasks(
        &self,
        parent_session_id: &str,
    ) -> BitFunResult<Vec<BackgroundTaskRecord>> {
        let parent_session_id = parent_session_id.to_string();
        let execution_owner_token = self.execution_owner_token.clone();
        self.with_connection(move |connection| {
            let mut statement = connection
                .prepare(&format!(
                    "{} WHERE tasks.parent_session_id = ?1 AND tasks.status = 'running' AND tasks.execution_owner_token != ?2",
                    BACKGROUND_TASK_SELECT
                ))
                .map_err(db_error)?;
            let rows = statement
                .query_map(
                    params![parent_session_id, execution_owner_token],
                    background_task_from_row,
                )
                .map_err(db_error)?;
            collect_rows(rows)
        })
        .await
    }

    pub(crate) async fn delete_session_references(
        &self,
        session_id: &str,
    ) -> BitFunResult<Vec<i64>> {
        let session_id = session_id.to_string();
        self.with_connection(move |connection| {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(db_error)?;
            let deleted_task_pks = {
                let mut statement = transaction
                    .prepare(
                        "SELECT task_pk FROM background_tasks WHERE parent_session_id = ?1 OR agent_pk IN (SELECT agent_pk FROM agents WHERE child_session_id = ?1)",
                    )
                    .map_err(db_error)?;
                let task_pks = statement
                    .query_map(params![session_id], |row| row.get::<_, i64>(0))
                    .map_err(db_error)?
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(db_error)?;
                task_pks
            };
            transaction
                .execute(
                    "DELETE FROM background_tasks WHERE parent_session_id = ?1 OR agent_pk IN (SELECT agent_pk FROM agents WHERE child_session_id = ?1)",
                    params![session_id],
                )
                .map_err(db_error)?;
            transaction
                .execute(
                    "DELETE FROM agents WHERE parent_session_id = ?1 OR child_session_id = ?1",
                    params![session_id],
                )
                .map_err(db_error)?;
            transaction
                .execute(
                    "DELETE FROM coordination_sessions WHERE parent_session_id = ?1",
                    params![session_id],
                )
                .map_err(db_error)?;
            transaction.commit().map_err(db_error)?;
            Ok(deleted_task_pks)
        })
        .await
    }

    pub(crate) async fn rollback_parent_turns(
        &self,
        parent_session_id: &str,
        parent_dialog_turn_ids: &[String],
    ) -> BitFunResult<Vec<i64>> {
        let parent_session_id = parent_session_id.to_string();
        let parent_dialog_turn_ids = parent_dialog_turn_ids.to_vec();
        self.with_connection(move |connection| {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(db_error)?;
            let mut deleted_task_pks = Vec::new();
            for turn_id in parent_dialog_turn_ids {
                {
                    let mut statement = transaction
                        .prepare(
                            "SELECT task_pk FROM background_tasks WHERE parent_session_id = ?1 AND parent_dialog_turn_id = ?2",
                        )
                        .map_err(db_error)?;
                    deleted_task_pks.extend(
                        statement
                            .query_map(params![parent_session_id, turn_id], |row| {
                                row.get::<_, i64>(0)
                            })
                            .map_err(db_error)?
                            .collect::<rusqlite::Result<Vec<_>>>()
                            .map_err(db_error)?,
                    );
                }
                transaction
                    .execute(
                        "DELETE FROM background_tasks WHERE parent_session_id = ?1 AND parent_dialog_turn_id = ?2",
                        params![parent_session_id, turn_id],
                    )
                    .map_err(db_error)?;
                transaction
                    .execute(
                        "UPDATE background_tasks SET delivered_at_ms = NULL, delivered_parent_dialog_turn_id = NULL WHERE parent_session_id = ?1 AND delivered_parent_dialog_turn_id = ?2",
                        params![parent_session_id, turn_id],
                    )
                    .map_err(db_error)?;
            }
            transaction.commit().map_err(db_error)?;
            Ok(deleted_task_pks)
        })
        .await
    }

    pub(crate) async fn initialize_fork(
        &self,
        source_parent_session_id: &str,
        target_parent_session_id: &str,
    ) -> BitFunResult<()> {
        let source_parent_session_id = source_parent_session_id.to_string();
        let target_parent_session_id = target_parent_session_id.to_string();
        self.with_connection(move |connection| {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(db_error)?;
            let next_auto_agent_seq = transaction
                .query_row(
                    "SELECT next_auto_agent_seq FROM coordination_sessions WHERE parent_session_id = ?1",
                    params![source_parent_session_id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()
                .map_err(db_error)?
                .unwrap_or(1);
            transaction
                .execute(
                    "INSERT OR IGNORE INTO coordination_sessions (parent_session_id, next_auto_agent_seq, updated_at_ms) VALUES (?1, ?2, ?3)",
                    params![target_parent_session_id, next_auto_agent_seq, unix_time_ms() as i64],
                )
                .map_err(db_error)?;

            let reservations = {
                let mut statement = transaction
                    .prepare(
                        "SELECT agent_id, next_bg_seq FROM agents WHERE parent_session_id = ?1 ORDER BY agent_pk",
                    )
                    .map_err(db_error)?;
                let rows = statement
                    .query_map(params![source_parent_session_id], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                    })
                    .map_err(db_error)?;
                rows.collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(db_error)?
            };
            for (agent_id, next_bg_seq) in reservations {
                transaction
                    .execute(
                        "INSERT OR IGNORE INTO agents (parent_session_id, agent_id, child_session_id, next_bg_seq, state, created_at_ms) VALUES (?1, ?2, NULL, ?3, 'historical', ?4)",
                        params![target_parent_session_id, agent_id, next_bg_seq, unix_time_ms() as i64],
                    )
                    .map_err(db_error)?;
            }
            transaction.commit().map_err(db_error)?;
            Ok(())
        })
        .await
    }
}

const BACKGROUND_TASK_SELECT: &str = r#"
SELECT
    tasks.task_pk,
    tasks.parent_session_id,
    agents.agent_id,
    tasks.bg_task_id,
    agents.child_session_id,
    tasks.parent_dialog_turn_id,
    tasks.parent_tool_call_id,
    tasks.child_dialog_turn_id,
    tasks.status,
    tasks.error_code,
    tasks.error_message,
    tasks.execution_owner_token,
    tasks.delivered_at_ms
FROM background_tasks AS tasks
JOIN agents ON agents.agent_pk = tasks.agent_pk
"#;

fn background_task_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<BackgroundTaskRecord> {
    let status = row.get::<_, String>(8)?;
    let status = BackgroundTaskStatus::parse(&status).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            8,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                error.to_string(),
            )),
        )
    })?;
    let delivered_at_ms = row
        .get::<_, Option<i64>>(12)?
        .and_then(|value| u64::try_from(value).ok());
    Ok(BackgroundTaskRecord {
        task_pk: row.get(0)?,
        parent_session_id: row.get(1)?,
        agent_id: row.get(2)?,
        bg_task_id: row.get(3)?,
        child_session_id: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
        parent_dialog_turn_id: row.get(5)?,
        parent_tool_call_id: row.get(6)?,
        child_dialog_turn_id: row.get(7)?,
        status,
        error_code: row.get(9)?,
        error_message: row.get(10)?,
        execution_owner_token: row.get(11)?,
        delivered_at_ms,
    })
}

fn collect_rows(
    rows: rusqlite::MappedRows<
        '_,
        impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<BackgroundTaskRecord>,
    >,
) -> BitFunResult<Vec<BackgroundTaskRecord>> {
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_error)
}

fn get_or_create_agent(
    transaction: &Transaction<'_>,
    parent_session_id: &str,
    child_session_id: &str,
    requested_agent_id: Option<&str>,
) -> BitFunResult<(i64, String)> {
    if let Some(existing) = transaction
        .query_row(
            "SELECT agent_pk, agent_id FROM agents WHERE parent_session_id = ?1 AND child_session_id = ?2",
            params![parent_session_id, child_session_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(db_error)?
    {
        if requested_agent_id.is_some_and(|requested_agent_id| existing.1 != requested_agent_id) {
            return Err(BitFunError::tool(format!(
                "Subagent session is already registered as agent_id={}",
                existing.1
            )));
        }
        return Ok(existing);
    }

    transaction
        .execute(
            "INSERT OR IGNORE INTO coordination_sessions (parent_session_id, next_auto_agent_seq, updated_at_ms) VALUES (?1, 1, ?2)",
            params![parent_session_id, unix_time_ms() as i64],
        )
        .map_err(db_error)?;

    let agent_id = match requested_agent_id {
        Some(agent_id) => {
            validate_agent_id(agent_id)?;
            agent_id.to_string()
        }
        None => loop {
            let next = transaction
                .query_row(
                    "SELECT next_auto_agent_seq FROM coordination_sessions WHERE parent_session_id = ?1",
                    params![parent_session_id],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(db_error)?;
            transaction
                .execute(
                    "UPDATE coordination_sessions SET next_auto_agent_seq = ?1, updated_at_ms = ?2 WHERE parent_session_id = ?3",
                    params![next.saturating_add(1), unix_time_ms() as i64, parent_session_id],
                )
                .map_err(db_error)?;
            let candidate = format!("a{next}");
            let exists = transaction
                .query_row(
                    "SELECT 1 FROM agents WHERE parent_session_id = ?1 AND agent_id = ?2",
                    params![parent_session_id, candidate],
                    |_row| Ok(()),
                )
                .optional()
                .map_err(db_error)?
                .is_some();
            if !exists {
                break candidate;
            }
        },
    };

    transaction
        .execute(
            "INSERT INTO agents (parent_session_id, agent_id, child_session_id, next_bg_seq, state, created_at_ms) VALUES (?1, ?2, ?3, 1, 'active', ?4)",
            params![parent_session_id, agent_id, child_session_id, unix_time_ms() as i64],
        )
        .map_err(|error| {
            BitFunError::tool(format!(
                "Failed to register agent_id={agent_id} for the parent session: {error}"
            ))
        })?;
    Ok((transaction.last_insert_rowid(), agent_id))
}

fn validate_agent_id(agent_id: &str) -> BitFunResult<()> {
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
        Err(BitFunError::tool(
            "agent_id must match [a-z][a-z0-9_-]{0,31}".to_string(),
        ))
    }
}

fn open_connection(db_path: PathBuf) -> BitFunResult<Arc<Mutex<Connection>>> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            BitFunError::io(format!(
                "Failed to create agent coordination database directory {}: {error}",
                parent.display()
            ))
        })?;
    }
    let connection = Connection::open(&db_path).map_err(|error| {
        BitFunError::io(format!(
            "Failed to open agent coordination database {}: {error}",
            db_path.display()
        ))
    })?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(db_error)?;
    connection
        .execute_batch(
            r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA synchronous = NORMAL;
            "#,
        )
        .map_err(db_error)?;
    initialize_schema(&connection)?;
    Ok(Arc::new(Mutex::new(connection)))
}

fn initialize_schema(connection: &Connection) -> BitFunResult<()> {
    let version = connection
        .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
        .map_err(db_error)?;
    if version > SCHEMA_VERSION {
        return Err(BitFunError::service(format!(
            "Agent coordination database schema {version} is newer than supported schema {SCHEMA_VERSION}"
        )));
    }
    if version == SCHEMA_VERSION {
        return Ok(());
    }
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
    Ok(())
}

fn db_error(error: rusqlite::Error) -> BitFunError {
    BitFunError::io(format!("Agent coordination database error: {error}"))
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> (tempfile::TempDir, CoordinationStore) {
        let root = tempfile::tempdir().expect("coordination store temp directory");
        let store = CoordinationStore::new(root.path().join("coordination.sqlite"));
        (root, store)
    }

    fn registration(
        parent_session_id: &str,
        child_session_id: &str,
        parent_dialog_turn_id: &str,
        requested_agent_id: Option<&str>,
    ) -> BackgroundTaskRegistration {
        BackgroundTaskRegistration {
            parent_session_id: parent_session_id.to_string(),
            requested_agent_id: requested_agent_id.map(str::to_string),
            child_session_id: child_session_id.to_string(),
            parent_dialog_turn_id: parent_dialog_turn_id.to_string(),
            parent_tool_call_id: format!("tool-{parent_dialog_turn_id}"),
            child_dialog_turn_id: format!("turn-{child_session_id}-{parent_dialog_turn_id}"),
        }
    }

    #[tokio::test]
    async fn model_ids_are_parent_scoped_and_task_ids_are_agent_scoped() {
        let (_root, store) = test_store();

        let first = store
            .register_background_task(registration("parent-1", "child-1", "parent-turn-1", None))
            .await
            .expect("register first task");
        let second = store
            .register_background_task(registration("parent-1", "child-1", "parent-turn-2", None))
            .await
            .expect("register second task");
        let named = store
            .register_background_task(registration(
                "parent-1",
                "child-reviewer",
                "parent-turn-3",
                Some("reviewer"),
            ))
            .await
            .expect("register named agent task");
        let other_parent = store
            .register_background_task(registration("parent-2", "child-2", "parent-turn-1", None))
            .await
            .expect("register task for another parent");

        assert_eq!(
            (first.agent_id.as_str(), first.bg_task_id.as_str()),
            ("a1", "a1_bg1")
        );
        assert_eq!(
            (second.agent_id.as_str(), second.bg_task_id.as_str()),
            ("a1", "a1_bg2")
        );
        assert_eq!(
            (named.agent_id.as_str(), named.bg_task_id.as_str()),
            ("reviewer", "reviewer_bg1")
        );
        assert_eq!(
            (
                other_parent.agent_id.as_str(),
                other_parent.bg_task_id.as_str()
            ),
            ("a1", "a1_bg1")
        );
        assert_eq!(
            store
                .resolve_agent_id("parent-1", "reviewer")
                .await
                .expect("resolve named agent"),
            "child-reviewer"
        );
    }

    #[tokio::test]
    async fn terminal_transition_and_delivery_claim_are_single_winner() {
        let (_root, store) = test_store();
        let task = store
            .register_background_task(registration("parent", "child", "spawn-turn", None))
            .await
            .expect("register task");

        assert!(store
            .update_task_status(task.task_pk, BackgroundTaskStatus::Completed, None, None)
            .await
            .expect("complete task"));
        assert!(!store
            .update_task_status(
                task.task_pk,
                BackgroundTaskStatus::Cancelled,
                Some("late_cancel".to_string()),
                Some("late cancellation".to_string()),
            )
            .await
            .expect("late terminal transition"));

        let first_claim = store
            .claim_terminal_tasks("parent", &[task.task_pk], "wait-turn-1")
            .await
            .expect("claim completed task");
        let second_claim = store
            .claim_terminal_tasks("parent", &[task.task_pk], "wait-turn-2")
            .await
            .expect("repeat claim");
        assert_eq!(first_claim.len(), 1);
        assert_eq!(first_claim[0].status, BackgroundTaskStatus::Completed);
        assert!(second_claim.is_empty());
    }

    #[tokio::test]
    async fn stale_running_tasks_can_only_be_reconciled_once() {
        let root = tempfile::tempdir().expect("coordination store temp directory");
        let db_path = root.path().join("coordination.sqlite");
        let first_owner = CoordinationStore::new(db_path.clone());
        let task = first_owner
            .register_background_task(registration("parent", "child", "spawn-turn", None))
            .await
            .expect("register running task");
        let second_owner = CoordinationStore::new(db_path);

        let stale = second_owner
            .stale_running_tasks("parent")
            .await
            .expect("load stale running tasks");
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].task_pk, task.task_pk);
        assert!(second_owner
            .update_task_status(
                task.task_pk,
                BackgroundTaskStatus::Interrupted,
                Some("execution_interrupted".to_string()),
                Some("execution interrupted".to_string()),
            )
            .await
            .expect("reconcile stale task"));
        assert!(!first_owner
            .update_task_status(task.task_pk, BackgroundTaskStatus::Completed, None, None)
            .await
            .expect("late original owner completion"));
    }

    #[tokio::test]
    async fn rollback_deletes_spawned_tasks_and_restores_rolled_back_deliveries() {
        let (_root, store) = test_store();
        let delivered = store
            .register_background_task(registration("parent", "child-1", "spawn-turn-1", None))
            .await
            .expect("register delivered task");
        let removed = store
            .register_background_task(registration("parent", "child-2", "spawn-turn-2", None))
            .await
            .expect("register removable task");
        assert!(store
            .update_task_status(
                delivered.task_pk,
                BackgroundTaskStatus::Completed,
                None,
                None,
            )
            .await
            .expect("complete delivered task"));
        store
            .claim_terminal_tasks("parent", &[delivered.task_pk], "delivery-turn")
            .await
            .expect("claim delivered task");

        let deleted = store
            .rollback_parent_turns(
                "parent",
                &["spawn-turn-2".to_string(), "delivery-turn".to_string()],
            )
            .await
            .expect("rollback parent turns");
        assert_eq!(deleted, vec![removed.task_pk]);
        let candidates = store
            .wait_candidates("parent", &[])
            .await
            .expect("load restored candidates");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].task_pk, delivered.task_pk);
        assert!(candidates[0].delivered_at_ms.is_none());
    }

    #[tokio::test]
    async fn fork_preserves_id_reservations_without_copying_tasks() {
        let (_root, store) = test_store();
        let first = store
            .register_background_task(registration("source", "child-1", "spawn-turn-1", None))
            .await
            .expect("register first source agent");
        store
            .agent_id_for_session("source", "child-2")
            .await
            .expect("reserve second source agent");
        store
            .initialize_fork("source", "target")
            .await
            .expect("initialize fork reservations");

        assert!(store
            .wait_candidates("target", &[])
            .await
            .expect("load fork tasks")
            .is_empty());
        let target = store
            .register_background_task(registration("target", "target-child", "spawn-turn", None))
            .await
            .expect("register target task");
        assert_eq!(first.agent_id, "a1");
        assert_eq!(target.agent_id, "a3");
        assert_eq!(target.bg_task_id, "a3_bg1");
        assert!(store
            .register_background_task(registration(
                "target",
                "explicit-child",
                "explicit-turn",
                Some("a1"),
            ))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn deleting_a_parent_or_child_removes_their_tasks() {
        let (_root, store) = test_store();
        let first = store
            .register_background_task(registration("parent", "child-1", "spawn-turn-1", None))
            .await
            .expect("register first child task");
        let second = store
            .register_background_task(registration("parent", "child-2", "spawn-turn-2", None))
            .await
            .expect("register second child task");

        assert_eq!(
            store
                .delete_session_references("child-1")
                .await
                .expect("delete child references"),
            vec![first.task_pk]
        );
        assert_eq!(
            store
                .delete_session_references("parent")
                .await
                .expect("delete parent references"),
            vec![second.task_pk]
        );
        assert!(store
            .wait_candidates("parent", &[])
            .await
            .expect("load remaining tasks")
            .is_empty());
    }
}
