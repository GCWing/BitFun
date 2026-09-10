use crate::{
    atomic_write_json, LegacyMigrationError, LegacyMigrationResult, MigrationLayout, MigrationRoots,
};
use openbitfun_product_domains::legacy_migration::{MigrationOnboardingState, MigrationRunReport};
use serde_json::{Map, Value};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

const MAX_ONBOARDING_STATE_BYTES: u64 = 64 * 1024;
const MAX_MIGRATION_REPORT_BYTES: u64 = 8 * 1024 * 1024;

/// Small persisted state that can be read before normal product storage opens.
///
/// The store intentionally retains unknown JSON fields when current code updates
/// known fields. This allows an older Desktop or Data Migrator to coexist with a
/// newer additive state shape during upgrades.
#[derive(Debug, Clone)]
pub struct MigrationOnboardingStore {
    roots: MigrationRoots,
}

impl MigrationOnboardingStore {
    pub fn new(roots: MigrationRoots) -> Self {
        Self { roots }
    }

    pub fn path(&self) -> PathBuf {
        self.roots.migration_root().join("onboarding.json")
    }

    pub fn load(&self) -> LegacyMigrationResult<MigrationOnboardingState> {
        let (_, state) = self.load_document()?;
        Ok(state)
    }

    pub fn update(
        &self,
        update: impl FnOnce(&mut MigrationOnboardingState),
    ) -> LegacyMigrationResult<MigrationOnboardingState> {
        let (mut document, mut state) = self.load_document()?;
        update(&mut state);
        let known = serde_json::to_value(&state)
            .map_err(|error| LegacyMigrationError::json(self.path(), error))?;
        let known = known.as_object().ok_or_else(|| {
            LegacyMigrationError::InvalidRequest(
                "migration onboarding state did not serialize as an object".to_string(),
            )
        })?;
        document.extend(known.clone());
        atomic_write_json(&self.path(), &Value::Object(document))?;
        Ok(state)
    }

    /// Consume a restart acknowledgement exactly once.
    ///
    /// A stale, malformed, or unrelated command-line value never suppresses the
    /// legacy probe. The matching id is cleared durably before startup proceeds.
    pub fn consume_handled_run_id(&self, run_id: &str) -> LegacyMigrationResult<bool> {
        if uuid::Uuid::parse_str(run_id).is_err() {
            return Ok(false);
        }
        let current = self.load()?;
        if current.handled_run_id.as_deref() != Some(run_id) {
            return Ok(false);
        }
        self.update(|state| state.handled_run_id = None)?;
        Ok(true)
    }

    pub fn load_report(&self, run_id: &str) -> LegacyMigrationResult<Option<MigrationRunReport>> {
        if uuid::Uuid::parse_str(run_id).is_err() {
            return Err(LegacyMigrationError::InvalidRequest(
                "migration report run id must be a UUID".to_string(),
            ));
        }
        let layout = MigrationLayout::new(&self.roots, run_id);
        let path = layout.report_path();
        ensure_path_chain_is_plain(layout.root(), &path)?;
        let file = match fs::File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(LegacyMigrationError::io(&path, error)),
        };
        let mut bytes = Vec::new();
        file.take(MAX_MIGRATION_REPORT_BYTES.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| LegacyMigrationError::io(&path, error))?;
        if bytes.len() as u64 > MAX_MIGRATION_REPORT_BYTES {
            return Err(LegacyMigrationError::ResourceLimit(
                "migration report exceeds the size limit".to_string(),
            ));
        }
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| LegacyMigrationError::json(&path, error))
    }

    pub fn load_last_report(&self) -> LegacyMigrationResult<Option<MigrationRunReport>> {
        let state = self.load()?;
        let Some(run_id) = state.last_report_run_id.as_deref() else {
            return Ok(None);
        };
        self.load_report(run_id)
    }

    fn load_document(
        &self,
    ) -> LegacyMigrationResult<(Map<String, Value>, MigrationOnboardingState)> {
        let path = self.path();
        ensure_path_chain_is_plain(self.roots.migration_root().as_path(), &path)?;
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok((Map::new(), MigrationOnboardingState::default()));
            }
            Err(error) => return Err(LegacyMigrationError::io(&path, error)),
        };
        if is_link_or_reparse(&metadata) {
            return Err(LegacyMigrationError::LinkedPath(path));
        }
        if metadata.len() > MAX_ONBOARDING_STATE_BYTES {
            return Err(LegacyMigrationError::ResourceLimit(
                "migration onboarding state exceeds the size limit".to_string(),
            ));
        }
        let bytes = fs::read(&path).map_err(|error| LegacyMigrationError::io(&path, error))?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|error| LegacyMigrationError::json(&path, error))?;
        let document = value.as_object().cloned().ok_or_else(|| {
            LegacyMigrationError::InvalidRequest(
                "migration onboarding state must be a JSON object".to_string(),
            )
        })?;
        let state = serde_json::from_value(Value::Object(document.clone()))
            .map_err(|error| LegacyMigrationError::json(&path, error))?;
        Ok((document, state))
    }
}

