use super::common::{
    backup_domain_dir, io_error, read_bounded_json, read_optional_bounded_json, stage_domain_dir,
    validate_regular_file,
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
use openbitfun_services_core::memory_store::{
    classify_memory_workspace_file, decode_memory_record, initialize_memory_schema,
    read_memory_store_snapshot, upsert_memory_record, MemoryRecord, MemoryStoreSnapshot,
    MemoryWorkspaceFileKind, MEMORY_STORE_SCHEMA,
};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

const STRUCTURED_MEMORY_RELATIVE_PATH: &str = "data/memories/memories.sqlite";
const LEGACY_STRUCTURED_MEMORY_SCHEMA: &str = "bitfun.memory.stage1.v1";
const FILE_MEMORY_SCHEMA: &str = "openbitfun.memory-files.v1";
const MAX_MEMORY_FILE_COUNT: usize = 4_096;
const MAX_MEMORY_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_MEMORY_TOTAL_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Default, Serialize)]
struct MigrationMemorySnapshot {
    #[serde(flatten)]
    data: MemoryStoreSnapshot,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    omitted: Vec<String>,
}
impl std::ops::Deref for MigrationMemorySnapshot {
    type Target = MemoryStoreSnapshot;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

fn memory_warning(path: &str) -> MigrationDiagnostic {
    MigrationDiagnostic {
        code: "memory_item_skipped".into(),
        severity: FindingSeverity::Warning,
        domain: None,
        relative_path: Some(path.to_string()),
        message: "An unreadable memory item was retained in the source.".into(),
        ..Default::default()
    }
}

pub(crate) struct StructuredMemoryAdapter;
pub(crate) struct FileMemoryAdapter;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StructuredMemoryManifest {
    source_present: bool,
    source_digest: Option<String>,
    target_existed: bool,
    target_digest: Option<String>,
    merged_digest: Option<String>,
    imported: u64,
    skipped: u64,
    conflicts: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StructuredMemoryCommitReceipt {
    target_existed: bool,
    merged_digest: String,
}

#[derive(Debug, Default)]
struct StructuredMergeOutcome {
    imports: Vec<MemoryRecord>,
    duplicate: u64,
    target_wins: u64,
    conflicts: Vec<MigrationConflict>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FileMemoryAction {
    Import,
    Duplicate,
    TargetWins,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileMemoryManifestEntry {
    source_relative: String,
    target_relative: String,
    action: FileMemoryAction,
    expected_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileMemoryCommitReceipt {
    target_relative: String,
    expected_hash: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct FileMemoryManifest {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    omitted: Vec<String>,
    entries: Vec<FileMemoryManifestEntry>,
    imported: u64,
    skipped: u64,
    conflicts: u64,
}

struct PlannedMemoryFile {
    source_relative: PathBuf,
    target_relative: PathBuf,
    source_path: PathBuf,
    action: FileMemoryAction,
    expected_hash: String,
}

struct FileMemoryPlan {
    files: Vec<PlannedMemoryFile>,
    conflicts: Vec<MigrationConflict>,
    logical_bytes: u64,
    omitted: Vec<String>,
}

struct MemoryFileFact {
    relative: PathBuf,
    path: PathBuf,
    hash: String,
    bytes: u64,
    kind: MemoryWorkspaceFileKind,
}

impl LegacyDomainAdapter for StructuredMemoryAdapter {
    fn domain(&self) -> MigrationDomainId {
        MigrationDomainId::StructuredMemory
    }

    fn scan(&self, roots: &MigrationRoots) -> LegacyMigrationResult<DomainScan> {
        let source_path = source_structured_memory_path(roots);
        if !path_entry_exists(&source_path)? {
            return Ok(empty_domain_scan(
                self.domain(),
                "legacy_structured_memory_absent",
                LEGACY_STRUCTURED_MEMORY_SCHEMA,
                MEMORY_STORE_SCHEMA,
                "No legacy structured Memory database was found.",
            ));
        }
        let source = read_consistent_scan_snapshot(
            roots,
            &roots.legacy_user_root,
            &source_path,
            "structured-memory-source",
            MemoryDatabaseRole::LegacySource,
        )?;
        let target_path = target_structured_memory_path(roots);
        let target = if path_entry_exists(&target_path)? {
            read_consistent_scan_snapshot(
                roots,
                &roots.target_user_root,
                &target_path,
                "structured-memory-target",
                MemoryDatabaseRole::CurrentTarget,
            )?
        } else {
            MigrationMemorySnapshot::default()
        };
        let outcome = preview_structured_merge(&source.records, &target.records);
        let logical_bytes = sqlite_family_size(&source_path)?;
        Ok(DomainScan {
            finding: ScanFinding {
                domain: self.domain(),
                code: if source.omitted.is_empty() {
                    "legacy_structured_memory_supported"
                } else {
                    "memory_items_skipped"
                }
                .to_string(),
                severity: if outcome.conflicts.is_empty() && source.omitted.is_empty() {
                    FindingSeverity::Info
                } else {
                    FindingSeverity::Warning
                },
                entity_count: source.records.len() as u64,
                logical_bytes,
                source_schema: Some(LEGACY_STRUCTURED_MEMORY_SCHEMA.to_string()),
                migratable: true,
                detail: format!(
                    "{} structured Memory records are readable by the current persistence owner",
                    source.records.len()
                ),
            },
            conflicts: outcome.conflicts,
            target_schema: Some(MEMORY_STORE_SCHEMA.to_string()),
            dependencies: Vec::new(),
        })
    }

    fn stage(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<MigrationDomainResult> {
        let domain_root = stage_domain_dir(context, "structured-memory");
        reset_directory(&domain_root)?;
        let source_path = source_structured_memory_path(context.roots);
        if !path_entry_exists(&source_path)? {
            let manifest = StructuredMemoryManifest {
                source_present: false,
                source_digest: None,
                target_existed: path_entry_exists(&target_structured_memory_path(context.roots))?,
                target_digest: None,
                merged_digest: None,
                imported: 0,
                skipped: 0,
                conflicts: 0,
            };
            atomic_write_json(&structured_manifest_path(context), &manifest)?;
            return Ok(MigrationDomainResult {
                domain: self.domain(),
                state: MigrationDomainState::Staged,
                ..MigrationDomainResult::default()
            });
        }

        let staged_source = domain_root.join("source.sqlite");
        let staged_target = domain_root.join("target-before.sqlite");
        let staged_merged = domain_root.join("merged.sqlite");
        validate_sqlite_family(&context.roots.legacy_user_root, &source_path)?;
        snapshot_sqlite_read_only(&source_path, &staged_source)?;
        let source = read_memory_snapshot(&staged_source, MemoryDatabaseRole::LegacySource)?;

        let target_path = target_structured_memory_path(context.roots);
        let target_existed = path_entry_exists(&target_path)?;
        let (target, target_digest) = if target_existed {
            validate_sqlite_family(&context.roots.target_user_root, &target_path)?;
            snapshot_sqlite_read_only(&target_path, &staged_target)?;
            let target = read_memory_snapshot(&staged_target, MemoryDatabaseRole::CurrentTarget)?;
            snapshot_sqlite_read_only(&staged_target, &staged_merged)?;
            let digest = memory_snapshot_digest(&target)?;
            (target, Some(digest))
        } else {
            let connection = Connection::open(&staged_merged)
                .map_err(|error| db_error(&staged_merged, error))?;
            initialize_memory_schema(&connection).map_err(owner_error)?;
            (MigrationMemorySnapshot::default(), None)
        };

        let outcome = preview_structured_merge(&source.records, &target.records);
        {
            let mut connection = Connection::open(&staged_merged)
                .map_err(|error| db_error(&staged_merged, error))?;
            let transaction = connection
                .transaction()
                .map_err(|error| db_error(&staged_merged, error))?;
            for record in &outcome.imports {
                upsert_memory_record(&transaction, record, false).map_err(owner_error)?;
            }
            transaction
                .commit()
                .map_err(|error| db_error(&staged_merged, error))?;
        }
        finalize_sqlite_file(&staged_merged)?;
        let merged = read_memory_snapshot(&staged_merged, MemoryDatabaseRole::StagedCurrent)?;
        let manifest = StructuredMemoryManifest {
            source_present: true,
            source_digest: Some(memory_snapshot_digest(&source)?),
            target_existed,
            target_digest,
            merged_digest: Some(memory_snapshot_digest(&merged)?),
            imported: outcome.imports.len() as u64,
            skipped: outcome
                .duplicate
                .saturating_add(outcome.target_wins)
                .saturating_add(source.omitted.len() as u64),
            conflicts: outcome.conflicts.len() as u64,
        };
        atomic_write_json(&structured_manifest_path(context), &manifest)?;
        Ok(MigrationDomainResult {
            domain: self.domain(),
            state: MigrationDomainState::Staged,
            imported: manifest.imported,
            skipped: manifest.skipped,
            conflicts: manifest.conflicts,
            warnings: source
                .omitted
                .iter()
                .map(|hash| memory_warning(hash))
                .collect(),
            ..MigrationDomainResult::default()
        })
    }

    fn validate_stage(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<()> {
        let manifest = read_structured_manifest(context)?;
        if !manifest.source_present {
            return Ok(());
        }
        let domain_root = stage_domain_dir(context, "structured-memory");
        let source = read_memory_snapshot(
            &domain_root.join("source.sqlite"),
            MemoryDatabaseRole::LegacySource,
        )?;
        let legacy_digest = || -> Option<String> {
            let connection = Connection::open_with_flags(
                domain_root.join("source.sqlite"),
                OpenFlags::SQLITE_OPEN_READ_ONLY,
            )
            .ok()?;
            let data = read_memory_store_snapshot(&connection).ok()?;
            memory_snapshot_digest(&MigrationMemorySnapshot {
                data,
                omitted: Vec::new(),
            })
            .ok()
        };
        if Some(memory_snapshot_digest(&source)?) != manifest.source_digest
            && (manifest.source_digest.is_none() || legacy_digest() != manifest.source_digest)
        {
            return Err(LegacyMigrationError::InvalidRequest(
                "staged structured Memory source differs from its manifest".to_string(),
            ));
        }
        let merged = read_memory_snapshot(
            &domain_root.join("merged.sqlite"),
            MemoryDatabaseRole::StagedCurrent,
        )?;
        if Some(memory_snapshot_digest(&merged)?) != manifest.merged_digest {
            return Err(LegacyMigrationError::InvalidRequest(
                "staged structured Memory merge differs from its manifest".to_string(),
            ));
        }
        Ok(())
    }

    fn commit(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<()> {
        let manifest = read_structured_manifest(context)?;
        if !manifest.source_present {
            return Ok(());
        }
        let expected = manifest.merged_digest.as_deref().ok_or_else(|| {
            LegacyMigrationError::InvalidRequest(
                "structured Memory manifest is missing its merged digest".to_string(),
            )
        })?;
        let target = target_structured_memory_path(context.roots);
        let existing_receipt = read_structured_commit_receipt(context)?;
        if path_entry_exists(&target)?
            && existing_receipt.as_ref().is_some_and(|receipt| {
                receipt.target_existed == manifest.target_existed
                    && receipt.merged_digest == expected
            })
        {
            validate_sqlite_family(&context.roots.target_user_root, &target)?;
            if let Ok(current) = read_memory_snapshot(&target, MemoryDatabaseRole::CurrentTarget) {
                if memory_snapshot_digest(&current)? == expected {
                    finalize_sqlite_file(&target)?;
                    return Ok(());
                }
            }
        }
        verify_structured_target_state(&target, &manifest)?;
        atomic_write_json(
            &structured_commit_receipt_path(context),
            &StructuredMemoryCommitReceipt {
                target_existed: manifest.target_existed,
                merged_digest: expected.to_string(),
            },
        )?;
        let backup = backup_domain_dir(context, "structured-memory").join("memories.sqlite");
        if manifest.target_existed && !path_entry_exists(&backup)? {
            snapshot_sqlite_read_only(&target, &backup)?;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
        }
        remove_sqlite_sidecars(&target)?;
        let merged = stage_domain_dir(context, "structured-memory").join("merged.sqlite");
        let bytes = fs::read(&merged).map_err(|error| io_error(&merged, error))?;
        atomic_write_bytes(&target, &bytes)?;
        finalize_sqlite_file(&target)?;
        let current = read_memory_snapshot(&target, MemoryDatabaseRole::CurrentTarget)?;
        if memory_snapshot_digest(&current)? != expected {
            return Err(LegacyMigrationError::InvalidRequest(
                "committed structured Memory database differs from the staged merge".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_commit(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<()> {
        let manifest = read_structured_manifest(context)?;
        if !manifest.source_present {
            return Ok(());
        }
        let current = read_memory_snapshot(
            &target_structured_memory_path(context.roots),
            MemoryDatabaseRole::CurrentTarget,
        )?;
        if Some(memory_snapshot_digest(&current)?) != manifest.merged_digest {
            return Err(LegacyMigrationError::InvalidRequest(
                "current Memory owner did not read the committed structured data".to_string(),
            ));
        }
        Ok(())
    }

    fn rollback_unverified(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<()> {
        let Some(manifest) = read_optional_bounded_json::<StructuredMemoryManifest>(
            &context.layout.stage_root(),
            &structured_manifest_path(context),
        )?
        else {
            return Ok(());
        };
        if !manifest.source_present {
            return Ok(());
        }
        let Some(receipt) = read_structured_commit_receipt(context)? else {
            return Ok(());
        };
        if receipt.target_existed != manifest.target_existed
            || Some(receipt.merged_digest.as_str()) != manifest.merged_digest.as_deref()
        {
            return Err(LegacyMigrationError::InvalidRequest(
                "structured Memory commit receipt differs from its manifest".to_string(),
            ));
        }
        let target = target_structured_memory_path(context.roots);
        let backup = backup_domain_dir(context, "structured-memory").join("memories.sqlite");
        if manifest.target_existed {
            let original_digest = manifest.target_digest.as_deref().ok_or_else(|| {
                LegacyMigrationError::InvalidRequest(
                    "structured Memory manifest is missing its original target digest".to_string(),
                )
            })?;
            let merged_digest = manifest.merged_digest.as_deref().ok_or_else(|| {
                LegacyMigrationError::InvalidRequest(
                    "structured Memory manifest is missing its merged target digest".to_string(),
                )
            })?;
            if path_entry_exists(&target)? {
                validate_sqlite_family(&context.roots.target_user_root, &target)?;
                if let Ok(current) =
                    read_memory_snapshot(&target, MemoryDatabaseRole::CurrentTarget)
                {
                    let current_digest = memory_snapshot_digest(&current)?;
                    if current_digest == original_digest {
                        return Ok(());
                    }
                    if current_digest != merged_digest {
                        return Err(LegacyMigrationError::InvalidRequest(
                            "structured Memory target changed after migration commit; refusing to overwrite it during rollback"
                                .to_string(),
                        ));
                    }
                }
            }
            if path_entry_exists(&backup)? {
                validate_sqlite_family(&context.layout.backup_root(), &backup)?;
                let original = read_memory_snapshot(&backup, MemoryDatabaseRole::CurrentTarget)?;
                if memory_snapshot_digest(&original)? != original_digest {
                    return Err(LegacyMigrationError::InvalidRequest(
                        "structured Memory rollback backup differs from the staged original target"
                            .to_string(),
                    ));
                }
                remove_sqlite_sidecars(&target)?;
                let bytes = fs::read(&backup).map_err(|error| io_error(&backup, error))?;
                atomic_write_bytes(&target, &bytes)?;
                remove_sqlite_sidecars(&target)?;
            }
        } else if path_entry_exists(&target)? {
            let should_remove = manifest.merged_digest.as_deref().is_some_and(|expected| {
                read_memory_snapshot(&target, MemoryDatabaseRole::CurrentTarget)
                    .and_then(|snapshot| memory_snapshot_digest(&snapshot))
                    .is_ok_and(|actual| actual == expected)
            });
            if should_remove {
                remove_sqlite_sidecars(&target)?;
                remove_file_if_present(&target)?;
            }
        }
        Ok(())
    }
}

impl LegacyDomainAdapter for FileMemoryAdapter {
    fn domain(&self) -> MigrationDomainId {
        MigrationDomainId::FileMemory
    }

    fn scan(&self, roots: &MigrationRoots) -> LegacyMigrationResult<DomainScan> {
        let plan = plan_file_memory(roots)?;
        Ok(DomainScan {
            finding: ScanFinding {
                domain: self.domain(),
                code: if !plan.omitted.is_empty() {
                    "memory_items_skipped"
                } else if plan.files.is_empty() {
                    "legacy_file_memory_absent"
                } else {
                    "legacy_file_memory_supported"
                }
                .to_string(),
                severity: if plan.conflicts.is_empty() && plan.omitted.is_empty() {
                    FindingSeverity::Info
                } else {
                    FindingSeverity::Warning
                },
                entity_count: plan.files.len() as u64,
                logical_bytes: plan.logical_bytes,
                source_schema: Some("bitfun.memory-files.v1".to_string()),
                migratable: true,
                detail: format!(
                    "{} owner-declared file Memory inputs are eligible for migration",
                    plan.files.len()
                ),
            },
            conflicts: plan.conflicts,
            target_schema: Some(FILE_MEMORY_SCHEMA.to_string()),
            dependencies: Vec::new(),
        })
    }

    fn stage(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<MigrationDomainResult> {
        let mut plan = plan_file_memory(context.roots)?;
        let domain_root = stage_domain_dir(context, "file-memory");
        reset_directory(&domain_root)?;
        let files_root = domain_root.join("files");
        let mut entries = Vec::with_capacity(plan.files.len());
        let mut imported = 0u64;
        let mut skipped = plan.omitted.len() as u64;
        for file in plan.files {
            if file.action == FileMemoryAction::Import {
                let staged_result = (|| -> LegacyMigrationResult<()> {
                    let bytes = fs::read(&file.source_path)
                        .map_err(|error| io_error(&file.source_path, error))?;
                    atomic_write_bytes(&files_root.join(&file.target_relative), &bytes)?;
                    Ok(())
                })();
                if staged_result.is_err() {
                    skipped += 1;
                    plan.omitted.push(relative_string(&file.source_relative));
                    continue;
                }
                imported = imported.saturating_add(1);
            } else {
                skipped = skipped.saturating_add(1);
            }
            entries.push(FileMemoryManifestEntry {
                source_relative: relative_string(&file.source_relative),
                target_relative: relative_string(&file.target_relative),
                action: file.action,
                expected_hash: file.expected_hash,
            });
        }
        let manifest = FileMemoryManifest {
            omitted: plan.omitted.clone(),
            entries,
            imported,
            skipped,
            conflicts: plan.conflicts.len() as u64,
        };
        atomic_write_json(&file_manifest_path(context), &manifest)?;
        Ok(MigrationDomainResult {
            domain: self.domain(),
            state: MigrationDomainState::Staged,
            imported,
            skipped,
            conflicts: manifest.conflicts,
            warnings: plan
                .omitted
                .iter()
                .map(|path| memory_warning(path))
                .collect(),
            ..MigrationDomainResult::default()
        })
    }

    fn validate_stage(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<()> {
        let manifest = read_file_manifest(context)?;
        let files_root = stage_domain_dir(context, "file-memory").join("files");
        for (_, entry) in imported_file_entries(&manifest) {
            let relative = validated_memory_relative_path(&entry.target_relative)?;
            let path = files_root.join(relative);
            validate_regular_file(&files_root, &path)?;
            if normalized_file_hash(&path)? != entry.expected_hash {
                return Err(LegacyMigrationError::InvalidRequest(format!(
                    "staged file Memory item {} differs from its manifest",
                    entry.target_relative
                )));
            }
        }
        Ok(())
    }

    fn commit(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<()> {
        let manifest = read_file_manifest(context)?;
        let source_root = stage_domain_dir(context, "file-memory").join("files");
        let target_root = target_file_memory_root(context.roots);
        if path_entry_exists(&target_root)? {
            validate_directory(&context.roots.target_home_root, &target_root)?;
        }
        for (entry_index, entry) in imported_file_entries(&manifest) {
            let relative = validated_memory_relative_path(&entry.target_relative)?;
            let source = source_root.join(&relative);
            let target = target_root.join(&relative);
            if path_entry_exists(&target)? {
                validate_regular_file(&target_root, &target)?;
                let receipt = read_file_commit_receipt(context, entry_index)?;
                if normalized_file_hash(&target)? == entry.expected_hash
                    && receipt.as_ref().is_some_and(|receipt| {
                        receipt.target_relative == entry.target_relative
                            && receipt.expected_hash == entry.expected_hash
                    })
                {
                    continue;
                }
                return Err(LegacyMigrationError::InvalidRequest(format!(
                    "file Memory target changed after staging: {}",
                    entry.target_relative
                )));
            }
            atomic_write_json(
                &file_commit_receipt_path(context, entry_index),
                &FileMemoryCommitReceipt {
                    target_relative: entry.target_relative.clone(),
                    expected_hash: entry.expected_hash.clone(),
                },
            )?;
            ensure_safe_target_parent(context.roots, &target_root, &target)?;
            let bytes = fs::read(&source).map_err(|error| io_error(&source, error))?;
            atomic_write_bytes(&target, &bytes)?;
        }
        Ok(())
    }

    fn validate_commit(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<()> {
        let manifest = read_file_manifest(context)?;
        let target_root = target_file_memory_root(context.roots);
        if path_entry_exists(&target_root)? {
            validate_directory(&context.roots.target_home_root, &target_root)?;
        }
        for (_, entry) in imported_file_entries(&manifest) {
            let relative = validated_memory_relative_path(&entry.target_relative)?;
            let target = target_root.join(relative);
            validate_regular_file(&target_root, &target)?;
            if normalized_file_hash(&target)? != entry.expected_hash {
                return Err(LegacyMigrationError::InvalidRequest(format!(
                    "current Memory owner could not validate file {}",
                    entry.target_relative
                )));
            }
        }
        Ok(())
    }

    fn rollback_unverified(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<()> {
        let Some(manifest) = read_optional_bounded_json::<FileMemoryManifest>(
            &context.layout.stage_root(),
            &file_manifest_path(context),
        )?
        else {
            return Ok(());
        };
        let target_root = target_file_memory_root(context.roots);
        if path_entry_exists(&target_root)? {
            validate_directory(&context.roots.target_home_root, &target_root)?;
        }
        for (entry_index, entry) in imported_file_entries(&manifest).rev() {
            let Some(receipt) = read_file_commit_receipt(context, entry_index)? else {
                continue;
            };
            if receipt.target_relative != entry.target_relative
                || receipt.expected_hash != entry.expected_hash
            {
                return Err(LegacyMigrationError::InvalidRequest(format!(
                    "file Memory commit receipt differs for {}",
                    entry.target_relative
                )));
            }
            let relative = validated_memory_relative_path(&entry.target_relative)?;
            let target = target_root.join(&relative);
            if path_entry_exists(&target)?
                && validate_regular_file(&target_root, &target).is_ok()
                && normalized_file_hash(&target)? == entry.expected_hash
            {
                fs::remove_file(&target).map_err(|error| io_error(&target, error))?;
                prune_empty_memory_parents(&target_root, target.parent())?;
            }
        }
        if path_entry_exists(&target_root)? {
            validate_directory(&context.roots.target_home_root, &target_root)?;
        }
        if path_entry_exists(&target_root)? && is_directory_empty(&target_root)? {
            fs::remove_dir(&target_root).map_err(|error| io_error(&target_root, error))?;
        }
        Ok(())
    }
}

fn preview_structured_merge(
    source: &[MemoryRecord],
    target: &[MemoryRecord],
) -> StructuredMergeOutcome {
    let target_by_id = target
        .iter()
        .map(|record| {
            (
                record.session_id.as_str(),
                normalized_memory_content_hash(record),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut occupied_hashes = target_by_id.values().cloned().collect::<BTreeSet<_>>();
    let mut outcome = StructuredMergeOutcome::default();
    for record in source {
        let content_hash = normalized_memory_content_hash(record);
        if let Some(target_hash) = target_by_id.get(record.session_id.as_str()) {
            if target_hash == &content_hash {
                outcome.duplicate = outcome.duplicate.saturating_add(1);
                outcome.conflicts.push(structured_conflict(
                    record,
                    "structured_memory_duplicate",
                    "The target contains the same stable id and normalized content.",
                    ConflictResolution::DuplicateSkipped,
                ));
            } else {
                outcome.target_wins = outcome.target_wins.saturating_add(1);
                outcome.conflicts.push(structured_conflict(
                    record,
                    "structured_memory_id_conflict",
                    "The target keeps its record because the stable id is also a Session reference.",
                    ConflictResolution::TargetWins,
                ));
            }
            continue;
        }
        if !occupied_hashes.insert(content_hash) {
            outcome.duplicate = outcome.duplicate.saturating_add(1);
            outcome.conflicts.push(structured_conflict(
                record,
                "structured_memory_content_duplicate",
                "The normalized content already exists under another stable id.",
                ConflictResolution::DuplicateSkipped,
            ));
            continue;
        }
        outcome.imports.push(record.clone());
    }
    outcome
}

fn structured_conflict(
    record: &MemoryRecord,
    code: &str,
    target_summary: &str,
    resolution: ConflictResolution,
) -> MigrationConflict {
    MigrationConflict {
        domain: MigrationDomainId::StructuredMemory,
        code: code.to_string(),
        source_summary: format!("Legacy structured Memory record {}", record.session_id),
        target_summary: target_summary.to_string(),
        resolution,
    }
}

fn plan_file_memory(roots: &MigrationRoots) -> LegacyMigrationResult<FileMemoryPlan> {
    let source_root = source_file_memory_root(roots);
    let mut omitted = Vec::new();
    let source = collect_source_memory_files(&roots.legacy_home_root, &source_root, &mut omitted)?;
    let target_root = target_file_memory_root(roots);
    let target = collect_memory_files(&roots.target_home_root, &target_root)?;
    let target_by_path = target
        .iter()
        .map(|file| (file.relative.clone(), file.hash.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut occupied_paths = target_by_path.clone();
    let mut occupied_hashes = target
        .iter()
        .map(|file| file.hash.clone())
        .collect::<BTreeSet<_>>();
    let mut files = Vec::with_capacity(source.len());
    let mut conflicts = Vec::new();
    let logical_bytes = source.iter().map(|file| file.bytes).sum();

    for file in source {
        let mut target_relative = file.relative.clone();
        let mut action = FileMemoryAction::Import;
        if occupied_hashes.contains(&file.hash) {
            action = FileMemoryAction::Duplicate;
            conflicts.push(file_conflict(
                &file.relative,
                &target_relative,
                "file_memory_content_duplicate",
                ConflictResolution::DuplicateSkipped,
            ));
        } else if occupied_paths.contains_key(&target_relative) {
            if file.kind == MemoryWorkspaceFileKind::AdHocNote {
                target_relative = remapped_note_path(&file.relative, &file.hash, &occupied_paths);
                conflicts.push(file_conflict(
                    &file.relative,
                    &target_relative,
                    "file_memory_path_remapped",
                    ConflictResolution::SourceRemapped,
                ));
            } else {
                action = FileMemoryAction::TargetWins;
                conflicts.push(file_conflict(
                    &file.relative,
                    &target_relative,
                    "file_memory_path_conflict",
                    ConflictResolution::TargetWins,
                ));
            }
        }
        if action == FileMemoryAction::Import {
            occupied_paths.insert(target_relative.clone(), file.hash.clone());
            occupied_hashes.insert(file.hash.clone());
        }
        files.push(PlannedMemoryFile {
            source_relative: file.relative,
            target_relative,
            source_path: file.path,
            action,
            expected_hash: file.hash,
        });
    }
    Ok(FileMemoryPlan {
        files,
        conflicts,
        logical_bytes,
        omitted,
    })
}

fn collect_source_memory_files(
    boundary: &Path,
    root: &Path,
    omitted: &mut Vec<String>,
) -> LegacyMigrationResult<Vec<MemoryFileFact>> {
    if !path_entry_exists(root)? {
        return Ok(Vec::new());
    }
    validate_directory(boundary, root)?;
    let mut files = Vec::new();
    for name in ["MEMORY.md", "memory_summary.md"] {
        let path = root.join(name);
        if path_entry_exists(&path)? && push_memory_file(root, &path, &mut files).is_err() {
            omitted.push(relative_string(Path::new(name)));
        }
    }
    fn visit(
        root: &Path,
        path: &Path,
        files: &mut Vec<MemoryFileFact>,
        omitted: &mut Vec<String>,
        depth: usize,
    ) {
        if depth > 16 || files.len() >= MAX_MEMORY_FILE_COUNT {
            omitted.push(relative_string(path.strip_prefix(root).unwrap_or(path)));
            return;
        }
        let inspect = (|| -> LegacyMigrationResult<()> {
            validate_directory(root, path)?;
            for entry in fs::read_dir(path).map_err(|error| io_error(path, error))? {
                let entry = entry.map_err(|error| io_error(path, error))?;
                let child = entry.path();
                let meta = fs::symlink_metadata(&child).map_err(|error| io_error(&child, error))?;
                if meta.is_dir() && !meta.file_type().is_symlink() && !is_reparse_point(&meta) {
                    visit(root, &child, files, omitted, depth + 1);
                } else if classify_memory_workspace_file(child.strip_prefix(root).unwrap_or(&child))
                    .is_some()
                    && push_memory_file(root, &child, files).is_err()
                {
                    omitted.push(relative_string(child.strip_prefix(root).unwrap_or(&child)));
                }
            }
            Ok(())
        })();
        if inspect.is_err() {
            omitted.push(relative_string(path.strip_prefix(root).unwrap_or(path)));
        }
    }
    let notes = root.join("extensions/ad_hoc/notes");
    if path_entry_exists(&notes)? {
        visit(root, &notes, &mut files, omitted, 0);
    }
    files.sort_by(|left, right| left.relative.cmp(&right.relative));
    let mut count = 0usize;
    let mut total = 0u64;
    files.retain(|file| {
        if count >= MAX_MEMORY_FILE_COUNT
            || total.saturating_add(file.bytes) > MAX_MEMORY_TOTAL_BYTES
        {
            omitted.push(relative_string(&file.relative));
            false
        } else {
            count += 1;
            total += file.bytes;
            true
        }
    });
    Ok(files)
}

fn collect_memory_files(
    boundary_root: &Path,
    memory_root: &Path,
) -> LegacyMigrationResult<Vec<MemoryFileFact>> {
    let mut omitted = Vec::new();
    let files = collect_source_memory_files(boundary_root, memory_root, &mut omitted)?;
    if !omitted.is_empty() {
        return Err(LegacyMigrationError::InvalidRequest(
            "target file Memory contains unreadable items; preserving target".into(),
        ));
    }
    finish_memory_files(files)
}

fn finish_memory_files(
    mut files: Vec<MemoryFileFact>,
) -> LegacyMigrationResult<Vec<MemoryFileFact>> {
    files.sort_by(|left, right| left.relative.cmp(&right.relative));
    if files.len() > MAX_MEMORY_FILE_COUNT {
        return Err(LegacyMigrationError::ResourceLimit(format!(
            "file Memory contains more than {MAX_MEMORY_FILE_COUNT} owner-declared files"
        )));
    }
    let total = files.iter().map(|file| file.bytes).sum::<u64>();
    if total > MAX_MEMORY_TOTAL_BYTES {
        return Err(LegacyMigrationError::ResourceLimit(format!(
            "file Memory exceeds {MAX_MEMORY_TOTAL_BYTES} bytes"
        )));
    }
    Ok(files)
}

fn push_memory_file(
    memory_root: &Path,
    path: &Path,
    files: &mut Vec<MemoryFileFact>,
) -> LegacyMigrationResult<()> {
    validate_regular_file(memory_root, path)?;
    let relative = path
        .strip_prefix(memory_root)
        .map_err(|_| LegacyMigrationError::PathEscape(path.to_path_buf()))?
        .to_path_buf();
    let kind = classify_memory_workspace_file(&relative).ok_or_else(|| {
        LegacyMigrationError::InvalidRequest(format!(
            "file is outside the current Memory owner contract: {}",
            relative.display()
        ))
    })?;
    let bytes = fs::metadata(path)
        .map_err(|error| io_error(path, error))?
        .len();
    if bytes > MAX_MEMORY_FILE_BYTES {
        return Err(LegacyMigrationError::ResourceLimit(format!(
            "file Memory item exceeds {MAX_MEMORY_FILE_BYTES} bytes: {}",
            relative.display()
        )));
    }
    files.push(MemoryFileFact {
        relative,
        path: path.to_path_buf(),
        hash: normalized_file_hash(path)?,
        bytes,
        kind,
    });
    Ok(())
}

fn file_conflict(
    source_relative: &Path,
    target_relative: &Path,
    code: &str,
    resolution: ConflictResolution,
) -> MigrationConflict {
    MigrationConflict {
        domain: MigrationDomainId::FileMemory,
        code: code.to_string(),
        source_summary: format!(
            "Legacy file Memory item {}",
            relative_string(source_relative)
        ),
        target_summary: format!(
            "Current file Memory path {}",
            relative_string(target_relative)
        ),
        resolution,
    }
}

fn remapped_note_path(source: &Path, hash: &str, occupied: &BTreeMap<PathBuf, String>) -> PathBuf {
    let parent = source.parent().unwrap_or_else(|| Path::new(""));
    let stem = source
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("legacy-note");
    let short_hash = hash.get(..8).unwrap_or(hash);
    for suffix in 0u32.. {
        let name = if suffix == 0 {
            format!("{stem}-from-bitfun-{short_hash}.md")
        } else {
            format!("{stem}-from-bitfun-{short_hash}-{suffix}.md")
        };
        let candidate = parent.join(name);
        if !occupied.contains_key(&candidate) {
            return candidate;
        }
    }
    unreachable!("u32 note remap namespace should not be exhausted")
}

fn normalized_memory_content_hash(record: &MemoryRecord) -> String {
    let mut hasher = Sha256::new();
    hasher.update(normalize_text(&record.raw_memory).as_bytes());
    hasher.update([0]);
    hasher.update(normalize_text(&record.rollout_summary).as_bytes());
    hex::encode(hasher.finalize())
}

fn normalized_file_hash(path: &Path) -> LegacyMigrationResult<String> {
    let bytes = fs::read(path).map_err(|error| io_error(path, error))?;
    let text = std::str::from_utf8(&bytes).map_err(|_| {
        LegacyMigrationError::UnsupportedSource(format!(
            "file Memory item is not UTF-8: {}",
            path.display()
        ))
    })?;
    let mut hasher = Sha256::new();
    hasher.update(normalize_text(text).as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

fn normalize_text(text: &str) -> String {
    let normalized = text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    normalized.trim_end_matches('\n').to_string()
}

fn memory_snapshot_digest(snapshot: &MigrationMemorySnapshot) -> LegacyMigrationResult<String> {
    let bytes = serde_json::to_vec(snapshot)
        .map_err(|error| LegacyMigrationError::InvalidRequest(error.to_string()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn read_consistent_scan_snapshot(
    roots: &MigrationRoots,
    boundary: &Path,
    source: &Path,
    label: &str,
    role: MemoryDatabaseRole,
) -> LegacyMigrationResult<MigrationMemorySnapshot> {
    validate_sqlite_family(boundary, source)?;
    let scan_root = roots.migration_root().join("scan-snapshots");
    fs::create_dir_all(&scan_root).map_err(|error| io_error(&scan_root, error))?;
    let snapshot = scan_root.join(format!("{label}-{}.sqlite", Uuid::new_v4()));
    let result = snapshot_sqlite_read_only(source, &snapshot)
        .and_then(|()| read_memory_snapshot(&snapshot, role));
    let cleanup = remove_sqlite_family(&snapshot);
    if scan_root.exists() && is_directory_empty(&scan_root).unwrap_or(false) {
        let _ = fs::remove_dir(&scan_root);
    }
    match (result, cleanup) {
        (Ok(snapshot), Ok(())) => Ok(snapshot),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
    }
}

#[derive(Debug, Clone, Copy)]
enum MemoryDatabaseRole {
    LegacySource,
    CurrentTarget,
    StagedCurrent,
}

fn read_memory_snapshot(
    path: &Path,
    role: MemoryDatabaseRole,
) -> LegacyMigrationResult<MigrationMemorySnapshot> {
    validate_sqlite(path)?;
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| db_error(path, error))?;
    if matches!(role, MemoryDatabaseRole::LegacySource) {
        // Historical jobs are recreated by runtime acquisition and never imported.
        // Only the content table is required; malformed rows do not poison peers.
        let mut statement = connection.prepare("SELECT thread_id, workspace_path, rollout_path, source_updated_at, raw_memory, rollout_summary, rollout_slug, generated_at, COALESCE(usage_count, 0), last_usage, selected_for_phase2, selected_for_phase2_source_updated_at FROM stage1_outputs ORDER BY thread_id")
            .map_err(|error| LegacyMigrationError::UnsupportedSource(format!("legacy structured Memory schema is not supported: {error}")))?;
        let mut rows = statement.query([]).map_err(|error| db_error(path, error))?;
        let mut snapshot = MigrationMemorySnapshot::default();
        while let Some(row) = rows.next().map_err(|error| db_error(path, error))? {
            match decode_memory_record(row) {
                Ok(mut record) if !record.session_id.trim().is_empty() => {
                    record.selected_for_phase2 = i64::from(record.selected_for_phase2 != 0);
                    record.usage_count = record.usage_count.max(0);
                    snapshot.data.records.push(record);
                }
                _ => {
                    let mut hash = Sha256::new();
                    for index in 0..row.as_ref().column_count() {
                        hash.update(
                            format!(
                                "{:?}",
                                row.get_ref(index).map_err(|error| db_error(path, error))?
                            )
                            .as_bytes(),
                        );
                        hash.update([0]);
                    }
                    snapshot.omitted.push(hex::encode(hash.finalize()));
                }
            }
        }
        return Ok(snapshot);
    }
    read_memory_store_snapshot(&connection)
        .map(|data| MigrationMemorySnapshot {
            data,
            omitted: Vec::new(),
        })
        .map_err(|error| {
            LegacyMigrationError::InvalidRequest(format!(
                "current structured Memory schema is invalid: {error}"
            ))
        })
}

fn verify_structured_target_state(
    target: &Path,
    manifest: &StructuredMemoryManifest,
) -> LegacyMigrationResult<()> {
    if !manifest.target_existed {
        if path_entry_exists(target)? {
            return Err(LegacyMigrationError::InvalidRequest(
                "structured Memory target appeared after staging".to_string(),
            ));
        }
        return Ok(());
    }
    if !path_entry_exists(target)? {
        return Err(LegacyMigrationError::InvalidRequest(
            "structured Memory target disappeared after staging".to_string(),
        ));
    }
    let current = read_memory_snapshot(target, MemoryDatabaseRole::CurrentTarget)?;
    if Some(memory_snapshot_digest(&current)?) != manifest.target_digest {
        return Err(LegacyMigrationError::InvalidRequest(
            "structured Memory target changed after staging".to_string(),
        ));
    }
    Ok(())
}

fn validate_sqlite_family(boundary: &Path, database: &Path) -> LegacyMigrationResult<()> {
    validate_regular_file(boundary, database)?;
    for sidecar in sqlite_sidecars(database) {
        if path_entry_exists(&sidecar)? {
            validate_regular_file(boundary, &sidecar)?;
        }
    }
    Ok(())
}

fn sqlite_family_size(database: &Path) -> LegacyMigrationResult<u64> {
    let mut total = fs::metadata(database)
        .map_err(|error| io_error(database, error))?
        .len();
    for sidecar in sqlite_sidecars(database) {
        if path_entry_exists(&sidecar)? {
            total = total.saturating_add(
                fs::metadata(&sidecar)
                    .map_err(|error| io_error(&sidecar, error))?
                    .len(),
            );
        }
    }
    Ok(total)
}

fn finalize_sqlite_file(path: &Path) -> LegacyMigrationResult<()> {
    let connection = Connection::open(path).map_err(|error| db_error(path, error))?;
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;")
        .map_err(|error| db_error(path, error))?;
    drop(connection);
    remove_sqlite_sidecars(path)
}

fn remove_sqlite_family(path: &Path) -> LegacyMigrationResult<()> {
    remove_file_if_present(path)?;
    remove_sqlite_sidecars(path)
}

fn remove_sqlite_sidecars(path: &Path) -> LegacyMigrationResult<()> {
    for sidecar in sqlite_sidecars(path) {
        remove_file_if_present(&sidecar)?;
    }
    Ok(())
}

fn sqlite_sidecars(path: &Path) -> [PathBuf; 2] {
    [
        PathBuf::from(format!("{}-wal", path.display())),
        PathBuf::from(format!("{}-shm", path.display())),
    ]
}

fn remove_file_if_present(path: &Path) -> LegacyMigrationResult<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(path, error)),
    }
}

fn path_entry_exists(path: &Path) -> LegacyMigrationResult<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(path, error)),
    }
}

fn reset_directory(path: &Path) -> LegacyMigrationResult<()> {
    if path_entry_exists(path)? {
        let metadata = fs::symlink_metadata(path).map_err(|error| io_error(path, error))?;
        if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            return Err(LegacyMigrationError::LinkedPath(path.to_path_buf()));
        }
        fs::remove_dir_all(path).map_err(|error| io_error(path, error))?;
    }
    fs::create_dir_all(path).map_err(|error| io_error(path, error))
}

fn validate_directory(boundary: &Path, path: &Path) -> LegacyMigrationResult<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_error(path, error))?;
    if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err(LegacyMigrationError::LinkedPath(path.to_path_buf()));
    }
    if !metadata.is_dir() {
        return Err(LegacyMigrationError::InvalidRequest(format!(
            "expected a directory at {}",
            path.display()
        )));
    }
    let canonical_boundary =
        fs::canonicalize(boundary).map_err(|error| io_error(boundary, error))?;
    let canonical_path = fs::canonicalize(path).map_err(|error| io_error(path, error))?;
    if !canonical_path.starts_with(canonical_boundary) {
        return Err(LegacyMigrationError::PathEscape(path.to_path_buf()));
    }
    Ok(())
}

fn ensure_safe_target_parent(
    roots: &MigrationRoots,
    target_root: &Path,
    target: &Path,
) -> LegacyMigrationResult<()> {
    if !path_entry_exists(&roots.target_home_root)? {
        fs::create_dir_all(&roots.target_home_root)
            .map_err(|error| io_error(&roots.target_home_root, error))?;
    }
    validate_directory(&roots.target_home_root, &roots.target_home_root)?;
    if !path_entry_exists(target_root)? {
        fs::create_dir(target_root).map_err(|error| io_error(target_root, error))?;
    }
    validate_directory(&roots.target_home_root, target_root)?;
    let parent = target.parent().ok_or_else(|| {
        LegacyMigrationError::InvalidRequest(format!(
            "file Memory target has no parent: {}",
            target.display()
        ))
    })?;
    let relative_parent = parent
        .strip_prefix(target_root)
        .map_err(|_| LegacyMigrationError::PathEscape(parent.to_path_buf()))?;
    let mut current = target_root.to_path_buf();
    for component in relative_parent.components() {
        let Component::Normal(component) = component else {
            return Err(LegacyMigrationError::PathEscape(parent.to_path_buf()));
        };
        current.push(component);
        if path_entry_exists(&current)? {
            validate_directory(target_root, &current)?;
        } else {
            fs::create_dir(&current).map_err(|error| io_error(&current, error))?;
        }
    }
    Ok(())
}

fn validated_memory_relative_path(raw: &str) -> LegacyMigrationResult<PathBuf> {
    let relative = PathBuf::from(raw);
    if classify_memory_workspace_file(&relative).is_none()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(LegacyMigrationError::PathEscape(relative));
    }
    Ok(relative)
}

fn imported_file_entries(
    manifest: &FileMemoryManifest,
) -> impl DoubleEndedIterator<Item = (usize, &FileMemoryManifestEntry)> {
    manifest
        .entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.action == FileMemoryAction::Import)
}

fn prune_empty_memory_parents(
    target_root: &Path,
    mut parent: Option<&Path>,
) -> LegacyMigrationResult<()> {
    while let Some(path) = parent {
        if path == target_root || !path.starts_with(target_root) || !is_directory_empty(path)? {
            break;
        }
        fs::remove_dir(path).map_err(|error| io_error(path, error))?;
        parent = path.parent();
    }
    Ok(())
}

fn is_directory_empty(path: &Path) -> LegacyMigrationResult<bool> {
    Ok(fs::read_dir(path)
        .map_err(|error| io_error(path, error))?
        .next()
        .is_none())
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

fn empty_domain_scan(
    domain: MigrationDomainId,
    code: &str,
    source_schema: &str,
    target_schema: &str,
    detail: &str,
) -> DomainScan {
    DomainScan {
        finding: ScanFinding {
            domain,
            code: code.to_string(),
            source_schema: Some(source_schema.to_string()),
            migratable: true,
            detail: detail.to_string(),
            ..ScanFinding::default()
        },
        conflicts: Vec::new(),
        target_schema: Some(target_schema.to_string()),
        dependencies: Vec::new(),
    }
}

fn source_structured_memory_path(roots: &MigrationRoots) -> PathBuf {
    roots.legacy_user_root.join(STRUCTURED_MEMORY_RELATIVE_PATH)
}

fn target_structured_memory_path(roots: &MigrationRoots) -> PathBuf {
    roots.target_user_root.join(STRUCTURED_MEMORY_RELATIVE_PATH)
}

fn source_file_memory_root(roots: &MigrationRoots) -> PathBuf {
    roots.legacy_home_root.join("memories")
}

fn target_file_memory_root(roots: &MigrationRoots) -> PathBuf {
    roots.target_home_root.join("memories")
}

fn structured_manifest_path(context: &DomainContext<'_>) -> PathBuf {
    stage_domain_dir(context, "structured-memory").join("manifest.json")
}

fn file_manifest_path(context: &DomainContext<'_>) -> PathBuf {
    stage_domain_dir(context, "file-memory").join("manifest.json")
}

fn structured_commit_receipt_path(context: &DomainContext<'_>) -> PathBuf {
    backup_domain_dir(context, "structured-memory").join("commit-receipt.json")
}

fn file_commit_receipt_path(context: &DomainContext<'_>, entry_index: usize) -> PathBuf {
    backup_domain_dir(context, "file-memory")
        .join("commit-receipts")
        .join(format!("{entry_index:04}.json"))
}

fn read_structured_manifest(
    context: &DomainContext<'_>,
) -> LegacyMigrationResult<StructuredMemoryManifest> {
    read_bounded_json(
        &context.layout.stage_root(),
        &structured_manifest_path(context),
    )
}

fn read_file_manifest(context: &DomainContext<'_>) -> LegacyMigrationResult<FileMemoryManifest> {
    read_bounded_json(&context.layout.stage_root(), &file_manifest_path(context))
}

fn read_structured_commit_receipt(
    context: &DomainContext<'_>,
) -> LegacyMigrationResult<Option<StructuredMemoryCommitReceipt>> {
    read_optional_bounded_json(
        &context.layout.backup_root(),
        &structured_commit_receipt_path(context),
    )
}

fn read_file_commit_receipt(
    context: &DomainContext<'_>,
    entry_index: usize,
) -> LegacyMigrationResult<Option<FileMemoryCommitReceipt>> {
    read_optional_bounded_json(
        &context.layout.backup_root(),
        &file_commit_receipt_path(context, entry_index),
    )
}

fn relative_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn db_error(path: &Path, error: rusqlite::Error) -> LegacyMigrationError {
    LegacyMigrationError::InvalidRequest(format!(
        "SQLite operation failed at {}: {error}",
        path.display()
    ))
}

fn owner_error(error: impl std::fmt::Display) -> LegacyMigrationError {
    LegacyMigrationError::InvalidRequest(format!("Memory persistence owner rejected data: {error}"))
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
    fn legacy_memory_jobs_are_optional_and_bad_rows_are_isolated() {
        let temp = test_tempdir("memory-bad-row");
        let roots = fixture_roots(temp.path());
        copy_fixture(&roots);
        let connection = materialize_source_memory(&roots, false);
        connection.execute_batch("DROP TABLE jobs;").unwrap();
        let original = read_memory_snapshot(
            &source_structured_memory_path(&roots),
            MemoryDatabaseRole::LegacySource,
        )
        .unwrap();
        assert!(!original.records.is_empty());
        let id = original.records[0].session_id.clone();
        connection
            .execute(
                "UPDATE stage1_outputs SET raw_memory = X'FF' WHERE thread_id = ?1",
                [&id],
            )
            .unwrap();
        let recovered = read_memory_snapshot(
            &source_structured_memory_path(&roots),
            MemoryDatabaseRole::LegacySource,
        )
        .unwrap();
        assert_eq!(recovered.records.len() + 1, original.records.len());
        assert_eq!(recovered.omitted.len(), 1);
        assert!(recovered.jobs.is_empty());
    }

    #[test]
    fn structured_memory_uses_wal_target_priority_hash_dedup_and_owner_validation() {
        let temp = test_tempdir("structured-memory");
        let roots = fixture_roots(temp.path());
        copy_fixture(&roots);
        let source_connection = materialize_source_memory(&roots, true);
        let target_path = target_structured_memory_path(&roots);
        fs::create_dir_all(target_path.parent().unwrap()).unwrap();
        let target_connection = Connection::open(&target_path).unwrap();
        initialize_memory_schema(&target_connection).unwrap();
        upsert_memory_record(
            &target_connection,
            &memory_record(
                "session-1",
                "Current target fact.",
                "Current target summary.",
            ),
            true,
        )
        .unwrap();
        upsert_memory_record(
            &target_connection,
            &memory_record(
                "target-duplicate",
                "Repeated durable fact.\n",
                "Repeated summary.",
            ),
            true,
        )
        .unwrap();
        target_connection
            .execute(
                "INSERT INTO jobs (kind, job_key, status, retry_remaining) VALUES ('memory_stage1', 'target-job', 'done', 3)",
                [],
            )
            .unwrap();
        drop(target_connection);

        let source_hash = hash_source_roots(&roots);
        let selection = memory_selection();
        let source = probe_legacy_source(&roots, ProbeLimits::default())
            .unwrap()
            .unwrap();
        let engine = MigrationEngine::new(roots.clone(), adapters_for_groups(&selection)).unwrap();
        let plan = engine
            .plan(&source, selection.clone(), &CancellationToken::default())
            .unwrap();
        let structured_conflicts = plan
            .conflicts
            .iter()
            .filter(|conflict| conflict.domain == MigrationDomainId::StructuredMemory)
            .collect::<Vec<_>>();
        assert!(structured_conflicts
            .iter()
            .any(|conflict| conflict.code == "structured_memory_id_conflict"));
        assert!(structured_conflicts
            .iter()
            .any(|conflict| conflict.code == "structured_memory_content_duplicate"));
        assert!(structured_conflicts.iter().all(|conflict| {
            !conflict
                .source_summary
                .contains("Synthetic migration fixture memory")
                && !conflict.target_summary.contains("Current target fact")
        }));

        let report = engine
            .execute(&plan, &CancellationToken::default(), &NoCrashInjection)
            .unwrap();
        assert!(matches!(
            report.status,
            MigrationRunStatus::Completed | MigrationRunStatus::CompletedWithWarnings
        ));
        let target_connection = Connection::open(&target_path).unwrap();
        let snapshot = read_memory_store_snapshot(&target_connection).unwrap();
        assert_eq!(snapshot.records.len(), 4);
        assert_eq!(snapshot.jobs.len(), 1);
        assert_eq!(snapshot.jobs[0].job_key, "target-job");
        assert_eq!(
            snapshot
                .records
                .iter()
                .find(|record| record.session_id == "session-1")
                .unwrap()
                .raw_memory,
            "Current target fact."
        );
        assert!(snapshot
            .records
            .iter()
            .any(|record| record.session_id == "session-memory-import"));
        assert!(snapshot
            .records
            .iter()
            .any(|record| record.session_id == "session-memory-wal"));
        assert!(!snapshot
            .records
            .iter()
            .any(|record| record.session_id == "session-memory-duplicate"));
        drop(target_connection);
        assert!(!PathBuf::from(format!("{}-wal", target_path.display())).exists());
        assert!(!PathBuf::from(format!("{}-shm", target_path.display())).exists());
        let report_json = serde_json::to_string(&report).unwrap();
        for private_text in [
            "Synthetic migration fixture memory",
            "Current target fact",
            "Repeated durable fact",
            "WAL-only durable fact",
        ] {
            assert!(!report_json.contains(private_text));
        }
        assert_eq!(hash_source_roots(&roots), source_hash);

        let repeated_source = probe_legacy_source(&roots, ProbeLimits::default())
            .unwrap()
            .unwrap();
        let repeated_plan = engine
            .plan(&repeated_source, selection, &CancellationToken::default())
            .unwrap();
        engine
            .execute(
                &repeated_plan,
                &CancellationToken::default(),
                &NoCrashInjection,
            )
            .unwrap();
        let target_connection = Connection::open(&target_path).unwrap();
        assert_eq!(
            read_memory_store_snapshot(&target_connection)
                .unwrap()
                .records
                .len(),
            4
        );
        assert_eq!(hash_source_roots(&roots), source_hash);
        drop(source_connection);
    }

    #[test]
    fn structured_memory_recovers_after_commit_before_journal() {
        let temp = test_tempdir("structured-memory-crash");
        let roots = fixture_roots(temp.path());
        copy_fixture(&roots);
        let source_connection = materialize_source_memory(&roots, false);
        let source_hash = hash_source_roots(&roots);
        let selection = memory_selection();
        let source = probe_legacy_source(&roots, ProbeLimits::default())
            .unwrap()
            .unwrap();
        let engine = MigrationEngine::new(roots.clone(), adapters_for_groups(&selection)).unwrap();
        let plan = engine
            .plan(&source, selection, &CancellationToken::default())
            .unwrap();
        let crash = CrashOnce {
            point: CrashPoint::AfterCommit(MigrationDomainId::StructuredMemory),
            fired: AtomicBool::new(false),
        };
        assert!(matches!(
            engine.execute(&plan, &CancellationToken::default(), &crash),
            Err(LegacyMigrationError::InjectedCrash(
                CrashPoint::AfterCommit(MigrationDomainId::StructuredMemory)
            ))
        ));
        let report = engine
            .execute(&plan, &CancellationToken::default(), &NoCrashInjection)
            .unwrap();
        assert!(matches!(
            report.status,
            MigrationRunStatus::Completed | MigrationRunStatus::CompletedWithWarnings
        ));
        let target = Connection::open(target_structured_memory_path(&roots)).unwrap();
        let snapshot = read_memory_store_snapshot(&target).unwrap();
        assert_eq!(snapshot.records.len(), 1);
        assert!(snapshot.jobs.is_empty());
        assert_eq!(hash_source_roots(&roots), source_hash);
        drop(source_connection);
    }

    #[test]
    fn structured_memory_rollback_preserves_post_commit_target_changes() {
        let temp = test_tempdir("structured-memory-rollback-race");
        let roots = fixture_roots(temp.path());
        copy_fixture(&roots);
        let source_connection = materialize_source_memory(&roots, false);
        let target_path = target_structured_memory_path(&roots);
        fs::create_dir_all(target_path.parent().unwrap()).unwrap();
        let target = Connection::open(&target_path).unwrap();
        initialize_memory_schema(&target).unwrap();
        upsert_memory_record(
            &target,
            &memory_record(
                "original-target",
                "Original target fact.",
                "Original target summary.",
            ),
            true,
        )
        .unwrap();
        drop(target);

        let selection = memory_selection();
        let source = probe_legacy_source(&roots, ProbeLimits::default())
            .unwrap()
            .unwrap();
        let engine = MigrationEngine::new(roots.clone(), adapters_for_groups(&selection)).unwrap();
        let plan = engine
            .plan(&source, selection, &CancellationToken::default())
            .unwrap();
        let crash = CrashOnce {
            point: CrashPoint::AfterCommit(MigrationDomainId::StructuredMemory),
            fired: AtomicBool::new(false),
        };
        assert!(matches!(
            engine.execute(&plan, &CancellationToken::default(), &crash),
            Err(LegacyMigrationError::InjectedCrash(
                CrashPoint::AfterCommit(MigrationDomainId::StructuredMemory)
            ))
        ));

        let target = Connection::open(&target_path).unwrap();
        upsert_memory_record(
            &target,
            &memory_record(
                "post-commit-target",
                "Post-commit target fact.",
                "Post-commit target summary.",
            ),
            true,
        )
        .unwrap();
        drop(target);

        let error = engine
            .execute(&plan, &CancellationToken::default(), &NoCrashInjection)
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("structured Memory target changed after staging"));
        let target = Connection::open(&target_path).unwrap();
        let snapshot = read_memory_store_snapshot(&target).unwrap();
        assert!(snapshot
            .records
            .iter()
            .any(|record| record.session_id == "original-target"));
        assert!(snapshot
            .records
            .iter()
            .any(|record| record.session_id == "post-commit-target"));
        assert!(snapshot
            .records
            .iter()
            .any(|record| record.session_id == "session-1"));
        drop(source_connection);
    }

    #[test]
    fn structured_memory_does_not_remove_a_target_that_appears_after_staging() {
        let temp = test_tempdir("structured-target-race");
        let roots = fixture_roots(temp.path());
        copy_fixture(&roots);
        let source_connection = materialize_source_memory(&roots, false);
        let source_hash = hash_source_roots(&roots);
        let selection = memory_selection();
        let source = probe_legacy_source(&roots, ProbeLimits::default())
            .unwrap()
            .unwrap();
        let engine = MigrationEngine::new(roots.clone(), adapters_for_groups(&selection)).unwrap();
        let plan = engine
            .plan(&source, selection, &CancellationToken::default())
            .unwrap();
        let crash = CrashOnce {
            point: CrashPoint::AfterStageValidated(MigrationDomainId::StructuredMemory),
            fired: AtomicBool::new(false),
        };
        assert!(matches!(
            engine.execute(&plan, &CancellationToken::default(), &crash),
            Err(LegacyMigrationError::InjectedCrash(
                CrashPoint::AfterStageValidated(MigrationDomainId::StructuredMemory)
            ))
        ));

        let target_path = target_structured_memory_path(&roots);
        fs::create_dir_all(target_path.parent().unwrap()).unwrap();
        let target = Connection::open(&target_path).unwrap();
        initialize_memory_schema(&target).unwrap();
        upsert_memory_record(
            &target,
            &memory_record("new-target", "New target fact.", "New target summary."),
            true,
        )
        .unwrap();
        drop(target);

        let error = engine
            .execute(&plan, &CancellationToken::default(), &NoCrashInjection)
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("structured Memory target appeared after staging"));
        let target = Connection::open(&target_path).unwrap();
        let snapshot = read_memory_store_snapshot(&target).unwrap();
        assert_eq!(snapshot.records.len(), 1);
        assert_eq!(snapshot.records[0].session_id, "new-target");
        assert_eq!(hash_source_roots(&roots), source_hash);
        drop(source_connection);
    }

    #[test]
    fn file_memory_preserves_target_remaps_notes_and_excludes_generated_files() {
        let temp = test_tempdir("file-memory");
        let roots = fixture_roots(temp.path());
        copy_fixture(&roots);
        let source_connection = materialize_source_memory(&roots, false);
        let source_memory = source_file_memory_root(&roots);
        let source_notes = source_memory.join("extensions/ad_hoc/notes");
        fs::create_dir_all(&source_notes).unwrap();
        fs::write(
            source_notes.join("collision.md"),
            "Legacy collision note.\r\n",
        )
        .unwrap();
        fs::write(source_notes.join("duplicate.md"), "Duplicate note.\n").unwrap();
        fs::write(
            source_memory.join("raw_memories.md"),
            "Generated raw memory.",
        )
        .unwrap();
        fs::write(
            source_memory.join("phase2_workspace_diff.md"),
            "Temporary diff.",
        )
        .unwrap();
        fs::create_dir_all(source_memory.join("rollout_summaries")).unwrap();
        fs::write(
            source_memory.join("rollout_summaries/generated.md"),
            "Generated rollout summary.",
        )
        .unwrap();
        fs::write(
            source_memory.join("extensions/ad_hoc/instructions.md"),
            "Generated owner instructions.",
        )
        .unwrap();

        let target_memory = target_file_memory_root(&roots);
        let target_notes = target_memory.join("extensions/ad_hoc/notes");
        fs::create_dir_all(&target_notes).unwrap();
        fs::write(target_memory.join("MEMORY.md"), "Current target Memory.\n").unwrap();
        fs::write(
            target_notes.join("collision.md"),
            "Current collision note.\n",
        )
        .unwrap();
        fs::write(target_notes.join("existing.md"), "Duplicate note.\r\n").unwrap();

        let source_hash = hash_source_roots(&roots);
        let selection = memory_selection();
        let source = probe_legacy_source(&roots, ProbeLimits::default())
            .unwrap()
            .unwrap();
        let engine = MigrationEngine::new(roots.clone(), adapters_for_groups(&selection)).unwrap();
        let plan = engine
            .plan(&source, selection, &CancellationToken::default())
            .unwrap();
        let file_conflicts = plan
            .conflicts
            .iter()
            .filter(|conflict| conflict.domain == MigrationDomainId::FileMemory)
            .collect::<Vec<_>>();
        assert!(file_conflicts
            .iter()
            .any(|conflict| conflict.code == "file_memory_path_conflict"));
        assert!(file_conflicts
            .iter()
            .any(|conflict| conflict.code == "file_memory_path_remapped"));
        assert!(file_conflicts
            .iter()
            .any(|conflict| conflict.code == "file_memory_content_duplicate"));

        let report = engine
            .execute(&plan, &CancellationToken::default(), &NoCrashInjection)
            .unwrap();
        assert_eq!(
            fs::read_to_string(target_memory.join("MEMORY.md")).unwrap(),
            "Current target Memory.\n"
        );
        assert_eq!(
            fs::read_to_string(target_notes.join("collision.md")).unwrap(),
            "Current collision note.\n"
        );
        let remapped = fs::read_dir(&target_notes)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("collision-from-bitfun-"))
            })
            .expect("colliding ad-hoc note should be deterministically remapped");
        assert_eq!(
            fs::read_to_string(remapped).unwrap(),
            "Legacy collision note.\r\n"
        );
        assert!(!target_notes.join("duplicate.md").exists());
        assert!(!target_memory.join("raw_memories.md").exists());
        assert!(!target_memory.join("phase2_workspace_diff.md").exists());
        assert!(!target_memory.join("rollout_summaries").exists());
        assert!(!target_memory
            .join("extensions/ad_hoc/instructions.md")
            .exists());
        let report_json = serde_json::to_string(&report).unwrap();
        for private_text in [
            "Synthetic memory fixture",
            "Legacy collision note",
            "Current target Memory",
            "Duplicate note",
        ] {
            assert!(!report_json.contains(private_text));
        }
        assert_eq!(hash_source_roots(&roots), source_hash);
        drop(source_connection);
    }

    #[test]
    fn file_memory_recovers_after_commit_before_journal_without_duplicates() {
        let temp = test_tempdir("file-memory-crash");
        let roots = fixture_roots(temp.path());
        copy_fixture(&roots);
        let source_connection = materialize_source_memory(&roots, false);
        let source_hash = hash_source_roots(&roots);
        let selection = memory_selection();
        let source = probe_legacy_source(&roots, ProbeLimits::default())
            .unwrap()
            .unwrap();
        let engine = MigrationEngine::new(roots.clone(), adapters_for_groups(&selection)).unwrap();
        let plan = engine
            .plan(&source, selection, &CancellationToken::default())
            .unwrap();
        let crash = CrashOnce {
            point: CrashPoint::AfterCommit(MigrationDomainId::FileMemory),
            fired: AtomicBool::new(false),
        };
        assert!(matches!(
            engine.execute(&plan, &CancellationToken::default(), &crash),
            Err(LegacyMigrationError::InjectedCrash(
                CrashPoint::AfterCommit(MigrationDomainId::FileMemory)
            ))
        ));
        let target_index = target_file_memory_root(&roots).join("MEMORY.md");
        let committed = fs::read(&target_index).unwrap();

        engine
            .execute(&plan, &CancellationToken::default(), &NoCrashInjection)
            .unwrap();
        assert_eq!(fs::read(&target_index).unwrap(), committed);
        assert_eq!(
            collect_memory_files(&roots.target_home_root, &target_file_memory_root(&roots))
                .unwrap()
                .len(),
            1
        );
        assert_eq!(hash_source_roots(&roots), source_hash);
        drop(source_connection);
    }

    #[test]
    fn file_memory_does_not_claim_an_equal_target_that_appears_after_staging() {
        let temp = test_tempdir("file-target-race");
        let roots = fixture_roots(temp.path());
        copy_fixture(&roots);
        let source_connection = materialize_source_memory(&roots, false);
        let source_hash = hash_source_roots(&roots);
        let selection = memory_selection();
        let source = probe_legacy_source(&roots, ProbeLimits::default())
            .unwrap()
            .unwrap();
        let engine = MigrationEngine::new(roots.clone(), adapters_for_groups(&selection)).unwrap();
        let plan = engine
            .plan(&source, selection, &CancellationToken::default())
            .unwrap();
        let crash = CrashOnce {
            point: CrashPoint::AfterStageValidated(MigrationDomainId::FileMemory),
            fired: AtomicBool::new(false),
        };
        assert!(matches!(
            engine.execute(&plan, &CancellationToken::default(), &crash),
            Err(LegacyMigrationError::InjectedCrash(
                CrashPoint::AfterStageValidated(MigrationDomainId::FileMemory)
            ))
        ));

        let target = target_file_memory_root(&roots).join("MEMORY.md");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        let source_bytes = fs::read(source_file_memory_root(&roots).join("MEMORY.md")).unwrap();
        fs::write(&target, &source_bytes).unwrap();
        let error = engine
            .execute(&plan, &CancellationToken::default(), &NoCrashInjection)
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("file Memory target changed after staging"));
        assert_eq!(fs::read(&target).unwrap(), source_bytes);
        assert_eq!(hash_source_roots(&roots), source_hash);
        drop(source_connection);
    }

    #[test]
    fn invalid_structured_schema_and_cancellation_leave_source_unchanged() {
        let temp = test_tempdir("memory-source-failure");
        let roots = fixture_roots(temp.path());
        copy_fixture(&roots);
        let invalid_path = source_structured_memory_path(&roots);
        fs::create_dir_all(invalid_path.parent().unwrap()).unwrap();
        let invalid = Connection::open(&invalid_path).unwrap();
        invalid
            .execute_batch(
                "CREATE TABLE memories (id TEXT PRIMARY KEY, content TEXT NOT NULL, created_at INTEGER NOT NULL);",
            )
            .unwrap();
        drop(invalid);
        let source_hash = hash_source_roots(&roots);
        let selection = memory_selection();
        let source = probe_legacy_source(&roots, ProbeLimits::default())
            .unwrap()
            .unwrap();
        let engine = MigrationEngine::new(roots.clone(), adapters_for_groups(&selection)).unwrap();
        assert!(engine
            .plan(&source, selection.clone(), &CancellationToken::default())
            .unwrap()
            .findings
            .iter()
            .any(|finding| !finding.migratable
                && finding
                    .detail
                    .contains("legacy structured Memory schema is not supported")));
        assert_eq!(hash_source_roots(&roots), source_hash);

        fs::remove_file(&invalid_path).unwrap();
        let source_connection = materialize_source_memory(&roots, false);
        let source_hash = hash_source_roots(&roots);
        let source = probe_legacy_source(&roots, ProbeLimits::default())
            .unwrap()
            .unwrap();
        let cancellation = CancellationToken::default();
        cancellation.cancel();
        assert!(matches!(
            engine.plan(&source, selection, &cancellation),
            Err(LegacyMigrationError::Cancelled)
        ));
        assert_eq!(hash_source_roots(&roots), source_hash);
        drop(source_connection);
    }

    #[test]
    fn file_memory_manifest_rejects_path_traversal() {
        assert!(matches!(
            validated_memory_relative_path("../MEMORY.md"),
            Err(LegacyMigrationError::PathEscape(_))
        ));
        assert!(validated_memory_relative_path("extensions/ad_hoc/notes/nested/note.md").is_ok());
        assert!(validated_memory_relative_path("extensions/ad_hoc/notes/../note.md").is_err());
    }

    #[test]
    fn content_normalization_preserves_leading_markdown_indentation() {
        assert_eq!(normalize_text("fact  \r\n\r\n"), "fact");
        assert_ne!(normalize_text("    code\n"), normalize_text("code\n"));
    }

    #[cfg(unix)]
    #[test]
    fn file_memory_rejects_symlinked_owner_file() {
        use std::os::unix::fs::symlink;

        let temp = test_tempdir("file-memory-symlink");
        let roots = fixture_roots(temp.path());
        fs::create_dir_all(&roots.legacy_home_root).unwrap();
        let memory_root = source_file_memory_root(&roots);
        fs::create_dir_all(&memory_root).unwrap();
        let outside = roots.legacy_home_root.join("outside.md");
        fs::write(&outside, "outside").unwrap();
        symlink(&outside, memory_root.join("MEMORY.md")).unwrap();

        assert!(matches!(
            plan_file_memory(&roots),
            Err(LegacyMigrationError::LinkedPath(_))
        ));
    }

    #[cfg(windows)]
    #[test]
    fn file_memory_rejects_reparse_owner_file_when_supported() {
        use std::os::windows::fs::symlink_file;

        let temp = test_tempdir("file-memory-reparse");
        let roots = fixture_roots(temp.path());
        fs::create_dir_all(&roots.legacy_home_root).unwrap();
        let memory_root = source_file_memory_root(&roots);
        fs::create_dir_all(&memory_root).unwrap();
        let outside = roots.legacy_home_root.join("outside.md");
        fs::write(&outside, "outside").unwrap();
        if symlink_file(&outside, memory_root.join("MEMORY.md")).is_err() {
            return;
        }

        assert!(matches!(
            plan_file_memory(&roots),
            Err(LegacyMigrationError::LinkedPath(_))
        ));
    }

    fn memory_selection() -> MigrationSelection {
        MigrationSelection {
            groups: BTreeSet::from([MigrationGroupId::Memory]),
        }
    }

    fn memory_record(session_id: &str, raw_memory: &str, summary: &str) -> MemoryRecord {
        MemoryRecord {
            session_id: session_id.to_string(),
            workspace_path: "C:\\fixture-workspace".to_string(),
            rollout_path: format!("C:\\fixture-workspace\\sessions\\{session_id}"),
            source_updated_at_unix_secs: 10,
            raw_memory: raw_memory.to_string(),
            rollout_summary: summary.to_string(),
            rollout_slug: Some(format!("memory-{session_id}")),
            generated_at_unix_secs: 11,
            usage_count: 0,
            last_usage_unix_secs: None,
            selected_for_phase2: 0,
            selected_for_phase2_source_updated_at: None,
        }
    }

    fn materialize_source_memory(roots: &MigrationRoots, with_wal_rows: bool) -> Connection {
        let sql_path = roots.legacy_user_root.join("data/memories/memories.sql");
        let database_path = source_structured_memory_path(roots);
        fs::create_dir_all(database_path.parent().unwrap()).unwrap();
        let connection = Connection::open(&database_path).unwrap();
        connection
            .execute_batch(&fs::read_to_string(sql_path).unwrap())
            .unwrap();
        if with_wal_rows {
            connection
                .pragma_update(None, "journal_mode", "WAL")
                .unwrap();
            connection
                .pragma_update(None, "wal_autocheckpoint", 0)
                .unwrap();
            for record in [
                memory_record(
                    "session-memory-import",
                    "Imported durable fact.",
                    "Imported summary.",
                ),
                memory_record(
                    "session-memory-duplicate",
                    "Repeated durable fact.\r\n",
                    "Repeated summary.  ",
                ),
                memory_record(
                    "session-memory-wal",
                    "WAL-only durable fact.",
                    "WAL-only summary.",
                ),
            ] {
                upsert_memory_record(&connection, &record, true).unwrap();
            }
            assert!(PathBuf::from(format!("{}-wal", database_path.display())).exists());
        }
        connection
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
            .prefix(&format!("obfm-{}-", &label[..label.len().min(4)]))
            .tempdir_in(root)
            .unwrap()
    }
}
