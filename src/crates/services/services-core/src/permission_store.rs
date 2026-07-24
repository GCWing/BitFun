//! SQLite-backed project permission grants and bounded audit facts.

use bitfun_runtime_ports::{
    PermissionAuditEvent, PermissionAuditRecord, PermissionAuditStorePort, PermissionGrant,
    PermissionGrantKey, PermissionGrantStorePort, PermissionReplyStorePort, PermissionRequest,
    PortError, PortErrorKind, PortResult, RuntimeServiceCapability, RuntimeServicePort,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const PERMISSION_STORE_FILE_NAME: &str = "tool-permissions.sqlite";
const PERMISSION_STORE_SCHEMA_VERSION: i64 = 1;
const DEFAULT_AUDIT_LIMIT_PER_PROJECT: usize = 1_000;

#[derive(Debug, Clone)]
struct PreparedAuditRecord {
    record: PermissionAuditRecord,
    request_json: String,
    event_json: String,
}

/// User-level permission persistence. Grants and audit records are scoped by
/// project ID inside one database under the user data directory.
#[derive(Debug, Clone)]
pub struct ProjectPermissionSqliteStore {
    path: PathBuf,
    audit_limit_per_project: usize,
}

impl ProjectPermissionSqliteStore {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            path: base_dir.into().join(PERMISSION_STORE_FILE_NAME),
            audit_limit_per_project: DEFAULT_AUDIT_LIMIT_PER_PROJECT,
        }
    }

    pub fn with_audit_limit(mut self, audit_limit_per_project: usize) -> Self {
        self.audit_limit_per_project = audit_limit_per_project.max(1);
        self
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    async fn execute<R: Send + 'static>(
        &self,
        operation: impl FnOnce(&mut Connection, usize) -> PortResult<R> + Send + 'static,
    ) -> PortResult<R> {
        let path = self.path.clone();
        let audit_limit_per_project = self.audit_limit_per_project;
        tokio::task::spawn_blocking(move || {
            let mut connection = open_connection(&path)?;
            operation(&mut connection, audit_limit_per_project)
        })
        .await
        .map_err(|error| {
            PortError::new(
                PortErrorKind::Backend,
                format!("Permission store worker failed: {error}"),
            )
        })?
    }
}

impl RuntimeServicePort for ProjectPermissionSqliteStore {
    fn capability(&self) -> RuntimeServiceCapability {
        RuntimeServiceCapability::Permission
    }
}

#[async_trait::async_trait]
impl PermissionGrantStorePort for ProjectPermissionSqliteStore {
    async fn list_project_grants(&self, project_id: &str) -> PortResult<Vec<PermissionGrant>> {
        let project_id = project_id.to_string();
        self.execute(move |connection, _| list_grants(connection, &project_id))
            .await
    }

    async fn add_project_grants(&self, grants: Vec<PermissionGrant>) -> PortResult<()> {
        if grants.is_empty() {
            return Ok(());
        }
        validate_grants(&grants)?;

        self.execute(move |connection, _| {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(store_error)?;
            insert_grants(&transaction, &grants)?;
            transaction.commit().map_err(store_error)
        })
        .await
    }

    async fn remove_project_grant(&self, key: PermissionGrantKey) -> PortResult<bool> {
        self.execute(move |connection, _| {
            let removed = connection
                .execute(
                    "DELETE FROM grants WHERE project_id = ?1 AND action = ?2 AND resource = ?3",
                    params![key.project_id, key.action, key.resource],
                )
                .map_err(store_error)?;
            Ok(removed > 0)
        })
        .await
    }

    async fn clear_project_grants(&self, project_id: &str) -> PortResult<usize> {
        if project_id.trim().is_empty() {
            return Err(PortError::new(
                PortErrorKind::InvalidRequest,
                "Permission grant project ID must be non-empty",
            ));
        }
        let project_id = project_id.to_string();

        self.execute(move |connection, _| {
            connection
                .execute(
                    "DELETE FROM grants WHERE project_id = ?1",
                    params![project_id],
                )
                .map_err(store_error)
        })
        .await
    }
}

#[async_trait::async_trait]
impl PermissionReplyStorePort for ProjectPermissionSqliteStore {
    async fn commit_permission_reply(
        &self,
        grants: Vec<PermissionGrant>,
        audit: Vec<PermissionAuditRecord>,
    ) -> PortResult<()> {
        validate_grants(&grants)?;
        let audit = prepare_audit_records(audit)?;

        self.execute(move |connection, audit_limit_per_project| {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(store_error)?;
            ensure_audit_records_compatible(&transaction, &audit)?;
            insert_grants(&transaction, &grants)?;
            insert_audit_records(&transaction, &audit)?;
            prune_audit_records(&transaction, &audit, audit_limit_per_project)?;
            transaction.commit().map_err(store_error)
        })
        .await
    }
}

