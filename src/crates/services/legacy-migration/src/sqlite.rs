use crate::{LegacyMigrationError, LegacyMigrationResult};
use rusqlite::backup::Backup;
use rusqlite::{Connection, OpenFlags};
use std::path::Path;
use std::time::Duration;

pub fn snapshot_sqlite_read_only(source: &Path, destination: &Path) -> LegacyMigrationResult<()> {
    if destination.exists() {
        return Err(LegacyMigrationError::InvalidRequest(format!(
            "SQLite snapshot destination already exists: {}",
            destination.display()
        )));
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|error| LegacyMigrationError::io(parent, error))?;
    }
    let source_connection = Connection::open_with_flags(
        source,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| LegacyMigrationError::sqlite(source, error))?;
    source_connection
        .pragma_update(None, "query_only", true)
        .map_err(|error| LegacyMigrationError::sqlite(source, error))?;
    validate_connection(&source_connection, source)?;

    let mut destination_connection = Connection::open(destination)
        .map_err(|error| LegacyMigrationError::sqlite(destination, error))?;
    let backup = Backup::new(&source_connection, &mut destination_connection)
        .map_err(|error| LegacyMigrationError::sqlite(source, error))?;
    backup
        .run_to_completion(128, Duration::from_millis(5), None)
        .map_err(|error| LegacyMigrationError::sqlite(destination, error))?;
    drop(backup);
    destination_connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE;")
        .map_err(|error| LegacyMigrationError::sqlite(destination, error))?;
    validate_connection(&destination_connection, destination)
}

pub fn validate_sqlite(path: &Path) -> LegacyMigrationResult<()> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| LegacyMigrationError::sqlite(path, error))?;
    validate_connection(&connection, path)
}

fn validate_connection(connection: &Connection, path: &Path) -> LegacyMigrationResult<()> {
    let result: String = connection
        .query_row("PRAGMA integrity_check(1)", [], |row| row.get(0))
        .map_err(|error| LegacyMigrationError::sqlite(path, error))?;
    if result != "ok" {
        return Err(LegacyMigrationError::UnsupportedSource(format!(
            "SQLite integrity check failed for {}",
            path.display()
        )));
    }
    Ok(())
}
