use super::common::{
    backup_domain_dir, io_error, read_bounded_json, read_optional_bounded_json, stage_domain_dir,
    validate_regular_file,
};
use super::workspace_sessions::{
    read_workspace_sessions_manifest, target_wins_session_ids, SessionImportAction,
    WorkspaceSessionsManifest,
};
use openbitfun_legacy_migration::{
    atomic_write_bytes, atomic_write_json, snapshot_sqlite_read_only, validate_sqlite,
    DomainContext, DomainScan, LegacyDomainAdapter, LegacyMigrationError, LegacyMigrationResult,
    MigrationRoots,
};
use openbitfun_product_domains::legacy_migration::{
    ConflictResolution, FindingSeverity, MigrationConflict, MigrationDiagnostic, MigrationDomainId,
    MigrationDomainResult, MigrationDomainState, ScanFinding,
};
use openbitfun_services_core::coordination_persistence::{
    coordination_table_has_column, initialize_coordination_schema, COORDINATION_SCHEMA_VERSION,
};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

const COORDINATION_RELATIVE_PATH: &str = "data/agent-runtime/coordination.sqlite";

pub(crate) struct AgentCoordinationAdapter;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CoordinationManifest {
    source_digest: String,
    target_existed: bool,
    target_digest: Option<String>,
    merged_digest: String,
    imported: u64,
    skipped: u64,
    conflicts: u64,
    #[serde(default)]
    incompatible_rows: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct CoordinationData {
    sessions: Vec<CoordinationSessionRow>,
    agents: Vec<AgentRow>,
    tasks: Vec<BackgroundTaskRow>,
    swarm_trees: Vec<SwarmTreeRow>,
    swarm_nodes: Vec<SwarmNodeRow>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    unreadable_rows: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct CoordinationSessionRow {
    parent_session_id: String,
    next_auto_agent_seq: i64,
    updated_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentRow {
    agent_pk: i64,
    parent_session_id: String,
    agent_id: String,
    child_session_id: Option<String>,
    next_bg_seq: i64,
    state: String,
    created_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct BackgroundTaskRow {
    task_pk: i64,
    parent_session_id: String,
    agent_pk: i64,
    bg_task_id: String,
    bg_ordinal: i64,
    parent_dialog_turn_id: String,
    parent_tool_call_id: String,
    child_dialog_turn_id: String,
    status: String,
    error_code: Option<String>,
    error_message: Option<String>,
    execution_owner_token: String,
    created_at_ms: i64,
    terminal_at_ms: Option<i64>,
    delivered_at_ms: Option<i64>,
    delivered_parent_dialog_turn_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SwarmTreeRow {
    root_session_id: String,
    created_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SwarmNodeRow {
    session_id: String,
    root_session_id: String,
    parent_session_id: Option<String>,
    agent_type: String,
    depth: i64,
    created_at_ms: i64,
}

#[derive(Default)]
struct MergeOutcome {
    imported: u64,
    duplicate: u64,
    target_wins: u64,
    incompatible_rows: u64,
    conflicts: Vec<MigrationConflict>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentMergeTarget {
    Imported(i64),
    Existing(i64),
    Rejected,
}

impl AgentMergeTarget {
    fn agent_pk(self) -> Option<i64> {
        match self {
            Self::Imported(agent_pk) | Self::Existing(agent_pk) => Some(agent_pk),
            Self::Rejected => None,
        }
    }
}

impl LegacyDomainAdapter for AgentCoordinationAdapter {
    fn domain(&self) -> MigrationDomainId {
        MigrationDomainId::AgentCoordination
    }

    fn scan(&self, roots: &MigrationRoots) -> LegacyMigrationResult<DomainScan> {
        let source_path = source_coordination_path(roots);
        validate_regular_file(&roots.legacy_user_root, &source_path).map_err(|_| {
            LegacyMigrationError::UnsupportedSource(
                "the selected Session group requires a readable legacy coordination.sqlite"
                    .to_string(),
            )
        })?;
        let source = load_coordination_data(&source_path, DatabaseRole::LegacySource)?;
        let target_path = target_coordination_path(roots);
        let target = if target_path.exists() {
            validate_regular_file(&roots.target_user_root, &target_path)?;
            load_coordination_data(&target_path, DatabaseRole::CurrentTarget)?
        } else {
            CoordinationData::default()
        };
        let blocked_sessions = target_wins_session_ids(roots)?;
        let conflicts = preview_conflicts(&source, &target, &blocked_sessions);
        let entity_count = coordination_entity_count(&source);
        let logical_bytes = fs::metadata(&source_path)
            .map_err(|error| io_error(&source_path, error))?
            .len();
        Ok(DomainScan {
            finding: ScanFinding {
                domain: self.domain(),
                code: if source.unreadable_rows.is_empty() {
                    "legacy_agent_coordination_supported"
                } else {
                    "coordination_rows_skipped"
                }
                .to_string(),
                severity: if conflicts.is_empty() && source.unreadable_rows.is_empty() {
                    FindingSeverity::Info
                } else {
                    FindingSeverity::Warning
                },
                entity_count,
                logical_bytes,
                source_schema: Some(format!(
                    "bitfun.agent-coordination.v{}",
                    schema_version(&source_path)?
                )),
                migratable: true,
                detail: format!("{entity_count} Agent coordination records are owner-readable"),
            },
            conflicts,
            target_schema: Some(format!(
                "openbitfun.agent-coordination.v{COORDINATION_SCHEMA_VERSION}"
            )),
            dependencies: vec![MigrationDomainId::WorkspaceSessions],
        })
    }

    fn stage(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<MigrationDomainResult> {
        let domain_root = stage_domain_dir(context, "agent-coordination");
        let staged_source = domain_root.join("source.sqlite");
        let staged_target = domain_root.join("target-before.sqlite");
        let staged_merged = domain_root.join("merged.sqlite");
        reset_stage_file(&staged_source)?;
        reset_stage_file(&staged_target)?;
        reset_stage_file(&staged_merged)?;

        let source_path = source_coordination_path(context.roots);
        validate_regular_file(&context.roots.legacy_user_root, &source_path)?;
        snapshot_sqlite_read_only(&source_path, &staged_source)?;
        initialize_snapshot(&staged_source)?;
        let source = load_coordination_data(&staged_source, DatabaseRole::StagedCurrent)?;
        let workspace_manifest = read_workspace_sessions_manifest(context)?;
        let blocked_sessions = manifest_target_wins_session_ids(&workspace_manifest);

        let target_path = target_coordination_path(context.roots);
        let target_existed = target_path.exists();
        let target_digest = if target_existed {
            validate_regular_file(&context.roots.target_user_root, &target_path)?;
            snapshot_sqlite_read_only(&target_path, &staged_target)?;
            initialize_snapshot(&staged_target)?;
            snapshot_sqlite_read_only(&staged_target, &staged_merged)?;
            Some(coordination_digest(&load_coordination_data(
                &staged_target,
                DatabaseRole::StagedCurrent,
            )?)?)
        } else {
            let connection = Connection::open(&staged_merged)
                .map_err(|error| db_error(&staged_merged, error))?;
            initialize_coordination_schema(&connection)
                .map_err(|error| owner_error("initialize staged coordination database", error))?;
            None
        };

        let outcome =
            merge_coordination_database(&staged_source, &staged_merged, &blocked_sessions)?;
        validate_sqlite(&staged_merged)?;
        validate_current_database(&staged_merged)?;
        let merged = load_coordination_data(&staged_merged, DatabaseRole::StagedCurrent)?;
        let manifest = CoordinationManifest {
            source_digest: coordination_digest(&source)?,
            target_existed,
            target_digest,
            merged_digest: coordination_digest(&merged)?,
            imported: outcome.imported,
            skipped: outcome
                .duplicate
                .saturating_add(outcome.target_wins)
                .saturating_add(outcome.incompatible_rows),
            conflicts: outcome.target_wins,
            incompatible_rows: outcome.incompatible_rows,
        };
        atomic_write_json(&coordination_manifest_path(context), &manifest)?;

        Ok(MigrationDomainResult {
            domain: self.domain(),
            state: MigrationDomainState::Staged,
            imported: manifest.imported,
            skipped: manifest.skipped,
            conflicts: manifest.conflicts,
            warnings: if manifest.incompatible_rows > 0 {
                vec![MigrationDiagnostic {
                code: "coordination_rows_skipped".to_string(), severity: FindingSeverity::Warning,
                domain: Some(self.domain()),
                message: format!("{} coordination records could not be represented in the target store and were preserved in the source.", manifest.incompatible_rows),
                ..Default::default()
            }]
            } else {
                Vec::new()
            },
            ..MigrationDomainResult::default()
        })
    }

    fn validate_stage(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<()> {
        let manifest = read_coordination_manifest(context)?;
        let domain_root = stage_domain_dir(context, "agent-coordination");
        let source_path = domain_root.join("source.sqlite");
        let merged_path = domain_root.join("merged.sqlite");
        validate_sqlite(&source_path)?;
        validate_sqlite(&merged_path)?;
        validate_current_database(&merged_path)?;
        let source = load_coordination_data(&source_path, DatabaseRole::StagedCurrent)?;
        if coordination_digest(&source)? != manifest.source_digest {
            return Err(LegacyMigrationError::InvalidRequest(
                "staged Agent coordination source changed after snapshot".to_string(),
            ));
        }
        let merged = load_coordination_data(&merged_path, DatabaseRole::StagedCurrent)?;
        if coordination_digest(&merged)? != manifest.merged_digest {
            return Err(LegacyMigrationError::InvalidRequest(
                "staged Agent coordination merge differs from its manifest".to_string(),
            ));
        }
        Ok(())
    }

    fn commit(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<()> {
        let manifest = read_coordination_manifest(context)?;
        let target = target_coordination_path(context.roots);
        if target.exists() {
            validate_regular_file(&context.roots.target_user_root, &target)?;
            if validate_current_database(&target).is_ok() {
                let current = load_coordination_data(&target, DatabaseRole::CurrentTarget)?;
                if coordination_digest(&current)? == manifest.merged_digest {
                    finalize_sqlite_file(&target)?;
                    return Ok(());
                }
            }
        }
        verify_target_state(&target, &manifest)?;
        let backup = backup_domain_dir(context, "agent-coordination").join("coordination.sqlite");
        if manifest.target_existed && !backup.exists() {
            snapshot_sqlite_read_only(&target, &backup)?;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
        }
        let staged_source = stage_domain_dir(context, "agent-coordination").join("source.sqlite");
        let blocked_sessions =
            manifest_target_wins_session_ids(&read_workspace_sessions_manifest(context)?);
        merge_coordination_database(&staged_source, &target, &blocked_sessions)?;
        finalize_sqlite_file(&target)?;
        validate_current_database(&target)?;
        let merged = load_coordination_data(&target, DatabaseRole::CurrentTarget)?;
        if coordination_digest(&merged)? != manifest.merged_digest {
            return Err(LegacyMigrationError::InvalidRequest(
                "committed Agent coordination database differs from the staged merge".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_commit(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<()> {
        validate_committed_coordination_storage(context)
    }

    fn rollback_unverified(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<()> {
        let Some(manifest) = read_optional_bounded_json::<CoordinationManifest>(
            &context.layout.stage_root(),
            &coordination_manifest_path(context),
        )?
        else {
            return Ok(());
        };
        let target = target_coordination_path(context.roots);
        remove_sqlite_sidecars(&target)?;
        let backup = backup_domain_dir(context, "agent-coordination").join("coordination.sqlite");
        if manifest.target_existed {
            if backup.exists() {
                let bytes = fs::read(&backup).map_err(|error| io_error(&backup, error))?;
                atomic_write_bytes(&target, &bytes)?;
            }
        } else {
            remove_file_if_present(&target)?;
        }
        remove_sqlite_sidecars(&target)
    }
}

pub(crate) fn validate_committed_coordination_storage(
    context: &DomainContext<'_>,
) -> LegacyMigrationResult<()> {
    let manifest = read_coordination_manifest(context)?;
    let target = target_coordination_path(context.roots);
    validate_sqlite(&target)?;
    validate_current_database(&target)?;
    let actual = load_coordination_data(&target, DatabaseRole::CurrentTarget)?;
    if coordination_digest(&actual)? != manifest.merged_digest {
        return Err(LegacyMigrationError::InvalidRequest(
            "committed Agent coordination database changed before cross-reference validation"
                .to_string(),
        ));
    }
    Ok(())
}

fn source_coordination_path(roots: &MigrationRoots) -> PathBuf {
    roots.legacy_user_root.join(COORDINATION_RELATIVE_PATH)
}

fn target_coordination_path(roots: &MigrationRoots) -> PathBuf {
    roots.target_user_root.join(COORDINATION_RELATIVE_PATH)
}

fn coordination_manifest_path(context: &DomainContext<'_>) -> PathBuf {
    stage_domain_dir(context, "agent-coordination").join("manifest.json")
}

fn read_coordination_manifest(
    context: &DomainContext<'_>,
) -> LegacyMigrationResult<CoordinationManifest> {
    read_bounded_json(
        &context.layout.stage_root(),
        &coordination_manifest_path(context),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DatabaseRole {
    LegacySource,
    CurrentTarget,
    StagedCurrent,
}

fn load_coordination_data(
    path: &Path,
    role: DatabaseRole,
) -> LegacyMigrationResult<CoordinationData> {
    validate_sqlite(path)?;
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| db_error(path, error))?;
    let version = connection
        .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
        .map_err(|error| db_error(path, error))?;
    match role {
        DatabaseRole::LegacySource if !(1..=COORDINATION_SCHEMA_VERSION).contains(&version) => {
            return Err(LegacyMigrationError::UnsupportedSource(format!(
                "legacy Agent coordination schema {version} is not supported"
            )));
        }
        DatabaseRole::CurrentTarget if !(0..=COORDINATION_SCHEMA_VERSION).contains(&version) => {
            return Err(LegacyMigrationError::InvalidRequest(format!(
                "target Agent coordination schema {version} is not supported"
            )));
        }
        DatabaseRole::StagedCurrent if version != COORDINATION_SCHEMA_VERSION => {
            return Err(LegacyMigrationError::InvalidRequest(format!(
                "staged Agent coordination schema {version} is not current"
            )));
        }
        _ => {}
    }
    if version == 0 && role == DatabaseRole::CurrentTarget {
        return Ok(CoordinationData::default());
    }
    validate_required_tables(&connection, path, version)?;
    let delivered_columns =
        coordination_table_has_column(&connection, "background_tasks", "delivered_at_ms")
            .map_err(|error| owner_error("inspect coordination delivery columns", error))?
            && coordination_table_has_column(
                &connection,
                "background_tasks",
                "delivered_parent_dialog_turn_id",
            )
            .map_err(|error| owner_error("inspect coordination delivery columns", error))?;
    let mut unreadable_rows = Vec::new();
    let sessions = query_rows(
        &connection,
        &mut unreadable_rows,
        path,
        "SELECT parent_session_id, next_auto_agent_seq, updated_at_ms FROM coordination_sessions ORDER BY parent_session_id",
        |row| {
            Ok(CoordinationSessionRow {
                parent_session_id: row.get(0)?,
                next_auto_agent_seq: row.get(1)?,
                updated_at_ms: row.get(2)?,
            })
        },
    )?;
    let agents = query_rows(
        &connection,
        &mut unreadable_rows,
        path,
        "SELECT agent_pk, parent_session_id, agent_id, child_session_id, next_bg_seq, state, created_at_ms FROM agents ORDER BY agent_pk",
        |row| {
            Ok(AgentRow {
                agent_pk: row.get(0)?,
                parent_session_id: row.get(1)?,
                agent_id: row.get(2)?,
                child_session_id: row.get(3)?,
                next_bg_seq: row.get(4)?,
                state: row.get(5)?,
                created_at_ms: row.get(6)?,
            })
        },
    )?;
    let task_sql = if delivered_columns {
        "SELECT task_pk, parent_session_id, agent_pk, bg_task_id, bg_ordinal, parent_dialog_turn_id, parent_tool_call_id, child_dialog_turn_id, status, error_code, error_message, execution_owner_token, created_at_ms, terminal_at_ms, delivered_at_ms, delivered_parent_dialog_turn_id FROM background_tasks ORDER BY task_pk"
    } else {
        "SELECT task_pk, parent_session_id, agent_pk, bg_task_id, bg_ordinal, parent_dialog_turn_id, parent_tool_call_id, child_dialog_turn_id, status, error_code, error_message, execution_owner_token, created_at_ms, terminal_at_ms, NULL, NULL FROM background_tasks ORDER BY task_pk"
    };
    let tasks = query_rows(&connection, &mut unreadable_rows, path, task_sql, |row| {
        Ok(BackgroundTaskRow {
            task_pk: row.get(0)?,
            parent_session_id: row.get(1)?,
            agent_pk: row.get(2)?,
            bg_task_id: row.get(3)?,
            bg_ordinal: row.get(4)?,
            parent_dialog_turn_id: row.get(5)?,
            parent_tool_call_id: row.get(6)?,
            child_dialog_turn_id: row.get(7)?,
            status: row.get(8)?,
            error_code: row.get(9)?,
            error_message: row.get(10)?,
            execution_owner_token: row.get(11)?,
            created_at_ms: row.get(12)?,
            terminal_at_ms: row.get(13)?,
            delivered_at_ms: row.get(14)?,
            delivered_parent_dialog_turn_id: row.get(15)?,
        })
    })?;
    let (swarm_trees, swarm_nodes) = if version >= 2 {
        (
            query_rows(
                &connection,
                &mut unreadable_rows,
                path,
                "SELECT root_session_id, created_at_ms FROM swarm_trees ORDER BY root_session_id",
                |row| {
                    Ok(SwarmTreeRow {
                        root_session_id: row.get(0)?,
                        created_at_ms: row.get(1)?,
                    })
                },
            )?,
            query_rows(
                &connection,
                &mut unreadable_rows,
                path,
                "SELECT session_id, root_session_id, parent_session_id, agent_type, depth, created_at_ms FROM swarm_nodes ORDER BY depth, session_id",
                |row| {
                    Ok(SwarmNodeRow {
                        session_id: row.get(0)?,
                        root_session_id: row.get(1)?,
                        parent_session_id: row.get(2)?,
                        agent_type: row.get(3)?,
                        depth: row.get(4)?,
                        created_at_ms: row.get(5)?,
                    })
                },
            )?,
        )
    } else {
        (Vec::new(), Vec::new())
    };
    let data = CoordinationData {
        sessions,
        agents,
        tasks,
        swarm_trees,
        swarm_nodes,
        unreadable_rows,
    };
    Ok(data)
}

fn query_rows<T, F>(
    connection: &Connection,
    unreadable_rows: &mut Vec<String>,
    path: &Path,
    sql: &str,
    mut mapper: F,
) -> LegacyMigrationResult<Vec<T>>
where
    F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
{
    let mut statement = connection
        .prepare(sql)
        .map_err(|error| db_error(path, error))?;
    let mut rows = statement.query([]).map_err(|error| db_error(path, error))?;
    let mut parsed = Vec::new();
    while let Some(row) = rows.next().map_err(|error| db_error(path, error))? {
        match mapper(row) {
            Ok(value) => parsed.push(value),
            Err(_) => {
                let mut digest = Sha256::new();
                digest.update(sql.as_bytes());
                for column in 0..row.as_ref().column_count() {
                    let value = row.get_ref(column).map_err(|error| db_error(path, error))?;
                    digest.update(format!("{value:?}").as_bytes());
                    digest.update([0]);
                }
                unreadable_rows.push(hex::encode(digest.finalize()));
            }
        }
    }
    Ok(parsed)
}

fn validate_required_tables(
    connection: &Connection,
    path: &Path,
    version: i64,
) -> LegacyMigrationResult<()> {
    let mut expected = vec![
        (
            "coordination_sessions",
            vec!["parent_session_id", "next_auto_agent_seq", "updated_at_ms"],
        ),
        (
            "agents",
            vec![
                "agent_pk",
                "parent_session_id",
                "agent_id",
                "child_session_id",
                "next_bg_seq",
                "state",
                "created_at_ms",
            ],
        ),
        (
            "background_tasks",
            vec![
                "task_pk",
                "parent_session_id",
                "agent_pk",
                "bg_task_id",
                "bg_ordinal",
                "parent_dialog_turn_id",
                "parent_tool_call_id",
                "child_dialog_turn_id",
                "status",
                "error_code",
                "error_message",
                "execution_owner_token",
                "created_at_ms",
                "terminal_at_ms",
            ],
        ),
    ];
    if version >= 2 {
        expected.extend([
            ("swarm_trees", vec!["root_session_id", "created_at_ms"]),
            (
                "swarm_nodes",
                vec![
                    "session_id",
                    "root_session_id",
                    "parent_session_id",
                    "agent_type",
                    "depth",
                    "created_at_ms",
                ],
            ),
        ]);
    }
    for (table, columns) in expected {
        let actual = table_columns(connection, path, table)?;
        if actual.is_empty() || columns.iter().any(|column| !actual.contains(*column)) {
            return Err(LegacyMigrationError::UnsupportedSource(format!(
                "Agent coordination table {table} does not match the supported schema"
            )));
        }
    }
    Ok(())
}

fn table_columns(
    connection: &Connection,
    path: &Path,
    table: &str,
) -> LegacyMigrationResult<HashSet<String>> {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|error| db_error(path, error))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| db_error(path, error))?;
    rows.collect::<rusqlite::Result<HashSet<_>>>()
        .map_err(|error| db_error(path, error))
}

fn merge_coordination_database(
    source_path: &Path,
    target_path: &Path,
    blocked_sessions: &BTreeSet<String>,
) -> LegacyMigrationResult<MergeOutcome> {
    let source = load_coordination_data(source_path, DatabaseRole::StagedCurrent)?;
    let mut connection =
        Connection::open(target_path).map_err(|error| db_error(target_path, error))?;
    initialize_coordination_schema(&connection)
        .map_err(|error| owner_error("initialize target coordination database", error))?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|error| db_error(target_path, error))?;
    let transaction = connection
        .transaction()
        .map_err(|error| db_error(target_path, error))?;
    let mut outcome = MergeOutcome {
        incompatible_rows: source.unreadable_rows.len() as u64,
        ..Default::default()
    };
    merge_coordination_sessions(
        &transaction,
        &source.sessions,
        blocked_sessions,
        &mut outcome,
        target_path,
    )?;
    let agent_map = merge_agents(
        &transaction,
        &source.agents,
        blocked_sessions,
        &mut outcome,
        target_path,
    )?;
    merge_background_tasks(
        &transaction,
        &source.tasks,
        &agent_map,
        blocked_sessions,
        &mut outcome,
        target_path,
    )?;
    merge_swarm_trees(
        &transaction,
        &source.swarm_trees,
        blocked_sessions,
        &mut outcome,
        target_path,
    )?;
    merge_swarm_nodes(
        &transaction,
        &source.swarm_nodes,
        blocked_sessions,
        &mut outcome,
        target_path,
    )?;
    transaction
        .commit()
        .map_err(|error| db_error(target_path, error))?;
    Ok(outcome)
}

fn merge_coordination_sessions(
    transaction: &Transaction<'_>,
    rows: &[CoordinationSessionRow],
    blocked_sessions: &BTreeSet<String>,
    outcome: &mut MergeOutcome,
    path: &Path,
) -> LegacyMigrationResult<()> {
    for row in rows {
        if blocked_sessions.contains(&row.parent_session_id) {
            record_target_win(
                outcome,
                "coordination_session_data_target_wins",
                &row.parent_session_id,
            );
            continue;
        }
        let existing = transaction
            .query_row(
                "SELECT parent_session_id, next_auto_agent_seq, updated_at_ms FROM coordination_sessions WHERE parent_session_id = ?1",
                params![row.parent_session_id],
                |record| Ok((|| -> rusqlite::Result<CoordinationSessionRow> { Ok(CoordinationSessionRow {
                    parent_session_id: record.get(0)?,
                    next_auto_agent_seq: record.get(1)?,
                    updated_at_ms: record.get(2)?,
                }) })()),
            )
            .optional()
            .map_err(|error| db_error(path, error))?;
        match existing {
            Some(Ok(existing)) if existing == *row => outcome.duplicate += 1,
            Some(_) => record_target_win(
                outcome,
                "coordination_session_target_wins",
                &row.parent_session_id,
            ),
            None => {
                if !insert_compatible_row(transaction.execute(
                    "INSERT INTO coordination_sessions (parent_session_id, next_auto_agent_seq, updated_at_ms) VALUES (?1, ?2, ?3)",
                    params![row.parent_session_id, row.next_auto_agent_seq, row.updated_at_ms],
                ), outcome, path)? { continue; }
                outcome.imported += 1;
            }
        }
    }
    Ok(())
}

fn merge_agents(
    transaction: &Transaction<'_>,
    rows: &[AgentRow],
    blocked_sessions: &BTreeSet<String>,
    outcome: &mut MergeOutcome,
    path: &Path,
) -> LegacyMigrationResult<HashMap<i64, AgentMergeTarget>> {
    let mut mapping = HashMap::new();
    for row in rows {
        if blocked_sessions.contains(&row.parent_session_id)
            || row
                .child_session_id
                .as_ref()
                .is_some_and(|session_id| blocked_sessions.contains(session_id))
        {
            mapping.insert(row.agent_pk, AgentMergeTarget::Rejected);
            record_target_win(
                outcome,
                "coordination_agent_session_target_wins",
                &format!("{}:{}", row.parent_session_id, row.agent_id),
            );
            continue;
        }
        let mut candidates = BTreeSet::new();
        if let Some(pk) = transaction
            .query_row(
                "SELECT agent_pk FROM agents WHERE parent_session_id = ?1 AND agent_id = ?2",
                params![row.parent_session_id, row.agent_id],
                |record| record.get::<_, i64>(0),
            )
            .optional()
            .map_err(|error| db_error(path, error))?
        {
            candidates.insert(pk);
        }
        if let Some(child) = row.child_session_id.as_deref() {
            if let Some(pk) = transaction.query_row(
                "SELECT agent_pk FROM agents WHERE parent_session_id = ?1 AND child_session_id = ?2",
                params![row.parent_session_id, child],
                |record| record.get::<_, i64>(0),
            ).optional().map_err(|error| db_error(path, error))? {
                candidates.insert(pk);
            }
        }
        if candidates.is_empty() {
            if !insert_compatible_row(transaction.execute(
                "INSERT INTO agents (parent_session_id, agent_id, child_session_id, next_bg_seq, state, created_at_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![row.parent_session_id, row.agent_id, row.child_session_id, row.next_bg_seq, row.state, row.created_at_ms],
            ), outcome, path)? { continue; }
            let pk = transaction.last_insert_rowid();
            mapping.insert(row.agent_pk, AgentMergeTarget::Imported(pk));
            outcome.imported += 1;
            continue;
        }
        if candidates.len() == 1 {
            let pk = *candidates.iter().next().expect("one candidate exists");
            if load_agent_by_pk(transaction, path, pk)
                .is_ok_and(|existing| agent_logically_equal(&existing, row))
            {
                mapping.insert(row.agent_pk, AgentMergeTarget::Existing(pk));
                outcome.duplicate += 1;
                continue;
            }
        }
        mapping.insert(row.agent_pk, AgentMergeTarget::Rejected);
        record_target_win(
            outcome,
            "coordination_agent_target_wins",
            &format!("{}:{}", row.parent_session_id, row.agent_id),
        );
    }
    Ok(mapping)
}

fn merge_background_tasks(
    transaction: &Transaction<'_>,
    rows: &[BackgroundTaskRow],
    agent_map: &HashMap<i64, AgentMergeTarget>,
    blocked_sessions: &BTreeSet<String>,
    outcome: &mut MergeOutcome,
    path: &Path,
) -> LegacyMigrationResult<()> {
    for row in rows {
        if blocked_sessions.contains(&row.parent_session_id) {
            record_target_win(
                outcome,
                "coordination_task_session_target_wins",
                &row.bg_task_id,
            );
            continue;
        }
        let Some(agent_target) = agent_map.get(&row.agent_pk).copied() else {
            outcome.incompatible_rows += 1;
            continue;
        };
        let Some(agent_pk) = agent_target.agent_pk() else {
            record_target_win(
                outcome,
                "coordination_task_agent_target_wins",
                &row.bg_task_id,
            );
            continue;
        };
        let by_id = transaction.query_row(
            "SELECT task_pk FROM background_tasks WHERE parent_session_id = ?1 AND bg_task_id = ?2",
            params![row.parent_session_id, row.bg_task_id],
            |record| record.get::<_, i64>(0),
        ).optional().map_err(|error| db_error(path, error))?;
        let by_ordinal = transaction
            .query_row(
                "SELECT task_pk FROM background_tasks WHERE agent_pk = ?1 AND bg_ordinal = ?2",
                params![agent_pk, row.bg_ordinal],
                |record| record.get::<_, i64>(0),
            )
            .optional()
            .map_err(|error| db_error(path, error))?;
        let candidates = [by_id, by_ordinal]
            .into_iter()
            .flatten()
            .collect::<BTreeSet<_>>();
        if candidates.is_empty() {
            if !insert_compatible_row(transaction.execute(
                "INSERT INTO background_tasks (parent_session_id, agent_pk, bg_task_id, bg_ordinal, parent_dialog_turn_id, parent_tool_call_id, child_dialog_turn_id, status, error_code, error_message, execution_owner_token, created_at_ms, terminal_at_ms, delivered_at_ms, delivered_parent_dialog_turn_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                params![row.parent_session_id, agent_pk, row.bg_task_id, row.bg_ordinal, row.parent_dialog_turn_id, row.parent_tool_call_id, row.child_dialog_turn_id, row.status, row.error_code, row.error_message, row.execution_owner_token, row.created_at_ms, row.terminal_at_ms, row.delivered_at_ms, row.delivered_parent_dialog_turn_id],
            ), outcome, path)? { continue; }
            outcome.imported += 1;
        } else if candidates.len() == 1 {
            let existing = load_task_by_pk(
                transaction,
                path,
                *candidates.iter().next().expect("one candidate exists"),
            );
            if existing.is_ok_and(|existing| task_logically_equal(&existing, row, agent_pk)) {
                outcome.duplicate += 1;
            } else {
                record_target_win(outcome, "coordination_task_target_wins", &row.bg_task_id);
            }
        } else {
            record_target_win(outcome, "coordination_task_target_wins", &row.bg_task_id);
        }
    }
    Ok(())
}

fn merge_swarm_trees(
    transaction: &Transaction<'_>,
    rows: &[SwarmTreeRow],
    blocked_sessions: &BTreeSet<String>,
    outcome: &mut MergeOutcome,
    path: &Path,
) -> LegacyMigrationResult<()> {
    for row in rows {
        if blocked_sessions.contains(&row.root_session_id) {
            record_target_win(
                outcome,
                "coordination_swarm_tree_session_target_wins",
                &row.root_session_id,
            );
            continue;
        }
        let existing = transaction
            .query_row(
                "SELECT root_session_id, created_at_ms FROM swarm_trees WHERE root_session_id = ?1",
                params![row.root_session_id],
                |record| {
                    Ok((|| -> rusqlite::Result<SwarmTreeRow> {
                        Ok(SwarmTreeRow {
                            root_session_id: record.get(0)?,
                            created_at_ms: record.get(1)?,
                        })
                    })())
                },
            )
            .optional()
            .map_err(|error| db_error(path, error))?;
        match existing {
            Some(Ok(existing)) if existing == *row => outcome.duplicate += 1,
            Some(_) => record_target_win(
                outcome,
                "coordination_swarm_tree_target_wins",
                &row.root_session_id,
            ),
            None => {
                if !insert_compatible_row(
                    transaction.execute(
                        "INSERT INTO swarm_trees (root_session_id, created_at_ms) VALUES (?1, ?2)",
                        params![row.root_session_id, row.created_at_ms],
                    ),
                    outcome,
                    path,
                )? {
                    continue;
                }
                outcome.imported += 1;
            }
        }
    }
    Ok(())
}

fn merge_swarm_nodes(
    transaction: &Transaction<'_>,
    rows: &[SwarmNodeRow],
    blocked_sessions: &BTreeSet<String>,
    outcome: &mut MergeOutcome,
    path: &Path,
) -> LegacyMigrationResult<()> {
    for row in rows {
        if blocked_sessions.contains(&row.session_id)
            || blocked_sessions.contains(&row.root_session_id)
            || row
                .parent_session_id
                .as_ref()
                .is_some_and(|session_id| blocked_sessions.contains(session_id))
        {
            record_target_win(
                outcome,
                "coordination_swarm_node_session_target_wins",
                &row.session_id,
            );
            continue;
        }
        let existing = transaction.query_row(
            "SELECT session_id, root_session_id, parent_session_id, agent_type, depth, created_at_ms FROM swarm_nodes WHERE session_id = ?1",
            params![row.session_id],
            |record| Ok((|| -> rusqlite::Result<SwarmNodeRow> { Ok(SwarmNodeRow { session_id: record.get(0)?, root_session_id: record.get(1)?, parent_session_id: record.get(2)?, agent_type: record.get(3)?, depth: record.get(4)?, created_at_ms: record.get(5)? }) })()),
        ).optional().map_err(|error| db_error(path, error))?;
        match existing {
            Some(Ok(existing)) if existing == *row => outcome.duplicate += 1,
            Some(_) => record_target_win(
                outcome,
                "coordination_swarm_node_target_wins",
                &row.session_id,
            ),
            None => {
                if !insert_compatible_row(transaction.execute(
                    "INSERT INTO swarm_nodes (session_id, root_session_id, parent_session_id, agent_type, depth, created_at_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![row.session_id, row.root_session_id, row.parent_session_id, row.agent_type, row.depth, row.created_at_ms],
                ), outcome, path)? { continue; }
                outcome.imported += 1;
            }
        }
    }
    Ok(())
}

fn insert_compatible_row(
    result: rusqlite::Result<usize>,
    outcome: &mut MergeOutcome,
    path: &Path,
) -> LegacyMigrationResult<bool> {
    match result {
        Ok(_) => Ok(true),
        Err(rusqlite::Error::SqliteFailure(error, _))
            if error.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            outcome.incompatible_rows += 1;
            Ok(false)
        }
        Err(error) => Err(db_error(path, error)),
    }
}

fn load_agent_by_pk(
    transaction: &Transaction<'_>,
    path: &Path,
    agent_pk: i64,
) -> LegacyMigrationResult<AgentRow> {
    transaction.query_row(
        "SELECT agent_pk, parent_session_id, agent_id, child_session_id, next_bg_seq, state, created_at_ms FROM agents WHERE agent_pk = ?1",
        params![agent_pk],
        |record| Ok(AgentRow { agent_pk: record.get(0)?, parent_session_id: record.get(1)?, agent_id: record.get(2)?, child_session_id: record.get(3)?, next_bg_seq: record.get(4)?, state: record.get(5)?, created_at_ms: record.get(6)? }),
    ).map_err(|error| db_error(path, error))
}

fn load_task_by_pk(
    transaction: &Transaction<'_>,
    path: &Path,
    task_pk: i64,
) -> LegacyMigrationResult<BackgroundTaskRow> {
    transaction.query_row(
        "SELECT task_pk, parent_session_id, agent_pk, bg_task_id, bg_ordinal, parent_dialog_turn_id, parent_tool_call_id, child_dialog_turn_id, status, error_code, error_message, execution_owner_token, created_at_ms, terminal_at_ms, delivered_at_ms, delivered_parent_dialog_turn_id FROM background_tasks WHERE task_pk = ?1",
        params![task_pk],
        |record| Ok(BackgroundTaskRow { task_pk: record.get(0)?, parent_session_id: record.get(1)?, agent_pk: record.get(2)?, bg_task_id: record.get(3)?, bg_ordinal: record.get(4)?, parent_dialog_turn_id: record.get(5)?, parent_tool_call_id: record.get(6)?, child_dialog_turn_id: record.get(7)?, status: record.get(8)?, error_code: record.get(9)?, error_message: record.get(10)?, execution_owner_token: record.get(11)?, created_at_ms: record.get(12)?, terminal_at_ms: record.get(13)?, delivered_at_ms: record.get(14)?, delivered_parent_dialog_turn_id: record.get(15)? }),
    ).map_err(|error| db_error(path, error))
}

fn agent_logically_equal(left: &AgentRow, right: &AgentRow) -> bool {
    left.parent_session_id == right.parent_session_id
        && left.agent_id == right.agent_id
        && left.child_session_id == right.child_session_id
        && left.next_bg_seq == right.next_bg_seq
        && left.state == right.state
        && left.created_at_ms == right.created_at_ms
}

fn task_logically_equal(
    left: &BackgroundTaskRow,
    right: &BackgroundTaskRow,
    agent_pk: i64,
) -> bool {
    left.parent_session_id == right.parent_session_id
        && left.agent_pk == agent_pk
        && left.bg_task_id == right.bg_task_id
        && left.bg_ordinal == right.bg_ordinal
        && left.parent_dialog_turn_id == right.parent_dialog_turn_id
        && left.parent_tool_call_id == right.parent_tool_call_id
        && left.child_dialog_turn_id == right.child_dialog_turn_id
        && left.status == right.status
        && left.error_code == right.error_code
        && left.error_message == right.error_message
        && left.execution_owner_token == right.execution_owner_token
        && left.created_at_ms == right.created_at_ms
        && left.terminal_at_ms == right.terminal_at_ms
        && left.delivered_at_ms == right.delivered_at_ms
        && left.delivered_parent_dialog_turn_id == right.delivered_parent_dialog_turn_id
}

fn preview_conflicts(
    source: &CoordinationData,
    target: &CoordinationData,
    blocked_sessions: &BTreeSet<String>,
) -> Vec<MigrationConflict> {
    let mut conflicts = Vec::new();
    let target_sessions = target
        .sessions
        .iter()
        .map(|row| (&row.parent_session_id, row))
        .collect::<HashMap<_, _>>();
    for row in &source.sessions {
        if blocked_sessions.contains(&row.parent_session_id) {
            conflicts.push(conflict(
                "coordination_session_data_target_wins",
                &row.parent_session_id,
            ));
        } else if target_sessions
            .get(&row.parent_session_id)
            .is_some_and(|existing| *existing != row)
        {
            conflicts.push(conflict(
                "coordination_session_target_wins",
                &row.parent_session_id,
            ));
        }
    }

    let mut agent_map = HashMap::new();
    for row in &source.agents {
        if blocked_sessions.contains(&row.parent_session_id)
            || row
                .child_session_id
                .as_ref()
                .is_some_and(|session_id| blocked_sessions.contains(session_id))
        {
            agent_map.insert(row.agent_pk, AgentMergeTarget::Rejected);
            conflicts.push(conflict(
                "coordination_agent_session_target_wins",
                &format!("{}:{}", row.parent_session_id, row.agent_id),
            ));
            continue;
        }

        let candidates = target
            .agents
            .iter()
            .filter(|existing| {
                existing.parent_session_id == row.parent_session_id
                    && (existing.agent_id == row.agent_id
                        || (row.child_session_id.is_some()
                            && existing.child_session_id == row.child_session_id))
            })
            .collect::<Vec<_>>();
        match candidates.as_slice() {
            [] => {
                agent_map.insert(row.agent_pk, AgentMergeTarget::Imported(row.agent_pk));
            }
            [existing] if agent_logically_equal(existing, row) => {
                agent_map.insert(row.agent_pk, AgentMergeTarget::Existing(existing.agent_pk));
            }
            _ => {
                agent_map.insert(row.agent_pk, AgentMergeTarget::Rejected);
                conflicts.push(conflict(
                    "coordination_agent_target_wins",
                    &format!("{}:{}", row.parent_session_id, row.agent_id),
                ));
            }
        }
    }

    for row in &source.tasks {
        if blocked_sessions.contains(&row.parent_session_id) {
            conflicts.push(conflict(
                "coordination_task_session_target_wins",
                &row.bg_task_id,
            ));
            continue;
        }
        let Some(agent_target) = agent_map.get(&row.agent_pk).copied() else {
            continue;
        };
        let agent_pk = match agent_target {
            AgentMergeTarget::Rejected => {
                conflicts.push(conflict(
                    "coordination_task_agent_target_wins",
                    &row.bg_task_id,
                ));
                continue;
            }
            AgentMergeTarget::Imported(_) => None,
            AgentMergeTarget::Existing(agent_pk) => Some(agent_pk),
        };
        let candidates = target
            .tasks
            .iter()
            .filter(|existing| {
                (existing.parent_session_id == row.parent_session_id
                    && existing.bg_task_id == row.bg_task_id)
                    || agent_pk.is_some_and(|agent_pk| {
                        existing.agent_pk == agent_pk && existing.bg_ordinal == row.bg_ordinal
                    })
            })
            .collect::<Vec<_>>();
        let duplicate = match (candidates.as_slice(), agent_pk) {
            ([existing], Some(agent_pk)) => task_logically_equal(existing, row, agent_pk),
            _ => false,
        };
        if !candidates.is_empty() && !duplicate {
            conflicts.push(conflict("coordination_task_target_wins", &row.bg_task_id));
        }
    }

    let target_trees = target
        .swarm_trees
        .iter()
        .map(|row| (&row.root_session_id, row))
        .collect::<HashMap<_, _>>();
    for row in &source.swarm_trees {
        if blocked_sessions.contains(&row.root_session_id) {
            conflicts.push(conflict(
                "coordination_swarm_tree_session_target_wins",
                &row.root_session_id,
            ));
        } else if target_trees
            .get(&row.root_session_id)
            .is_some_and(|existing| *existing != row)
        {
            conflicts.push(conflict(
                "coordination_swarm_tree_target_wins",
                &row.root_session_id,
            ));
        }
    }

    let target_nodes = target
        .swarm_nodes
        .iter()
        .map(|row| (&row.session_id, row))
        .collect::<HashMap<_, _>>();
    for row in &source.swarm_nodes {
        if blocked_sessions.contains(&row.session_id)
            || blocked_sessions.contains(&row.root_session_id)
            || row
                .parent_session_id
                .as_ref()
                .is_some_and(|session_id| blocked_sessions.contains(session_id))
        {
            conflicts.push(conflict(
                "coordination_swarm_node_session_target_wins",
                &row.session_id,
            ));
        } else if target_nodes
            .get(&row.session_id)
            .is_some_and(|existing| *existing != row)
        {
            conflicts.push(conflict(
                "coordination_swarm_node_target_wins",
                &row.session_id,
            ));
        }
    }
    conflicts
}

fn record_target_win(outcome: &mut MergeOutcome, code: &'static str, logical_id: &str) {
    outcome.target_wins = outcome.target_wins.saturating_add(1);
    outcome.conflicts.push(conflict(code, logical_id));
}

fn conflict(code: &'static str, logical_id: &str) -> MigrationConflict {
    MigrationConflict {
        domain: MigrationDomainId::AgentCoordination,
        code: code.to_string(),
        source_summary: format!("legacy Agent coordination record {logical_id}"),
        target_summary: format!("current Agent coordination record {logical_id}"),
        resolution: ConflictResolution::TargetWins,
    }
}

fn verify_target_state(path: &Path, manifest: &CoordinationManifest) -> LegacyMigrationResult<()> {
    if path.exists() != manifest.target_existed {
        return Err(LegacyMigrationError::InvalidRequest(format!(
            "target changed after staging: {}",
            path.display()
        )));
    }
    if let Some(expected) = manifest.target_digest.as_deref() {
        let actual =
            coordination_digest(&load_coordination_data(path, DatabaseRole::CurrentTarget)?)?;
        if actual != expected {
            return Err(LegacyMigrationError::InvalidRequest(format!(
                "target changed after staging: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn initialize_snapshot(path: &Path) -> LegacyMigrationResult<()> {
    let connection = Connection::open(path).map_err(|error| db_error(path, error))?;
    initialize_coordination_schema(&connection)
        .map_err(|error| owner_error("upgrade staged coordination snapshot", error))
}

fn finalize_sqlite_file(path: &Path) -> LegacyMigrationResult<()> {
    let connection = Connection::open(path).map_err(|error| db_error(path, error))?;
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;")
        .map_err(|error| db_error(path, error))?;
    drop(connection);
    remove_sqlite_sidecars(path)
}

fn validate_current_database(path: &Path) -> LegacyMigrationResult<()> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| db_error(path, error))?;
    let version = connection
        .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
        .map_err(|error| db_error(path, error))?;
    if version != COORDINATION_SCHEMA_VERSION {
        return Err(LegacyMigrationError::InvalidRequest(format!(
            "Agent coordination database is schema {version}, expected {COORDINATION_SCHEMA_VERSION}"
        )));
    }
    validate_required_tables(&connection, path, version)?;
    for column in ["delivered_at_ms", "delivered_parent_dialog_turn_id"] {
        if !coordination_table_has_column(&connection, "background_tasks", column)
            .map_err(|error| owner_error("validate coordination schema", error))?
        {
            return Err(LegacyMigrationError::InvalidRequest(format!(
                "Agent coordination database is missing {column}"
            )));
        }
    }
    Ok(())
}

fn schema_version(path: &Path) -> LegacyMigrationResult<i64> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| db_error(path, error))?;
    connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|error| db_error(path, error))
}

fn coordination_digest(data: &CoordinationData) -> LegacyMigrationResult<String> {
    serde_json::to_vec(data)
        .map(|bytes| format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
        .map_err(|error| {
            LegacyMigrationError::InvalidRequest(format!(
                "failed to hash Agent coordination data: {error}"
            ))
        })
}

fn coordination_entity_count(data: &CoordinationData) -> u64 {
    [
        data.sessions.len(),
        data.agents.len(),
        data.tasks.len(),
        data.swarm_trees.len(),
        data.swarm_nodes.len(),
    ]
    .into_iter()
    .fold(0u64, |total, count| total.saturating_add(count as u64))
}

fn manifest_target_wins_session_ids(manifest: &WorkspaceSessionsManifest) -> BTreeSet<String> {
    manifest
        .sessions
        .iter()
        .filter(|entry| entry.action == SessionImportAction::TargetWins)
        .map(|entry| entry.session_id.clone())
        .collect()
}

fn reset_stage_file(path: &Path) -> LegacyMigrationResult<()> {
    remove_file_if_present(path)?;
    remove_sqlite_sidecars(path)
}

fn remove_sqlite_sidecars(path: &Path) -> LegacyMigrationResult<()> {
    for suffix in ["-wal", "-shm"] {
        let sidecar = PathBuf::from(format!("{}{suffix}", path.display()));
        remove_file_if_present(&sidecar)?;
    }
    Ok(())
}

fn remove_file_if_present(path: &Path) -> LegacyMigrationResult<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(path, error)),
    }
}

fn owner_error(context: &str, error: impl std::fmt::Display) -> LegacyMigrationError {
    LegacyMigrationError::InvalidRequest(format!("{context}: {error}"))
}

fn db_error(path: &Path, error: rusqlite::Error) -> LegacyMigrationError {
    LegacyMigrationError::InvalidRequest(format!(
        "Agent coordination database error at {}: {error}",
        path.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters_for_groups;
    use openbitfun_legacy_migration::{
        probe_legacy_source, CancellationToken, CrashInjector, CrashPoint, MigrationEngine,
        NoCrashInjection, ProbeLimits,
    };
    use openbitfun_product_domains::legacy_migration::{
        MigrationGroupId, MigrationRunStatus, MigrationSelection,
    };
    use openbitfun_services_core::session::OfflineSessionImportStore;
    use openbitfun_services_core::session_projection_format::validate_runtime_event_log;
    use openbitfun_services_core::workspace_persistence::{
        validate_workspace_persistence_data, WorkspacePersistenceData,
    };
    use std::io::Read;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct CrashOnce {
        point: CrashPoint,
        fired: AtomicBool,
    }

    impl CrashInjector for CrashOnce {
        fn should_crash(&self, point: CrashPoint) -> bool {
            point == self.point && !self.fired.swap(true, Ordering::AcqRel)
        }
    }

    #[test]
    fn deep_assistant_content_does_not_skip_session_domains() {
        let temp = test_tempdir("deep-assistant-content");
        let roots = fixture_roots(temp.path());
        copy_fixture(&roots);
        let _connection = materialize_coordination(&roots, false);
        let mut deep = roots
            .legacy_home_root
            .join("personal_assistant/workspace/build");
        for _ in 0..20 {
            deep.push("d");
        }
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("content.txt"), b"preserve deep workspace content").unwrap();
        let selection = session_selection();
        let source = probe_legacy_source(&roots, ProbeLimits::default())
            .unwrap()
            .unwrap();
        let engine = MigrationEngine::new(roots.clone(), adapters_for_groups(&selection)).unwrap();
        let plan = engine
            .plan(&source, selection, &CancellationToken::default())
            .unwrap();
        assert!(plan.findings.iter().all(|finding| finding.migratable));
        let report = engine
            .execute(&plan, &CancellationToken::default(), &NoCrashInjection)
            .unwrap();
        assert!(report
            .domain_results
            .iter()
            .all(|result| result.state == MigrationDomainState::Verified));
        assert!(roots
            .target_home_root
            .join("projects/c--fixture-workspace/sessions/session-1/metadata.json")
            .is_file());
        assert!(roots
            .target_user_root
            .join(COORDINATION_RELATIVE_PATH)
            .is_file());
        assert!(deep.is_dir());
        let relative = deep.strip_prefix(&roots.legacy_home_root).unwrap();
        assert_eq!(
            fs::read(roots.target_home_root.join(relative).join("content.txt")).unwrap(),
            b"preserve deep workspace content"
        );
        assert_eq!(
            fs::read(deep.join("content.txt")).unwrap(),
            b"preserve deep workspace content"
        );
        assert!(!report
            .domain_results
            .iter()
            .flat_map(|r| &r.warnings)
            .any(|w| w.code == "assistant_workspace_not_migrated"));
    }

    #[test]
    fn session_group_migrates_owner_data_wal_and_relationship_closure() {
        let temp = test_tempdir("session-group");
        let roots = fixture_roots(temp.path());
        copy_fixture(&roots);
        let wal_connection = materialize_coordination(&roots, true);
        let source_hash = hash_source_roots(&roots);
        let selection = session_selection();
        assert_eq!(
            selection.expanded_domains(),
            [
                MigrationDomainId::WorkspaceSessions,
                MigrationDomainId::AgentCoordination,
                MigrationDomainId::CrossReferenceRepair,
            ]
        );

        let source = probe_legacy_source(&roots, ProbeLimits::default())
            .unwrap()
            .unwrap();
        let engine = MigrationEngine::new(roots.clone(), adapters_for_groups(&selection)).unwrap();
        let plan = engine
            .plan(&source, selection.clone(), &CancellationToken::default())
            .unwrap();
        let report = engine
            .execute(&plan, &CancellationToken::default(), &NoCrashInjection)
            .unwrap();
        assert_eq!(report.status, MigrationRunStatus::Completed);
        assert!(report
            .domain_results
            .iter()
            .all(|result| result.state == MigrationDomainState::Verified));

        let workspace_path = roots.target_user_root.join("data/workspace_data.json");
        let workspace: WorkspacePersistenceData =
            serde_json::from_slice(&fs::read(workspace_path).unwrap()).unwrap();
        validate_workspace_persistence_data(
            &workspace,
            &roots.target_user_root.join("data/miniapps"),
        )
        .unwrap();
        assert_eq!(workspace.workspaces.len(), 1);
        assert_eq!(
            workspace.current_workspace_id,
            workspace.opened_workspace_ids.first().cloned()
        );

        let sessions_root = roots
            .target_home_root
            .join("projects/c--fixture-workspace/sessions");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        let store = OfflineSessionImportStore::new(&sessions_root);
        let parent = runtime
            .block_on(store.load_bundle("session-1"))
            .unwrap()
            .unwrap();
        let child = runtime
            .block_on(store.load_bundle("session-child-1"))
            .unwrap()
            .unwrap();
        assert_eq!(parent.turns[0].turn_id, "turn-1");
        assert_eq!(
            child
                .metadata
                .relationship
                .as_ref()
                .and_then(|relationship| relationship.parent_session_id.as_deref()),
            Some("session-1")
        );
        assert!(!sessions_root
            .join("session-1/request-traces/trace.json")
            .exists());
        assert!(!roots
            .target_user_root
            .join("data/agent-runtime/ownership")
            .exists());
        assert!(!roots
            .target_user_root
            .join("data/agent-runtime/ipc-v17")
            .exists());

        let event_path = roots
            .target_home_root
            .join("runtime-events/session-1.jsonl");
        let event_summary = validate_runtime_event_log(&event_path, "session-1").unwrap();
        assert!(event_summary.turn_ids.contains("turn-runtime-1"));

        let target_coordination = target_coordination_path(&roots);
        let coordination =
            load_coordination_data(&target_coordination, DatabaseRole::CurrentTarget).unwrap();
        assert_eq!(coordination.sessions.len(), 2);
        assert!(coordination
            .sessions
            .iter()
            .any(|row| row.parent_session_id == "session-child-1"));
        assert_eq!(coordination.agents.len(), 1);
        assert_eq!(coordination.tasks.len(), 1);
        assert_eq!(coordination.swarm_nodes.len(), 2);
        assert!(!PathBuf::from(format!("{}-wal", target_coordination.display())).exists());
        assert!(!PathBuf::from(format!("{}-shm", target_coordination.display())).exists());
        let report_json = serde_json::to_string(&report).unwrap();
        assert!(!report_json.contains("Synthetic parent request"));
        assert!(!report_json.contains("Synthetic delegated request"));
        assert_eq!(hash_source_roots(&roots), source_hash);

        let second_source = probe_legacy_source(&roots, ProbeLimits::default())
            .unwrap()
            .unwrap();
        let second_plan = engine
            .plan(&second_source, selection, &CancellationToken::default())
            .unwrap();
        engine
            .execute(
                &second_plan,
                &CancellationToken::default(),
                &NoCrashInjection,
            )
            .unwrap();
        let repeated =
            load_coordination_data(&target_coordination, DatabaseRole::CurrentTarget).unwrap();
        assert_eq!(
            coordination_entity_count(&repeated),
            coordination_entity_count(&coordination)
        );
        assert_eq!(hash_source_roots(&roots), source_hash);
        drop(wal_connection);
    }

    #[test]
    fn orphaned_session_relationship_is_preserved_without_blocking_import() {
        let temp = test_tempdir("orphaned-session-relationship");
        let roots = fixture_roots(temp.path());
        copy_fixture(&roots);
        let metadata_path = roots
            .legacy_home_root
            .join("projects/c--fixture-workspace/sessions/session-1/metadata.json");
        let mut metadata: serde_json::Value =
            serde_json::from_slice(&fs::read(&metadata_path).unwrap()).unwrap();
        metadata["relationship"] = serde_json::json!({
            "kind": "subagent",
            "parentSessionId": "missing-parent-session",
            "parentDialogTurnId": "missing-parent-turn"
        });
        atomic_write_json(&metadata_path, &metadata).unwrap();
        drop(materialize_coordination(&roots, false));
        let source_hash = hash_source_roots(&roots);
        let selection = session_selection();
        let source = probe_legacy_source(&roots, ProbeLimits::default())
            .unwrap()
            .unwrap();
        let engine = MigrationEngine::new(roots.clone(), adapters_for_groups(&selection)).unwrap();
        let plan = engine
            .plan(&source, selection, &CancellationToken::default())
            .unwrap();

        let report = engine
            .execute(&plan, &CancellationToken::default(), &NoCrashInjection)
            .unwrap();

        assert_eq!(report.status, MigrationRunStatus::Completed);
        let sessions_result = report
            .domain_results
            .iter()
            .find(|result| result.domain == MigrationDomainId::WorkspaceSessions)
            .unwrap();
        assert_eq!(sessions_result.state, MigrationDomainState::Verified);
        assert!(sessions_result
            .warnings
            .iter()
            .any(|warning| warning.code == "session_parent_not_present"
                && warning.severity == FindingSeverity::Info));

        let sessions_root = roots
            .target_home_root
            .join("projects/c--fixture-workspace/sessions");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        let imported = runtime
            .block_on(OfflineSessionImportStore::new(sessions_root).load_bundle("session-1"))
            .unwrap()
            .unwrap();
        assert_eq!(
            imported
                .metadata
                .relationship
                .as_ref()
                .and_then(|relationship| relationship.parent_session_id.as_deref()),
            Some("missing-parent-session")
        );
        assert_eq!(hash_source_roots(&roots), source_hash);
    }

    #[test]
    fn missing_or_unknown_coordination_schema_skips_only_coordination_and_dependents() {
        let temp = test_tempdir("coordination-schema");
        let roots = fixture_roots(temp.path());
        copy_fixture(&roots);
        let selection = session_selection();
        let engine = MigrationEngine::new(roots.clone(), adapters_for_groups(&selection)).unwrap();
        let source = probe_legacy_source(&roots, ProbeLimits::default())
            .unwrap()
            .unwrap();
        let missing = engine.plan(&source, selection.clone(), &CancellationToken::default());
        assert!(missing
            .unwrap()
            .findings
            .iter()
            .any(|finding| !finding.migratable
                && finding
                    .detail
                    .contains("requires a readable legacy coordination.sqlite")));

        let connection = materialize_coordination(&roots, false);
        connection.pragma_update(None, "user_version", 99).unwrap();
        drop(connection);
        let source = probe_legacy_source(&roots, ProbeLimits::default())
            .unwrap()
            .unwrap();
        let unknown = engine.plan(&source, selection, &CancellationToken::default());
        assert!(unknown
            .unwrap()
            .findings
            .iter()
            .any(|finding| !finding.migratable
                && finding.detail.contains("schema 99 is not supported")));
    }

    #[test]
    fn target_session_and_coordination_records_win_without_cross_attaching_source_tasks() {
        let temp = test_tempdir("coordination-target-wins");
        let roots = fixture_roots(temp.path());
        copy_fixture(&roots);
        let source_connection = materialize_coordination(&roots, false);
        drop(source_connection);

        let source_parent = roots
            .legacy_home_root
            .join("projects/c--fixture-workspace/sessions/session-1");
        let target_parent = roots
            .target_home_root
            .join("projects/c--fixture-workspace/sessions/session-1");
        copy_directory(&source_parent, &target_parent).unwrap();
        let metadata_path = target_parent.join("metadata.json");
        let mut metadata: serde_json::Value =
            serde_json::from_slice(&fs::read(&metadata_path).unwrap()).unwrap();
        metadata["sessionName"] = serde_json::Value::String("Current target Session".to_string());
        atomic_write_json(&metadata_path, &metadata).unwrap();

        let target_coordination = target_coordination_path(&roots);
        fs::create_dir_all(target_coordination.parent().unwrap()).unwrap();
        let target_connection = Connection::open(&target_coordination).unwrap();
        initialize_coordination_schema(&target_connection).unwrap();
        target_connection.execute(
            "INSERT INTO coordination_sessions (parent_session_id, next_auto_agent_seq, updated_at_ms) VALUES ('session-1', 99, 99)",
            [],
        ).unwrap();
        drop(target_connection);

        let selection = session_selection();
        let source = probe_legacy_source(&roots, ProbeLimits::default())
            .unwrap()
            .unwrap();
        let engine = MigrationEngine::new(roots.clone(), adapters_for_groups(&selection)).unwrap();
        let plan = engine
            .plan(&source, selection, &CancellationToken::default())
            .unwrap();
        let coordination_conflicts = plan
            .conflicts
            .iter()
            .filter(|entry| entry.domain == MigrationDomainId::AgentCoordination)
            .collect::<Vec<_>>();
        assert_eq!(coordination_conflicts.len(), 6);
        assert!(coordination_conflicts
            .iter()
            .any(|entry| entry.code == "coordination_session_data_target_wins"));
        assert!(coordination_conflicts
            .iter()
            .any(|entry| entry.code == "coordination_agent_session_target_wins"));
        assert!(coordination_conflicts
            .iter()
            .any(|entry| entry.code == "coordination_task_session_target_wins"));
        assert!(coordination_conflicts
            .iter()
            .any(|entry| entry.code == "coordination_swarm_tree_session_target_wins"));
        assert_eq!(
            coordination_conflicts
                .iter()
                .filter(|entry| entry.code == "coordination_swarm_node_session_target_wins")
                .count(),
            2
        );
        let report = engine
            .execute(&plan, &CancellationToken::default(), &NoCrashInjection)
            .unwrap();
        assert_eq!(
            report
                .domain_results
                .iter()
                .find(|result| result.domain == MigrationDomainId::AgentCoordination)
                .unwrap()
                .conflicts,
            coordination_conflicts.len() as u64
        );

        let target =
            load_coordination_data(&target_coordination, DatabaseRole::CurrentTarget).unwrap();
        assert_eq!(target.sessions.len(), 1);
        assert_eq!(target.sessions[0].next_auto_agent_seq, 99);
        assert!(target.agents.is_empty());
        assert!(target.tasks.is_empty());
        assert!(target.swarm_trees.is_empty());
        assert!(target.swarm_nodes.is_empty());
        let stored: serde_json::Value =
            serde_json::from_slice(&fs::read(metadata_path).unwrap()).unwrap();
        assert_eq!(stored["sessionName"], "Current target Session");
    }

    #[test]
    fn preview_reports_task_and_swarm_conflicts_before_stage() {
        let temp = test_tempdir("coordination-conflict-preview");
        let roots = fixture_roots(temp.path());
        copy_fixture(&roots);
        let connection = materialize_coordination(&roots, false);
        drop(connection);
        let source = load_coordination_data(
            &source_coordination_path(&roots),
            DatabaseRole::LegacySource,
        )
        .unwrap();
        let mut target = source.clone();
        target.tasks[0].status = "failed".to_string();
        target.swarm_trees[0].created_at_ms += 1;
        target.swarm_nodes[1].created_at_ms += 1;

        let conflicts = preview_conflicts(&source, &target, &BTreeSet::new());
        assert_eq!(conflicts.len(), 3);
        assert!(conflicts
            .iter()
            .any(|entry| entry.code == "coordination_task_target_wins"));
        assert!(conflicts
            .iter()
            .any(|entry| entry.code == "coordination_swarm_tree_target_wins"));
        assert!(conflicts
            .iter()
            .any(|entry| entry.code == "coordination_swarm_node_target_wins"));
    }

    #[test]
    fn committed_session_domains_resume_idempotently_after_crash() {
        for (label, crash_point) in [
            (
                "workspace-commit-recovery",
                CrashPoint::AfterCommit(MigrationDomainId::WorkspaceSessions),
            ),
            (
                "coordination-commit-recovery",
                CrashPoint::AfterCommit(MigrationDomainId::AgentCoordination),
            ),
        ] {
            let temp = test_tempdir(label);
            let roots = fixture_roots(temp.path());
            copy_fixture(&roots);
            let connection = materialize_coordination(&roots, false);
            drop(connection);
            let source_hash = hash_source_roots(&roots);
            let selection = session_selection();
            let source = probe_legacy_source(&roots, ProbeLimits::default())
                .unwrap()
                .unwrap();
            let engine =
                MigrationEngine::new(roots.clone(), adapters_for_groups(&selection)).unwrap();
            let plan = engine
                .plan(&source, selection, &CancellationToken::default())
                .unwrap();
            let crash = CrashOnce {
                point: crash_point,
                fired: AtomicBool::new(false),
            };

            let error = engine
                .execute(&plan, &CancellationToken::default(), &crash)
                .unwrap_err();
            assert!(matches!(
                error,
                LegacyMigrationError::InjectedCrash(actual) if actual == crash_point
            ));
            let report = engine
                .execute(&plan, &CancellationToken::default(), &NoCrashInjection)
                .unwrap();
            assert!(matches!(
                report.status,
                MigrationRunStatus::Completed | MigrationRunStatus::CompletedWithWarnings
            ));
            assert!(report
                .domain_results
                .iter()
                .all(|result| result.state == MigrationDomainState::Verified));
            assert_eq!(hash_source_roots(&roots), source_hash);
        }
    }

    #[test]
    fn missing_turn_is_imported_and_cancellation_keeps_the_source_unchanged() {
        let temp = test_tempdir("coordination-source-read-only");
        let roots = fixture_roots(temp.path());
        copy_fixture(&roots);
        let connection = materialize_coordination(&roots, false);
        connection
            .execute(
                "UPDATE background_tasks SET parent_dialog_turn_id = 'missing-turn'",
                [],
            )
            .unwrap();
        drop(connection);
        let source_hash = hash_source_roots(&roots);
        let selection = session_selection();
        let engine = MigrationEngine::new(roots.clone(), adapters_for_groups(&selection)).unwrap();
        let source = probe_legacy_source(&roots, ProbeLimits::default())
            .unwrap()
            .unwrap();
        let plan = engine
            .plan(&source, selection.clone(), &CancellationToken::default())
            .unwrap();
        let report = engine
            .execute(&plan, &CancellationToken::default(), &NoCrashInjection)
            .unwrap();
        assert_eq!(report.status, MigrationRunStatus::Completed);
        let imported = load_coordination_data(
            &target_coordination_path(&roots),
            DatabaseRole::CurrentTarget,
        )
        .unwrap();
        assert_eq!(imported.tasks[0].parent_dialog_turn_id, "missing-turn");
        assert_eq!(hash_source_roots(&roots), source_hash);

        let cancellation = CancellationToken::default();
        cancellation.cancel();
        let cancelled = engine.plan(&source, selection, &cancellation);
        assert!(matches!(cancelled, Err(LegacyMigrationError::Cancelled)));
        assert_eq!(hash_source_roots(&roots), source_hash);
    }

    #[test]
    fn historical_business_inconsistencies_and_bad_rows_do_not_block_merge() {
        let temp = test_tempdir("permissive-coordination");
        let roots = fixture_roots(temp.path());
        copy_fixture(&roots);
        let connection = materialize_coordination(&roots, false);
        connection.execute_batch(r#"
            UPDATE coordination_sessions SET next_auto_agent_seq = -2, updated_at_ms = -1;
            UPDATE agents SET child_session_id = 'missing-child', next_bg_seq = 0, created_at_ms = -1;
            UPDATE background_tasks SET parent_dialog_turn_id = 'missing-turn', parent_tool_call_id = '', child_dialog_turn_id = '', execution_owner_token = '', status = 'running', bg_ordinal = 0, created_at_ms = -1;
            UPDATE swarm_nodes SET parent_session_id = 'missing-parent', depth = -9, agent_type = '', created_at_ms = -1;
            INSERT INTO agents VALUES (99, 'parent-99', 'bad-type', NULL, 'not-an-integer', 'active', 0);
            INSERT INTO agents VALUES (100, 'parent-100', 'unknown-state', NULL, 1, 'retired-old-state', 0);
            INSERT INTO swarm_nodes VALUES ('orphan-node', 'missing-tree', NULL, '', -1, -1);
            INSERT INTO background_tasks (parent_session_id, agent_pk, bg_task_id, bg_ordinal, parent_dialog_turn_id, parent_tool_call_id, child_dialog_turn_id, status, execution_owner_token, created_at_ms)
            VALUES ('orphan', 999, 'orphan-task', 1, '', '', '', 'running', '', 0);
        "#).unwrap();
        drop(connection);
        let source_hash = hash_source_roots(&roots);
        let selection = session_selection();
        let engine = MigrationEngine::new(roots.clone(), adapters_for_groups(&selection)).unwrap();
        let source = probe_legacy_source(&roots, ProbeLimits::default())
            .unwrap()
            .unwrap();
        let plan = engine
            .plan(&source, selection, &CancellationToken::default())
            .unwrap();
        let report = engine
            .execute(&plan, &CancellationToken::default(), &NoCrashInjection)
            .unwrap();
        assert_eq!(report.status, MigrationRunStatus::CompletedWithWarnings);
        let result = report
            .domain_results
            .iter()
            .find(|result| result.domain == MigrationDomainId::AgentCoordination)
            .unwrap();
        assert_eq!(result.state, MigrationDomainState::Verified);
        assert_eq!(result.skipped, 4);
        assert_eq!(result.warnings[0].code, "coordination_rows_skipped");
        let target = load_coordination_data(
            &target_coordination_path(&roots),
            DatabaseRole::CurrentTarget,
        )
        .unwrap();
        assert_eq!(target.sessions[0].next_auto_agent_seq, -2);
        assert_eq!(
            target.agents[0].child_session_id.as_deref(),
            Some("missing-child")
        );
        assert_eq!(target.tasks.len(), 1);
        assert_eq!(target.tasks[0].parent_dialog_turn_id, "missing-turn");
        assert_eq!(target.tasks[0].status, "running");
        assert_eq!(target.tasks[0].execution_owner_token, "");
        assert!(target.swarm_nodes.iter().all(|row| row.depth == -9));
        assert_eq!(hash_source_roots(&roots), source_hash);
        engine
            .execute(&plan, &CancellationToken::default(), &NoCrashInjection)
            .unwrap();
        assert_eq!(
            coordination_digest(&target).unwrap(),
            coordination_digest(
                &load_coordination_data(
                    &target_coordination_path(&roots),
                    DatabaseRole::CurrentTarget
                )
                .unwrap()
            )
            .unwrap()
        );
        let layout = openbitfun_legacy_migration::MigrationLayout::new(&roots, &plan.run_id);
        let context = DomainContext {
            roots: &roots,
            layout: &layout,
            plan: &plan,
            step: plan
                .steps
                .iter()
                .find(|step| step.domain == MigrationDomainId::AgentCoordination)
                .unwrap(),
        };
        let manifest = read_coordination_manifest(&context).unwrap();
        let mut old = serde_json::to_value(&manifest).unwrap();
        old.as_object_mut().unwrap().remove("incompatibleRows");
        assert_eq!(
            serde_json::from_value::<CoordinationManifest>(old)
                .unwrap()
                .incompatible_rows,
            0
        );
    }

    #[test]
    fn unreadable_target_record_is_preserved_and_still_participates_in_change_detection() {
        let temp = test_tempdir("target-row-preserved");
        let roots = fixture_roots(temp.path());
        copy_fixture(&roots);
        drop(materialize_coordination(&roots, false));
        let target = target_coordination_path(&roots);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        let connection = Connection::open(&target).unwrap();
        initialize_coordination_schema(&connection).unwrap();
        connection
            .execute(
                "INSERT INTO coordination_sessions VALUES ('session-1', 'broken-counter', 0)",
                [],
            )
            .unwrap();
        drop(connection);
        let selection = session_selection();
        let engine = MigrationEngine::new(roots.clone(), adapters_for_groups(&selection)).unwrap();
        let source = probe_legacy_source(&roots, ProbeLimits::default())
            .unwrap()
            .unwrap();
        let plan = engine
            .plan(&source, selection, &CancellationToken::default())
            .unwrap();
        engine
            .execute(&plan, &CancellationToken::default(), &NoCrashInjection)
            .unwrap();
        let before = coordination_digest(
            &load_coordination_data(&target, DatabaseRole::CurrentTarget).unwrap(),
        )
        .unwrap();
        let connection = Connection::open(&target).unwrap();
        let value: String = connection.query_row("SELECT next_auto_agent_seq FROM coordination_sessions WHERE parent_session_id = 'session-1'", [], |row| row.get(0)).unwrap();
        assert_eq!(value, "broken-counter");
        connection.execute("UPDATE coordination_sessions SET next_auto_agent_seq = 'changed-counter' WHERE parent_session_id = 'session-1'", []).unwrap();
        drop(connection);
        let after = coordination_digest(
            &load_coordination_data(&target, DatabaseRole::CurrentTarget).unwrap(),
        )
        .unwrap();
        assert_ne!(before, after);
    }

    fn session_selection() -> MigrationSelection {
        MigrationSelection {
            groups: BTreeSet::from([MigrationGroupId::WorkspacesSessionsAndTasks]),
        }
    }

    fn fixture_roots(root: &Path) -> MigrationRoots {
        let legacy_user_root = root.join("legacy-user");
        MigrationRoots {
            legacy_skills_root: legacy_user_root.join("skills"),
            legacy_user_root,
            legacy_home_root: root.join("legacy-home"),
            legacy_ssh_root: root.join("legacy-ssh"),
            target_user_root: root.join("target-user"),
            target_home_root: root.join("target-home"),
            target_skills_root: root.join("target-skills"),
            target_ssh_root: root.join("target-ssh"),
        }
    }

    fn copy_fixture(roots: &MigrationRoots) {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../services/legacy-migration/tests/fixtures/v0.2.19");
        copy_directory(&fixture.join("user-root"), &roots.legacy_user_root).unwrap();
        copy_directory(&fixture.join("home"), &roots.legacy_home_root).unwrap();
        copy_directory(&fixture.join("ssh"), &roots.legacy_ssh_root).unwrap();
        // The archived fixture uses a Windows path; identity validation needs
        // an absolute path in the platform running this test.
        let workspace_path = roots.legacy_user_root.join("data/workspace_data.json");
        let mut workspace_data: serde_json::Value =
            serde_json::from_slice(&fs::read(&workspace_path).unwrap()).unwrap();
        workspace_data["workspaces"]["workspace-1"]["rootPath"] =
            serde_json::json!(roots.legacy_home_root.join("fixture-workspace"));
        atomic_write_json(&workspace_path, &workspace_data).unwrap();
    }

    fn materialize_coordination(roots: &MigrationRoots, with_wal_row: bool) -> Connection {
        let sql_path = roots
            .legacy_user_root
            .join("data/agent-runtime/coordination.sql");
        let database_path = source_coordination_path(roots);
        let connection = Connection::open(&database_path).unwrap();
        connection
            .execute_batch(&fs::read_to_string(sql_path).unwrap())
            .unwrap();
        if with_wal_row {
            connection
                .pragma_update(None, "journal_mode", "WAL")
                .unwrap();
            connection
                .execute(
                    "INSERT INTO coordination_sessions (parent_session_id, next_auto_agent_seq, updated_at_ms) VALUES ('session-child-1', 1, 2)",
                    [],
                )
                .unwrap();
            assert!(PathBuf::from(format!("{}-wal", database_path.display())).exists());
        }
        connection
    }

    fn copy_directory(source: &Path, target: &Path) -> std::io::Result<()> {
        fs::create_dir_all(target)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let source_path = entry.path();
            let target_path = target.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                copy_directory(&source_path, &target_path)?;
            } else {
                fs::copy(source_path, target_path)?;
            }
        }
        Ok(())
    }

    fn hash_source_roots(roots: &MigrationRoots) -> String {
        let mut entries = Vec::new();
        for root in [
            &roots.legacy_user_root,
            &roots.legacy_home_root,
            &roots.legacy_ssh_root,
        ] {
            collect_source_files(root, root, &mut entries);
        }
        entries.sort_by(|left, right| left.0.cmp(&right.0));
        let mut hasher = Sha256::new();
        for (path, bytes) in entries {
            hasher.update(path.to_string_lossy().replace('\\', "/").as_bytes());
            hasher.update([0]);
            hasher.update(bytes);
            hasher.update([0]);
        }
        hex::encode(hasher.finalize())
    }

    fn collect_source_files(root: &Path, path: &Path, entries: &mut Vec<(PathBuf, Vec<u8>)>) {
        if path.is_file() {
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with("-shm"))
            {
                return;
            }
            let mut bytes = Vec::new();
            fs::File::open(path)
                .unwrap()
                .read_to_end(&mut bytes)
                .unwrap();
            entries.push((path.strip_prefix(root).unwrap().to_path_buf(), bytes));
            return;
        }
        let mut children = fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        children.sort();
        for child in children {
            collect_source_files(root, &child, entries);
        }
    }

    fn test_tempdir(label: &str) -> tempfile::TempDir {
        let root = std::env::var_os("OPENBITFUN_TEST_TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        fs::create_dir_all(&root).unwrap();
        tempfile::Builder::new()
            .prefix(&format!("openbitfun-migration-{label}-"))
            .tempdir_in(root)
            .unwrap()
    }
}