#[async_trait::async_trait]
impl PermissionAuditStorePort for ProjectPermissionSqliteStore {
    async fn append_permission_audit(&self, record: PermissionAuditRecord) -> PortResult<()> {
        let record = prepare_audit_record(record)?;

        self.execute(move |connection, audit_limit_per_project| {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(store_error)?;
            ensure_audit_record_compatible(&transaction, &record)?;
            insert_audit_records(&transaction, std::slice::from_ref(&record))?;
            prune_audit_records(
                &transaction,
                std::slice::from_ref(&record),
                audit_limit_per_project,
            )?;
            transaction.commit().map_err(store_error)
        })
        .await
    }

    async fn list_project_permission_audit(
        &self,
        project_id: &str,
    ) -> PortResult<Vec<PermissionAuditRecord>> {
        let project_id = project_id.to_string();
        self.execute(move |connection, _| list_audit_records(connection, &project_id))
            .await
    }
}

fn open_connection(path: &Path) -> PortResult<Connection> {
    let parent = path.parent().ok_or_else(|| {
        PortError::new(
            PortErrorKind::Backend,
            format!(
                "Permission store database path has no parent: {}",
                path.display()
            ),
        )
    })?;
    fs::create_dir_all(parent).map_err(store_error)?;

    let mut connection = Connection::open(path).map_err(store_error)?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(store_error)?;
    connection
        .execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;
            "#,
        )
        .map_err(store_error)?;
    initialize_schema(&mut connection)?;
    Ok(connection)
}

fn initialize_schema(connection: &mut Connection) -> PortResult<()> {
    let schema_version = connection
        .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
        .map_err(store_error)?;
    if schema_version == PERMISSION_STORE_SCHEMA_VERSION {
        return Ok(());
    }
    if schema_version != 0 {
        return Err(PortError::new(
            PortErrorKind::Backend,
            format!(
                "Unsupported permission store schema version: {schema_version}; expected at most {PERMISSION_STORE_SCHEMA_VERSION}"
            ),
        ));
    }

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(store_error)?;
    transaction
        .execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS grants (
                project_id TEXT NOT NULL,
                action TEXT NOT NULL,
                resource TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL,
                PRIMARY KEY (project_id, action, resource)
            );

            CREATE TABLE IF NOT EXISTS audit (
                audit_id TEXT PRIMARY KEY NOT NULL,
                project_id TEXT NOT NULL,
                request_json TEXT NOT NULL,
                event_json TEXT NOT NULL,
                timestamp_ms INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_audit_project_timestamp
                ON audit (project_id, timestamp_ms, audit_id);
            "#,
        )
        .map_err(store_error)?;
    transaction
        .pragma_update(None, "user_version", PERMISSION_STORE_SCHEMA_VERSION)
        .map_err(store_error)?;
    transaction.commit().map_err(store_error)
}

fn list_grants(connection: &Connection, project_id: &str) -> PortResult<Vec<PermissionGrant>> {
    let mut statement = connection
        .prepare(
            "SELECT project_id, action, resource, created_at_ms
             FROM grants
             WHERE project_id = ?1
             ORDER BY action ASC, resource ASC",
        )
        .map_err(store_error)?;
    let rows = statement
        .query_map(params![project_id], |row| {
            Ok(PermissionGrant {
                project_id: row.get(0)?,
                action: row.get(1)?,
                resource: row.get(2)?,
                created_at_ms: row.get(3)?,
            })
        })
        .map_err(store_error)?;
    collect_rows(rows)
}

