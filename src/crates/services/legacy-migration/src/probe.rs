use crate::{
    LegacyMigrationError, LegacyMigrationResult, MigrationOnboardingStore, MigrationRoots,
    LEGACY_PRODUCT_ID,
};
use openbitfun_product_domains::legacy_migration::{
    FindingSeverity, LegacyRootDescriptor, LegacyRootKind, LegacySourceDescriptor,
    MigrationDiagnostic, MigrationPromptChoice, MigrationRunStatus,
};
use semver::{Version, VersionReq};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const SUPPORTED_VERSION_REQ: &str = ">=0.2.0,<1.0.0";

#[derive(Debug, Clone, Copy)]
pub struct ProbeLimits {
    pub max_entries: usize,
    pub max_config_bytes: u64,
}

impl Default for ProbeLimits {
    fn default() -> Self {
        Self {
            max_entries: 4096,
            max_config_bytes: 1024 * 1024,
        }
    }
}

pub fn probe_legacy_source(
    roots: &MigrationRoots,
    limits: ProbeLimits,
) -> LegacyMigrationResult<Option<LegacySourceDescriptor>> {
    roots.validate_distinct()?;
    let version = read_source_version(&roots.legacy_user_root, limits.max_config_bytes)?;
    let markers = supported_markers(roots)?;
    if markers.iter().all(|marker| !marker.present) {
        return Ok(None);
    }

    let supported = version
        .as_deref()
        .and_then(|value| Version::parse(value).ok())
        .is_some_and(|value| {
            VersionReq::parse(SUPPORTED_VERSION_REQ)
                .expect("static version requirement is valid")
                .matches(&value)
        });

    let mut hasher = Sha256::new();
    hasher.update(LEGACY_PRODUCT_ID.as_bytes());
    hasher.update(version.as_deref().unwrap_or("unknown").as_bytes());
    let mut approximate_bytes = 0u64;
    let mut entries = 0usize;
    for root in [
        &roots.legacy_user_root,
        &roots.legacy_home_root,
        &roots.legacy_skills_root,
        &roots.legacy_ssh_root,
    ] {
        hash_path_fact(&mut hasher, root, root)?;
        let summary = bounded_tree_summary(root, limits.max_entries.saturating_sub(entries))?;
        approximate_bytes = approximate_bytes.saturating_add(summary.bytes);
        entries = entries.saturating_add(summary.entries);
    }
    for marker in &markers {
        hasher.update(marker.name.as_bytes());
        hasher.update([u8::from(marker.present)]);
    }
    let fingerprint = format!("sha256:{}", hex::encode(hasher.finalize()));
    let source_id = format!("bitfun-{}", &fingerprint[7..23]);
    let already_migrated = source_was_migrated(roots, &fingerprint)?;
    let mut diagnostics = Vec::new();
    if version.is_none() {
        diagnostics.push(MigrationDiagnostic {
            code: "source_version_missing".to_string(),
            severity: FindingSeverity::Blocking,
            message: "Legacy BitFun version could not be identified".to_string(),
            action: Some(
                "Keep the source unchanged and use a supported BitFun profile".to_string(),
            ),
            ..MigrationDiagnostic::default()
        });
    } else if !supported {
        diagnostics.push(MigrationDiagnostic {
            code: "source_version_unsupported".to_string(),
            severity: FindingSeverity::Blocking,
            message: format!("Legacy BitFun version is outside {SUPPORTED_VERSION_REQ}"),
            action: Some("Use a migrator that supports this source version".to_string()),
            ..MigrationDiagnostic::default()
        });
    }
    if entries >= limits.max_entries {
        diagnostics.push(MigrationDiagnostic {
            code: "probe_entry_limit_reached".to_string(),
            severity: FindingSeverity::Warning,
            message: "Legacy source is larger than the lightweight probe budget".to_string(),
            action: Some("Run the full read-only scan in Data Migrator".to_string()),
            ..MigrationDiagnostic::default()
        });
    }

    Ok(Some(LegacySourceDescriptor {
        source_id,
        source_fingerprint: fingerprint,
        product_id: LEGACY_PRODUCT_ID.to_string(),
        product_version: version.unwrap_or_else(|| "unknown".to_string()),
        platform: std::env::consts::OS.to_string(),
        roots: vec![
            root_descriptor(LegacyRootKind::ProductData, &roots.legacy_user_root),
            root_descriptor(LegacyRootKind::ProductHome, &roots.legacy_home_root),
            root_descriptor(LegacyRootKind::RemoteSsh, &roots.legacy_ssh_root),
        ],
        readable: true,
        supported,
        approximate_bytes,
        already_migrated,
        diagnostics,
    }))
}

#[derive(Debug)]
struct MarkerFact {
    name: &'static str,
    present: bool,
}