fn ensure_path_chain_is_plain(root: &Path, target: &Path) -> LegacyMigrationResult<()> {
    let relative = target
        .strip_prefix(root)
        .map_err(|_| LegacyMigrationError::PathEscape(target.to_path_buf()))?;
    let mut current = root.to_path_buf();
    if let Ok(metadata) = fs::symlink_metadata(&current) {
        if is_link_or_reparse(&metadata) {
            return Err(LegacyMigrationError::LinkedPath(current));
        }
    }
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(LegacyMigrationError::PathEscape(target.to_path_buf()));
        };
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if is_link_or_reparse(&metadata) => {
                return Err(LegacyMigrationError::LinkedPath(current));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(LegacyMigrationError::io(&current, error)),
        }
    }
    Ok(())
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(test)]
mod tests {
    use super::*;
    use openbitfun_product_domains::legacy_migration::{MigrationPromptChoice, MigrationRunStatus};

    fn roots(root: &Path) -> MigrationRoots {
        MigrationRoots {
            legacy_user_root: root.join("legacy/user"),
            legacy_home_root: root.join("legacy/home"),
            legacy_skills_root: root.join("legacy/skills"),
            legacy_ssh_root: root.join("legacy/ssh"),
            target_user_root: root.join("target/user"),
            target_home_root: root.join("target/home"),
            target_skills_root: root.join("target/skills"),
            target_ssh_root: root.join("target/ssh"),
        }
    }

    #[test]
    fn updates_preserve_unknown_future_fields() {
        let temporary = tempfile::tempdir().unwrap();
        let store = MigrationOnboardingStore::new(roots(temporary.path()));
        fs::create_dir_all(store.path().parent().unwrap()).unwrap();
        fs::write(&store.path(), br#"{"futureField":{"keep":true}}"#).unwrap();

        store
            .update(|state| {
                state.choice = MigrationPromptChoice::RemindLater;
                state.run_id = Some("run-id".to_string());
            })
            .unwrap();

        let value: Value = serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
        assert_eq!(value["futureField"]["keep"], true);
        assert_eq!(value["choice"], "remind_later");
        assert_eq!(value["runId"], "run-id");
    }

    #[test]
    fn restart_acknowledgement_is_consumed_only_once() {
        let temporary = tempfile::tempdir().unwrap();
        let store = MigrationOnboardingStore::new(roots(temporary.path()));
        let run_id = uuid::Uuid::new_v4().to_string();
        store
            .update(|state| state.handled_run_id = Some(run_id.clone()))
            .unwrap();

        assert!(store.consume_handled_run_id(&run_id).unwrap());
        assert!(!store.consume_handled_run_id(&run_id).unwrap());
        assert!(!store.consume_handled_run_id("not-a-uuid").unwrap());
    }

    #[test]
    fn last_report_uses_a_distinct_persisted_run_reference() {
        let temporary = tempfile::tempdir().unwrap();
        let roots = roots(temporary.path());
        let store = MigrationOnboardingStore::new(roots.clone());
        let report_run_id = uuid::Uuid::new_v4().to_string();
        let current_run_id = uuid::Uuid::new_v4().to_string();
        let layout = MigrationLayout::new(&roots, &report_run_id);
        layout.initialize().unwrap();
        atomic_write_json(
            &layout.report_path(),
            &MigrationRunReport {
                run_id: report_run_id.clone(),
                status: MigrationRunStatus::Completed,
                ..MigrationRunReport::default()
            },
        )
        .unwrap();
        store
            .update(|state| {
                state.run_id = Some(current_run_id);
                state.last_report_run_id = Some(report_run_id.clone());
            })
            .unwrap();

        assert_eq!(
            store.load_last_report().unwrap().unwrap().run_id,
            report_run_id
        );
    }

    #[test]
    fn oversized_report_is_rejected_before_deserialization() {
        let temporary = tempfile::tempdir().unwrap();
        let roots = roots(temporary.path());
        let store = MigrationOnboardingStore::new(roots.clone());
        let run_id = uuid::Uuid::new_v4().to_string();
        let layout = MigrationLayout::new(&roots, &run_id);
        layout.initialize().unwrap();
        fs::write(
            layout.report_path(),
            vec![b' '; MAX_MIGRATION_REPORT_BYTES as usize + 1],
        )
        .unwrap();

        assert!(matches!(
            store.load_report(&run_id),
            Err(LegacyMigrationError::ResourceLimit(_))
        ));
    }
}