fn list_audit_records(
    connection: &Connection,
    project_id: &str,
) -> PortResult<Vec<PermissionAuditRecord>> {
    let mut statement = connection
        .prepare(
            "SELECT audit_id, request_json, event_json, timestamp_ms
             FROM audit
             WHERE project_id = ?1
             ORDER BY timestamp_ms ASC, audit_id ASC",
        )
        .map_err(store_error)?;
    let rows = statement
        .query_map(params![project_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .map_err(store_error)?;

    let mut records = Vec::new();
    for row in rows {
        let (audit_id, request_json, event_json, timestamp_ms) = row.map_err(store_error)?;
        records.push(PermissionAuditRecord {
            audit_id,
            request: deserialize_audit_value(&request_json, "request")?,
            event: deserialize_audit_value(&event_json, "event")?,
            timestamp_ms,
        });
    }
    Ok(records)
}

fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>>,
) -> PortResult<Vec<T>> {
    rows.collect::<Result<Vec<_>, _>>().map_err(store_error)
}

fn insert_grants(transaction: &Transaction<'_>, grants: &[PermissionGrant]) -> PortResult<()> {
    let mut statement = transaction
        .prepare(
            "INSERT OR IGNORE INTO grants (project_id, action, resource, created_at_ms)
             VALUES (?1, ?2, ?3, ?4)",
        )
        .map_err(store_error)?;
    for grant in grants {
        statement
            .execute(params![
                grant.project_id,
                grant.action,
                grant.resource,
                grant.created_at_ms,
            ])
            .map_err(store_error)?;
    }
    Ok(())
}

fn ensure_audit_records_compatible(
    transaction: &Transaction<'_>,
    records: &[PreparedAuditRecord],
) -> PortResult<()> {
    for record in records {
        ensure_audit_record_compatible(transaction, record)?;
    }
    Ok(())
}

fn ensure_audit_record_compatible(
    transaction: &Transaction<'_>,
    record: &PreparedAuditRecord,
) -> PortResult<()> {
    let existing = transaction
        .query_row(
            "SELECT request_json, event_json FROM audit WHERE audit_id = ?1",
            params![record.record.audit_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(store_error)?;

    if let Some((request_json, event_json)) = existing {
        let existing_request: PermissionRequest =
            deserialize_audit_value(&request_json, "request")?;
        let existing_event: PermissionAuditEvent = deserialize_audit_value(&event_json, "event")?;
        if existing_request != record.record.request || existing_event != record.record.event {
            return Err(audit_conflict(&record.record.audit_id));
        }
    }
    Ok(())
}

fn insert_audit_records(
    transaction: &Transaction<'_>,
    records: &[PreparedAuditRecord],
) -> PortResult<()> {
    let mut statement = transaction
        .prepare(
            "INSERT OR IGNORE INTO audit
             (audit_id, project_id, request_json, event_json, timestamp_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .map_err(store_error)?;
    for record in records {
        statement
            .execute(params![
                record.record.audit_id,
                record.record.request.project_id,
                record.request_json,
                record.event_json,
                record.record.timestamp_ms,
            ])
            .map_err(store_error)?;
    }
    Ok(())
}

fn prune_audit_records(
    transaction: &Transaction<'_>,
    records: &[PreparedAuditRecord],
    audit_limit_per_project: usize,
) -> PortResult<()> {
    let project_ids = records
        .iter()
        .map(|record| record.record.request.project_id.as_str())
        .collect::<HashSet<_>>();
    let offset = i64::try_from(audit_limit_per_project).map_err(|_| {
        PortError::new(
            PortErrorKind::InvalidRequest,
            "Permission audit retention limit exceeds SQLite integer range",
        )
    })?;

    for project_id in project_ids {
        transaction
            .execute(
                "DELETE FROM audit
                 WHERE project_id = ?1
                   AND audit_id IN (
                       SELECT audit_id
                       FROM audit
                       WHERE project_id = ?2
                       ORDER BY timestamp_ms DESC, audit_id DESC
                       LIMIT -1 OFFSET ?3
                   )",
                params![project_id, project_id, offset],
            )
            .map_err(store_error)?;
    }
    Ok(())
}

fn prepare_audit_records(
    records: Vec<PermissionAuditRecord>,
) -> PortResult<Vec<PreparedAuditRecord>> {
    records.into_iter().map(prepare_audit_record).collect()
}

fn prepare_audit_record(record: PermissionAuditRecord) -> PortResult<PreparedAuditRecord> {
    if record.audit_id.trim().is_empty() || record.request.project_id.trim().is_empty() {
        return Err(PortError::new(
            PortErrorKind::InvalidRequest,
            "Permission audit ID and project ID must be non-empty",
        ));
    }

    let request_json = serde_json::to_string(&record.request).map_err(store_error)?;
    let event_json = serde_json::to_string(&record.event).map_err(store_error)?;
    Ok(PreparedAuditRecord {
        record,
        request_json,
        event_json,
    })
}

fn deserialize_audit_value<T: serde::de::DeserializeOwned>(
    value: &str,
    field: &str,
) -> PortResult<T> {
    serde_json::from_str(value).map_err(|error| {
        PortError::new(
            PortErrorKind::Backend,
            format!("Permission audit {field} data is invalid: {error}"),
        )
    })
}

fn validate_grants(grants: &[PermissionGrant]) -> PortResult<()> {
    if grants.iter().any(|grant| {
        grant.project_id.trim().is_empty()
            || grant.action.trim().is_empty()
            || grant.resource.trim().is_empty()
    }) {
        return Err(PortError::new(
            PortErrorKind::InvalidRequest,
            "Permission grant project, action, and resource must be non-empty",
        ));
    }
    Ok(())
}

fn audit_conflict(audit_id: &str) -> PortError {
    PortError::new(
        PortErrorKind::InvalidRequest,
        format!("Permission audit ID already exists with different content: {audit_id}"),
    )
}

fn store_error(error: impl std::fmt::Display) -> PortError {
    PortError::new(
        PortErrorKind::Backend,
        format!("Permission store operation failed: {error}"),
    )
}