fn supported_markers(roots: &MigrationRoots) -> LegacyMigrationResult<Vec<MarkerFact>> {
    Ok(vec![
        marker("settings", roots.legacy_user_root.join("config/app.json")),
        marker("agents", roots.legacy_user_root.join("agents")),
        MarkerFact {
            name: "skills",
            present: directory_has_user_content(&roots.legacy_skills_root, &[".system"])?,
        },
        marker("miniapps", roots.legacy_user_root.join("data/miniapps")),
        marker(
            "workspaces",
            roots.legacy_user_root.join("data/workspace_data.json"),
        ),
        marker("sessions", roots.legacy_home_root.join("projects")),
        marker(
            "coordination",
            roots
                .legacy_user_root
                .join("data/agent-runtime/coordination.sqlite"),
        ),
        marker(
            "structured_memory",
            roots.legacy_user_root.join("data/memories/memories.sqlite"),
        ),
        marker("file_memory", roots.legacy_home_root.join("memories")),
        marker(
            "remote_connect",
            roots
                .legacy_home_root
                .join("remote_connect_persistence.json"),
        ),
        marker(
            "remote_ssh",
            roots.legacy_ssh_root.join("ssh_connections.json"),
        ),
    ])
}

fn marker(name: &'static str, path: PathBuf) -> MarkerFact {
    MarkerFact {
        name,
        present: path.exists(),
    }
}

fn directory_has_user_content(path: &Path, excluded_names: &[&str]) -> LegacyMigrationResult<bool> {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(LegacyMigrationError::io(path, error)),
    };
    for entry in entries {
        let entry = entry.map_err(|error| LegacyMigrationError::io(path, error))?;
        if !excluded_names.iter().any(|name| {
            entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(name)
        }) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn read_source_version(root: &Path, max_bytes: u64) -> LegacyMigrationResult<Option<String>> {
    let path = root.join("config/app.json");
    let metadata = match fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(LegacyMigrationError::io(&path, error)),
    };
    if metadata.len() > max_bytes {
        return Err(LegacyMigrationError::ResourceLimit(
            "legacy app configuration exceeds the probe size limit".to_string(),
        ));
    }
    let bytes = fs::read(&path).map_err(|error| LegacyMigrationError::io(&path, error))?;
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|error| LegacyMigrationError::json(&path, error))?;
    Ok(value
        .get("version")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned))
}

fn root_descriptor(kind: LegacyRootKind, path: &Path) -> LegacyRootDescriptor {
    LegacyRootDescriptor {
        kind,
        display_path: path.to_string_lossy().to_string(),
    }
}

#[derive(Debug, Default)]
struct TreeSummary {
    entries: usize,
    bytes: u64,
}

fn bounded_tree_summary(root: &Path, max_entries: usize) -> LegacyMigrationResult<TreeSummary> {
    if max_entries == 0 || !root.exists() {
        return Ok(TreeSummary::default());
    }
    let mut summary = TreeSummary::default();
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        let metadata =
            fs::symlink_metadata(&path).map_err(|error| LegacyMigrationError::io(&path, error))?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_file() {
            summary.bytes = summary.bytes.saturating_add(metadata.len());
            summary.entries += 1;
        } else if metadata.is_dir() {
            for entry in
                fs::read_dir(&path).map_err(|error| LegacyMigrationError::io(&path, error))?
            {
                let entry = entry.map_err(|error| LegacyMigrationError::io(&path, error))?;
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if should_skip_probe_name(&name) {
                    continue;
                }
                pending.push(entry.path());
                if summary.entries + pending.len() >= max_entries {
                    return Ok(summary);
                }
            }
        }
    }
    Ok(summary)
}

fn should_skip_probe_name(name: &str) -> bool {
    matches!(name, "cache" | "logs" | "cli-logs" | "temp" | "runtimes")
        || name.eq_ignore_ascii_case(".system")
        || name.starts_with("ipc-v")
        || matches!(name, "ownership" | "request-traces")
}

fn hash_path_fact(hasher: &mut Sha256, root: &Path, path: &Path) -> LegacyMigrationResult<()> {
    hasher.update(path.to_string_lossy().as_bytes());
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            hasher.update(metadata.len().to_le_bytes());
            let modified_ms = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_millis() as u64)
                .unwrap_or_default();
            hasher.update(modified_ms.to_le_bytes());
            hasher.update(
                path.strip_prefix(root)
                    .unwrap_or(path)
                    .as_os_str()
                    .as_encoded_bytes(),
            );
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => hasher.update(b"missing"),
        Err(error) => return Err(LegacyMigrationError::io(path, error)),
    }
    Ok(())
}

fn source_was_migrated(roots: &MigrationRoots, fingerprint: &str) -> LegacyMigrationResult<bool> {
    let store = MigrationOnboardingStore::new(roots.clone());
    let state = store.load()?;
    if state.source_fingerprint != fingerprint || state.choice != MigrationPromptChoice::MigrateNow
    {
        return Ok(false);
    }
    Ok(store.load_last_report()?.is_some_and(|report| {
        matches!(
            report.status,
            MigrationRunStatus::Completed | MigrationRunStatus::CompletedWithWarnings
        )
    }))
}
