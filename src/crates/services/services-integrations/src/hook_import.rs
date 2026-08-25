//! Product-owned persistence for imported command Hook snapshots.

use bitfun_agent_runtime::native_hooks::{
    AgentHookScope, AgentHookSettings, AgentHookSettingsLayer, MAX_HOOKS_FILE_BYTES,
};
use bitfun_product_domains::external_hook_catalog::{ExternalHookSource, ExternalHookSourceKind};
use bitfun_product_domains::external_hook_import::{
    PreparedExternalHookAsset, MAX_EXTERNAL_HOOK_IMPORT_ASSETS,
    MAX_EXTERNAL_HOOK_IMPORT_ASSET_BYTES, MAX_EXTERNAL_HOOK_IMPORT_ASSET_DEPTH,
    MAX_EXTERNAL_HOOK_IMPORT_TOTAL_ASSET_BYTES,
};
use bitfun_product_domains::external_sources::{
    EcosystemId, ExternalSourceHealth, ExternalSourceScope, SourceKey,
};
use bitfun_services_core::json_store::JsonFileStore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;
use thiserror::Error;
use tokio::sync::RwLock;

const INDEX_SCHEMA_V1: u32 = 1;
const MAX_INDEX_BYTES: u64 = 1024 * 1024;

#[derive(Clone)]
pub struct HookImportWrite {
    pub source: ExternalHookSource,
    pub behavior_version: String,
    pub hooks_json: Vec<u8>,
    pub assets: Vec<PreparedExternalHookAsset>,
}

impl std::fmt::Debug for HookImportWrite {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HookImportWrite")
            .field("source", &self.source.key)
            .field("behavior_version", &self.behavior_version)
            .field("hooks_json", &"<redacted>")
            .field("asset_count", &self.assets.len())
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct HookImportRecord {
    pub import_id: String,
    pub source: ExternalHookSource,
    pub enabled: bool,
    pub behavior_version: String,
    pub bundle_path: PathBuf,
    content_digest: String,
    bundle_valid: bool,
}

impl HookImportRecord {
    /// Whether the exact indexed managed snapshot passed bounded content verification.
    pub fn bundle_is_valid(&self) -> bool {
        self.bundle_valid
    }
}

#[derive(Debug, Clone, Default)]
pub struct HookImportStoreSnapshot {
    pub generation: u64,
    pub imports: Vec<HookImportRecord>,
    pub corrupt_marker: Option<String>,
}

#[derive(Debug, Clone)]
pub enum HookImportApply {
    Applied,
    Unchanged,
}

#[derive(Debug, Error)]
pub enum HookImportStoreError {
    #[error("Hook import store generation changed")]
    StaleGeneration,
    #[error("Hook import store is corrupt")]
    Corrupt,
    #[error("invalid Hook import: {0}")]
    InvalidInput(&'static str),
    #[error("Hook import IO failed: {0}")]
    Io(String),
}

pub struct HookImportStore {
    root: PathBuf,
    scope: ExternalSourceScope,
    state: RwLock<CachedStore>,
}

struct CachedStore {
    snapshot: HookImportStoreSnapshot,
    fingerprint: Option<IndexFingerprint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IndexFingerprint {
    len: u64,
    modified_nanos: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoreIndexV1 {
    schema_version: u32,
    generation: u64,
    imports: Vec<StoreRecordV1>,
}

impl Default for StoreIndexV1 {
    fn default() -> Self {
        Self {
            schema_version: INDEX_SCHEMA_V1,
            generation: 0,
            imports: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoreRecordV1 {
    import_id: String,
    source: SourceKey,
    ecosystem_id: EcosystemId,
    display_name: String,
    source_kind: ExternalHookSourceKind,
    scope: ExternalSourceScope,
    location_hint: String,
    catalog_content_version: String,
    behavior_version: String,
    bundle_digest: String,
    content_digest: String,
    enabled: bool,
}

enum LoadedIndex {
    Missing,
    Ready(StoreIndexV1),
    Corrupt(String),
}

impl HookImportStore {
    pub async fn open(
        root: PathBuf,
        scope: ExternalSourceScope,
    ) -> Result<Self, HookImportStoreError> {
        if !root.is_absolute() {
            return Err(HookImportStoreError::InvalidInput(
                "store root must be absolute",
            ));
        }
        validate_store_root(&root).await?;
        let loaded = load_index(&root.join("index.json")).await?;
        let mut snapshot = snapshot_from_loaded(&root, scope, &loaded)?;
        verify_snapshot_bundles(&root, &mut snapshot).await;
        let fingerprint = index_fingerprint(&root.join("index.json")).await?;
        Ok(Self {
            root,
            scope,
            state: RwLock::new(CachedStore {
                snapshot,
                fingerprint,
            }),
        })
    }

    pub async fn snapshot(&self) -> Result<HookImportStoreSnapshot, HookImportStoreError> {
        self.refresh_if_changed().await?;
        Ok(self.state.read().await.snapshot.clone())
    }

    pub fn stable_import_id(source: &SourceKey) -> String {
        import_id(source)
    }

    pub fn planned_bundle_path(&self, source: &SourceKey, behavior_version: &str) -> PathBuf {
        self.root
            .join("bundles")
            .join(import_id(source))
            .join(behavior_directory(behavior_version))
    }

    pub async fn apply(
        &self,
        expected_generation: u64,
        write: HookImportWrite,
    ) -> Result<HookImportApply, HookImportStoreError> {
        validate_write(self.scope, &write)?;
        ensure_store_root(&self.root).await?;
        let index_path = self.root.join("index.json");
        let json_store = JsonFileStore;
        let _lock = json_store
            .acquire_cross_process_lock(&index_path)
            .await
            .map_err(io_error)?;
        let mut index = match load_index(&index_path).await? {
            LoadedIndex::Missing => StoreIndexV1::default(),
            LoadedIndex::Ready(index) => {
                validate_index(self.scope, &index)?;
                index
            }
            LoadedIndex::Corrupt(_) => return Err(HookImportStoreError::Corrupt),
        };
        if index.generation != expected_generation {
            return Err(HookImportStoreError::StaleGeneration);
        }

        let import_id = import_id(&write.source.key);
        let bundle_digest = bundle_digest(&write);
        let content_digest = bundle_content_digest(&write);
        let bundle_path = self
            .root
            .join("bundles")
            .join(&import_id)
            .join(&bundle_digest);
        let index_is_unchanged = index.imports.iter().any(|record| {
            record.import_id == import_id
                && record.behavior_version == write.behavior_version
                && record.bundle_digest == bundle_digest
                && record.content_digest == content_digest
        });
        if index_is_unchanged
            && validate_bundle_content(&self.root, &bundle_path, &content_digest)
                .await
                .is_ok()
        {
            let snapshot = verified_snapshot_from_index(&self.root, self.scope, &index).await?;
            self.replace_cached(snapshot).await?;
            return Ok(HookImportApply::Unchanged);
        }
        let next_generation = index
            .generation
            .checked_add(1)
            .ok_or(HookImportStoreError::InvalidInput("generation overflow"))?;
        let publication = publish_bundle(&self.root, &bundle_path, &write, &content_digest).await?;
        let previous_bundle = index
            .imports
            .iter()
            .find(|record| record.import_id == import_id)
            .map(|record| record.bundle_digest.clone());
        let enabled = index
            .imports
            .iter()
            .find(|record| record.import_id == import_id)
            .is_none_or(|record| record.enabled);
        index.imports.retain(|record| record.import_id != import_id);
        index.imports.push(StoreRecordV1::from_write(
            import_id.clone(),
            bundle_digest.clone(),
            content_digest,
            enabled,
            &write,
        ));
        index
            .imports
            .sort_by(|left, right| left.import_id.cmp(&right.import_id));
        index.generation = next_generation;
        if let Err(error) = json_store.write_atomic_strict(&index_path, &index).await {
            let publication_error = io_error(error);
            if let Err(rollback_error) = publication.rollback().await {
                return Err(HookImportStoreError::Io(format!(
                    "index publication failed ({publication_error}); restoring the indexed bundle failed ({rollback_error})"
                )));
            }
            return Err(publication_error);
        }
        let snapshot = verified_snapshot_from_index(&self.root, self.scope, &index).await?;
        self.replace_cached(snapshot).await?;
        publication.finalize().await;

        if let Some(previous_bundle) = previous_bundle.filter(|value| value != &bundle_digest) {
            let old_path = self
                .root
                .join("bundles")
                .join(&import_id)
                .join(previous_bundle);
            let _ = remove_owned_path(&self.root, &old_path).await;
        }
        Ok(HookImportApply::Applied)
    }

    pub async fn set_enabled(
        &self,
        expected_generation: u64,
        import_id: &str,
        enabled: bool,
    ) -> Result<HookImportStoreSnapshot, HookImportStoreError> {
        if !safe_component(import_id) {
            return Err(HookImportStoreError::InvalidInput("import id"));
        }
        if enabled {
            let snapshot = self.snapshot().await?;
            let record = snapshot
                .imports
                .iter()
                .find(|record| record.import_id == import_id)
                .ok_or(HookImportStoreError::InvalidInput("unknown import id"))?;
            if !record.bundle_valid
                || validate_bundle_content(&self.root, &record.bundle_path, &record.content_digest)
                    .await
                    .is_err()
            {
                return Err(HookImportStoreError::InvalidInput(
                    "bundle missing or invalid",
                ));
            }
        }
        self.update_index(expected_generation, |index| {
            let record = index
                .imports
                .iter_mut()
                .find(|record| record.import_id == import_id)
                .ok_or(HookImportStoreError::InvalidInput("unknown import id"))?;
            record.enabled = enabled;
            Ok(())
        })
        .await
    }

    pub async fn remove(
        &self,
        expected_generation: u64,
        import_id: &str,
    ) -> Result<HookImportStoreSnapshot, HookImportStoreError> {
        if !safe_component(import_id) {
            return Err(HookImportStoreError::InvalidInput("import id"));
        }
        ensure_store_root(&self.root).await?;
        let index_path = self.root.join("index.json");
        let json_store = JsonFileStore;
        let _lock = json_store
            .acquire_cross_process_lock(&index_path)
            .await
            .map_err(io_error)?;
        let mut index = match load_index(&index_path).await? {
            LoadedIndex::Missing => StoreIndexV1::default(),
            LoadedIndex::Ready(index) => {
                validate_index(self.scope, &index)?;
                index
            }
            LoadedIndex::Corrupt(_) => return Err(HookImportStoreError::Corrupt),
        };
        if index.generation != expected_generation {
            return Err(HookImportStoreError::StaleGeneration);
        }
        let removed_digest = index
            .imports
            .iter()
            .find(|record| record.import_id == import_id)
            .map(|record| record.bundle_digest.clone())
            .ok_or(HookImportStoreError::InvalidInput("unknown import id"))?;
        index.imports.retain(|record| record.import_id != import_id);
        index.generation = index
            .generation
            .checked_add(1)
            .ok_or(HookImportStoreError::InvalidInput("generation overflow"))?;
        json_store
            .write_atomic_strict(&index_path, &index)
            .await
            .map_err(io_error)?;
        let snapshot = verified_snapshot_from_index(&self.root, self.scope, &index).await?;
        self.replace_cached(snapshot.clone()).await?;
        let removed_path = self
            .root
            .join("bundles")
            .join(import_id)
            .join(removed_digest);
        let _ = remove_owned_path(&self.root, &removed_path).await;
        Ok(snapshot)
    }

    pub async fn reset_corrupt(&self) -> Result<HookImportStoreSnapshot, HookImportStoreError> {
        ensure_store_root(&self.root).await?;
        let index_path = self.root.join("index.json");
        let json_store = JsonFileStore;
        let _lock = json_store
            .acquire_cross_process_lock(&index_path)
            .await
            .map_err(io_error)?;
        if !matches!(load_index(&index_path).await?, LoadedIndex::Corrupt(_)) {
            return Err(HookImportStoreError::InvalidInput("store is not corrupt"));
        }
        let mut index = StoreIndexV1::default();
        index.generation = reset_generation();
        json_store
            .write_atomic_strict(&index_path, &index)
            .await
            .map_err(io_error)?;
        let snapshot = verified_snapshot_from_index(&self.root, self.scope, &index).await?;
        self.replace_cached(snapshot.clone()).await?;
        Ok(snapshot)
    }

    pub async fn enabled_layers(
        &self,
    ) -> Result<Vec<AgentHookSettingsLayer>, HookImportStoreError> {
        let snapshot = self.snapshot().await?;
        let mut layers = Vec::new();
        for record in snapshot
            .imports
            .into_iter()
            .filter(|record| record.enabled && record.bundle_valid)
        {
            let hooks_path = record.bundle_path.join("hooks.json");
            let bytes =
                match read_verified_bundle(&self.root, &record.bundle_path, &record.content_digest)
                    .await
                {
                    Ok(bytes) => bytes,
                    Err(_) => continue,
                };
            let layer = AgentHookSettingsLayer {
                scope: hook_scope(self.scope),
                source: hooks_path.to_string_lossy().to_string(),
                bytes,
            };
            let (settings, issues) = AgentHookSettings::from_layers(std::slice::from_ref(&layer));
            if issues.is_empty() && !settings.is_empty() {
                layers.push(layer);
            }
        }
        Ok(layers)
    }

    async fn update_index(
        &self,
        expected_generation: u64,
        mutate: impl FnOnce(&mut StoreIndexV1) -> Result<(), HookImportStoreError>,
    ) -> Result<HookImportStoreSnapshot, HookImportStoreError> {
        ensure_store_root(&self.root).await?;
        let index_path = self.root.join("index.json");
        let json_store = JsonFileStore;
        let _lock = json_store
            .acquire_cross_process_lock(&index_path)
            .await
            .map_err(io_error)?;
        let mut index = match load_index(&index_path).await? {
            LoadedIndex::Missing => StoreIndexV1::default(),
            LoadedIndex::Ready(index) => {
                validate_index(self.scope, &index)?;
                index
            }
            LoadedIndex::Corrupt(_) => return Err(HookImportStoreError::Corrupt),
        };
        if index.generation != expected_generation {
            return Err(HookImportStoreError::StaleGeneration);
        }
        mutate(&mut index)?;
        index.generation = index
            .generation
            .checked_add(1)
            .ok_or(HookImportStoreError::InvalidInput("generation overflow"))?;
        json_store
            .write_atomic_strict(&index_path, &index)
            .await
            .map_err(io_error)?;
        let snapshot = verified_snapshot_from_index(&self.root, self.scope, &index).await?;
        self.replace_cached(snapshot.clone()).await?;
        Ok(snapshot)
    }

    async fn refresh_if_changed(&self) -> Result<(), HookImportStoreError> {
        let index_path = self.root.join("index.json");
        let fingerprint = index_fingerprint(&index_path).await?;
        if self.state.read().await.fingerprint == fingerprint {
            return Ok(());
        }
        validate_store_root(&self.root).await?;
        let loaded = load_index(&index_path).await?;
        let mut snapshot = snapshot_from_loaded(&self.root, self.scope, &loaded)?;
        verify_snapshot_bundles(&self.root, &mut snapshot).await;
        *self.state.write().await = CachedStore {
            snapshot,
            fingerprint,
        };
        Ok(())
    }

    async fn replace_cached(
        &self,
        snapshot: HookImportStoreSnapshot,
    ) -> Result<(), HookImportStoreError> {
        let fingerprint = index_fingerprint(&self.root.join("index.json")).await?;
        *self.state.write().await = CachedStore {
            snapshot,
            fingerprint,
        };
        Ok(())
    }
}

impl StoreRecordV1 {
    fn from_write(
        import_id: String,
        bundle_digest: String,
        content_digest: String,
        enabled: bool,
        write: &HookImportWrite,
    ) -> Self {
        Self {
            import_id,
            source: write.source.key.clone(),
            ecosystem_id: write.source.ecosystem_id.clone(),
            display_name: write.source.display_name.clone(),
            source_kind: write.source.source_kind,
            scope: write.source.scope,
            location_hint: write.source.location_hint.clone(),
            catalog_content_version: write.source.content_version.clone(),
            behavior_version: write.behavior_version.clone(),
            bundle_digest,
            content_digest,
            enabled,
        }
    }
}

struct BundlePublication {
    root: PathBuf,
    final_path: PathBuf,
    retired_path: Option<PathBuf>,
    changed: bool,
}

impl BundlePublication {
    async fn rollback(self) -> Result<(), HookImportStoreError> {
        if !self.changed {
            return Ok(());
        }
        remove_owned_path(&self.root, &self.final_path).await?;
        if let Some(retired_path) = self.retired_path {
            tokio::fs::rename(retired_path, self.final_path)
                .await
                .map_err(io_error)?;
        }
        Ok(())
    }

    async fn finalize(self) {
        if let Some(retired_path) = self.retired_path {
            let _ = remove_owned_path(&self.root, &retired_path).await;
        }
    }
}

async fn publish_bundle(
    root: &Path,
    final_path: &Path,
    write: &HookImportWrite,
    content_digest: &str,
) -> Result<BundlePublication, HookImportStoreError> {
    if tokio::fs::symlink_metadata(final_path).await.is_ok()
        && validate_bundle_content(root, final_path, content_digest)
            .await
            .is_ok()
    {
        return Ok(BundlePublication {
            root: root.to_path_buf(),
            final_path: final_path.to_path_buf(),
            retired_path: None,
            changed: false,
        });
    }
    let staging = root
        .join(".staging")
        .join(format!("import-{}", uuid::Uuid::new_v4()));
    ensure_owned_directory(root, &staging).await?;
    let result = async {
        tokio::fs::write(staging.join("hooks.json"), &write.hooks_json)
            .await
            .map_err(|error| HookImportStoreError::Io(error.to_string()))?;
        for asset in &write.assets {
            let target = staging.join(&asset.relative_path);
            if let Some(parent) = target.parent() {
                ensure_owned_directory(&staging, parent).await?;
            }
            tokio::fs::write(target, &asset.bytes)
                .await
                .map_err(|error| HookImportStoreError::Io(error.to_string()))?;
        }
        validate_bundle_content(root, &staging, content_digest).await?;
        let parent = final_path
            .parent()
            .ok_or(HookImportStoreError::InvalidInput("bundle path"))?;
        ensure_owned_directory(root, parent).await?;
        let retired = if tokio::fs::symlink_metadata(final_path).await.is_ok() {
            validate_owned_directory(root, final_path).await?;
            let retired = root
                .join(".staging")
                .join(format!("retired-{}", uuid::Uuid::new_v4()));
            tokio::fs::rename(final_path, &retired)
                .await
                .map_err(io_error)?;
            Some(retired)
        } else {
            None
        };
        match tokio::fs::rename(&staging, final_path).await {
            Ok(()) => Ok(BundlePublication {
                root: root.to_path_buf(),
                final_path: final_path.to_path_buf(),
                retired_path: retired,
                changed: true,
            }),
            Err(error) => {
                if let Some(retired) = &retired {
                    tokio::fs::rename(retired, final_path).await.map_err(|restore| {
                        HookImportStoreError::Io(format!(
                            "bundle publication failed ({error}); restoring the indexed bundle failed ({restore})"
                        ))
                    })?;
                } else if tokio::fs::symlink_metadata(final_path).await.is_ok() {
                    validate_bundle_content(root, final_path, content_digest).await?;
                    return Ok(BundlePublication {
                        root: root.to_path_buf(),
                        final_path: final_path.to_path_buf(),
                        retired_path: None,
                        changed: false,
                    });
                }
                Err(HookImportStoreError::Io(error.to_string()))
            }
        }
    }
    .await;
    if result.is_err() {
        let _ = remove_owned_path(root, &staging).await;
    }
    result
}

async fn validate_bundle_content(
    root: &Path,
    path: &Path,
    expected_digest: &str,
) -> Result<(), HookImportStoreError> {
    read_verified_bundle(root, path, expected_digest)
        .await
        .map(drop)
}

async fn read_verified_bundle(
    root: &Path,
    path: &Path,
    expected_digest: &str,
) -> Result<Vec<u8>, HookImportStoreError> {
    validate_owned_directory(root, path).await?;
    let mut entries = tokio::fs::read_dir(path).await.map_err(io_error)?;
    while let Some(entry) = entries.next_entry().await.map_err(io_error)? {
        let name = entry.file_name();
        if name != "hooks.json" && name != "hooks" {
            return Err(HookImportStoreError::InvalidInput("bundle entry"));
        }
        let metadata = tokio::fs::symlink_metadata(entry.path())
            .await
            .map_err(io_error)?;
        if is_unsupported_link(&metadata)
            || (name == "hooks.json" && !metadata.is_file())
            || (name == "hooks" && !metadata.is_dir())
        {
            return Err(HookImportStoreError::InvalidInput("bundle entry"));
        }
    }
    let hooks_json = read_bounded(&path.join("hooks.json"), MAX_HOOKS_FILE_BYTES as u64)
        .await?
        .ok_or(HookImportStoreError::InvalidInput("bundle hooks missing"))?;
    validate_native_hooks(&hooks_json)?;
    let assets = read_bundle_assets(root, path).await?;
    let observed = content_digest_from_parts(&hooks_json, &assets);
    if observed != expected_digest {
        return Err(HookImportStoreError::InvalidInput("bundle content digest"));
    }
    Ok(hooks_json)
}

async fn read_bundle_assets(
    root: &Path,
    bundle: &Path,
) -> Result<Vec<PreparedExternalHookAsset>, HookImportStoreError> {
    let assets_root = bundle.join("hooks");
    let metadata = match tokio::fs::symlink_metadata(&assets_root).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(io_error(error)),
    };
    if !metadata.is_dir() || is_unsupported_link(&metadata) {
        return Err(HookImportStoreError::InvalidInput("bundle assets"));
    }
    validate_owned_directory(root, &assets_root).await?;
    let mut pending = vec![assets_root];
    let mut assets = Vec::new();
    let mut total_bytes = 0usize;
    while let Some(directory) = pending.pop() {
        let mut entries = tokio::fs::read_dir(&directory).await.map_err(io_error)?;
        while let Some(entry) = entries.next_entry().await.map_err(io_error)? {
            let path = entry.path();
            let relative_path = path
                .strip_prefix(bundle)
                .map_err(|_| HookImportStoreError::InvalidInput("bundle asset path"))?
                .to_path_buf();
            let depth = relative_path.components().count();
            if depth > MAX_EXTERNAL_HOOK_IMPORT_ASSET_DEPTH
                || relative_path
                    .components()
                    .any(|component| !matches!(component, Component::Normal(_)))
            {
                return Err(HookImportStoreError::InvalidInput("bundle asset path"));
            }
            let metadata = tokio::fs::symlink_metadata(&path).await.map_err(io_error)?;
            if is_unsupported_link(&metadata) {
                return Err(HookImportStoreError::InvalidInput("bundle asset link"));
            }
            if metadata.is_dir() {
                pending.push(path);
                continue;
            }
            if !metadata.is_file() || assets.len() >= MAX_EXTERNAL_HOOK_IMPORT_ASSETS {
                return Err(HookImportStoreError::InvalidInput("bundle asset"));
            }
            let bytes = read_bounded(&path, MAX_EXTERNAL_HOOK_IMPORT_ASSET_BYTES as u64)
                .await?
                .ok_or(HookImportStoreError::InvalidInput("bundle asset"))?;
            total_bytes = total_bytes
                .checked_add(bytes.len())
                .ok_or(HookImportStoreError::InvalidInput("bundle asset bytes"))?;
            if total_bytes > MAX_EXTERNAL_HOOK_IMPORT_TOTAL_ASSET_BYTES {
                return Err(HookImportStoreError::InvalidInput("bundle asset bytes"));
            }
            assets.push(PreparedExternalHookAsset {
                relative_path,
                bytes,
            });
        }
    }
    Ok(assets)
}

async fn verified_snapshot_from_index(
    root: &Path,
    scope: ExternalSourceScope,
    index: &StoreIndexV1,
) -> Result<HookImportStoreSnapshot, HookImportStoreError> {
    let mut snapshot = snapshot_from_index(root, scope, index)?;
    verify_snapshot_bundles(root, &mut snapshot).await;
    Ok(snapshot)
}

async fn verify_snapshot_bundles(root: &Path, snapshot: &mut HookImportStoreSnapshot) {
    for record in &mut snapshot.imports {
        record.bundle_valid =
            validate_bundle_content(root, &record.bundle_path, &record.content_digest)
                .await
                .is_ok();
    }
}

fn bundle_content_digest(write: &HookImportWrite) -> String {
    content_digest_from_parts(&write.hooks_json, &write.assets)
}

fn content_digest_from_parts(hooks_json: &[u8], assets: &[PreparedExternalHookAsset]) -> String {
    let mut ordered = assets.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let mut hasher = Sha256::new();
    digest_part(&mut hasher, b"bitfun-hook-import-bundle-v1");
    digest_part(&mut hasher, hooks_json);
    for asset in ordered {
        digest_part(
            &mut hasher,
            asset
                .relative_path
                .to_string_lossy()
                .replace('\\', "/")
                .as_bytes(),
        );
        digest_part(&mut hasher, &asset.bytes);
    }
    hex::encode(hasher.finalize())
}

fn digest_part(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

async fn validate_store_root(root: &Path) -> Result<(), HookImportStoreError> {
    match tokio::fs::symlink_metadata(root).await {
        Ok(metadata) if metadata.is_dir() && !is_unsupported_link(&metadata) => Ok(()),
        Ok(_) => Err(HookImportStoreError::InvalidInput("store root")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(error)),
    }
}

async fn ensure_store_root(root: &Path) -> Result<(), HookImportStoreError> {
    tokio::fs::create_dir_all(root).await.map_err(io_error)?;
    validate_store_root(root).await
}

async fn ensure_owned_directory(root: &Path, directory: &Path) -> Result<(), HookImportStoreError> {
    ensure_store_root(root).await?;
    walk_owned_directory(root, directory, true).await
}

async fn validate_owned_directory(
    root: &Path,
    directory: &Path,
) -> Result<(), HookImportStoreError> {
    validate_store_root(root).await?;
    walk_owned_directory(root, directory, false).await
}

async fn walk_owned_directory(
    root: &Path,
    directory: &Path,
    create: bool,
) -> Result<(), HookImportStoreError> {
    let relative = directory
        .strip_prefix(root)
        .map_err(|_| HookImportStoreError::InvalidInput("managed directory"))?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(HookImportStoreError::InvalidInput("managed directory"));
        };
        current.push(component);
        if create {
            match tokio::fs::create_dir(&current).await {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(io_error(error)),
            }
        }
        let metadata = tokio::fs::symlink_metadata(&current)
            .await
            .map_err(io_error)?;
        if !metadata.is_dir() || is_unsupported_link(&metadata) {
            return Err(HookImportStoreError::InvalidInput("managed directory"));
        }
    }
    Ok(())
}

fn is_unsupported_link(metadata: &std::fs::Metadata) -> bool {
    metadata.file_type().is_symlink() || is_windows_reparse_point(metadata)
}

#[cfg(windows)]
fn is_windows_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_windows_reparse_point(_metadata: &std::fs::Metadata) -> bool {
    false
}

fn validate_native_hooks(bytes: &[u8]) -> Result<(), HookImportStoreError> {
    let layer = AgentHookSettingsLayer {
        scope: AgentHookScope::User,
        source: "managed-hook-import".to_string(),
        bytes: bytes.to_vec(),
    };
    let (settings, issues) = AgentHookSettings::from_layers(&[layer]);
    if !issues.is_empty() || settings.is_empty() {
        return Err(HookImportStoreError::InvalidInput("native Hook document"));
    }
    Ok(())
}

fn validate_write(
    scope: ExternalSourceScope,
    write: &HookImportWrite,
) -> Result<(), HookImportStoreError> {
    write
        .source
        .validate()
        .map_err(|_| HookImportStoreError::InvalidInput("source"))?;
    if !scope_accepts(scope, write.source.scope) {
        return Err(HookImportStoreError::InvalidInput("source scope"));
    }
    validate_native_hooks(&write.hooks_json)?;
    if write.assets.len() > MAX_EXTERNAL_HOOK_IMPORT_ASSETS {
        return Err(HookImportStoreError::InvalidInput("asset count"));
    }
    let mut paths = BTreeSet::new();
    let mut total_bytes = 0usize;
    for asset in &write.assets {
        if asset.relative_path.is_absolute()
            || asset.relative_path.components().count() > MAX_EXTERNAL_HOOK_IMPORT_ASSET_DEPTH
            || asset
                .relative_path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
            || asset.bytes.len() > MAX_EXTERNAL_HOOK_IMPORT_ASSET_BYTES
            || !paths.insert(asset.relative_path.clone())
        {
            return Err(HookImportStoreError::InvalidInput("asset"));
        }
        total_bytes = total_bytes
            .checked_add(asset.bytes.len())
            .ok_or(HookImportStoreError::InvalidInput("asset bytes"))?;
    }
    if total_bytes > MAX_EXTERNAL_HOOK_IMPORT_TOTAL_ASSET_BYTES {
        return Err(HookImportStoreError::InvalidInput("asset bytes"));
    }
    Ok(())
}

fn validate_index(
    scope: ExternalSourceScope,
    index: &StoreIndexV1,
) -> Result<(), HookImportStoreError> {
    if index.schema_version != INDEX_SCHEMA_V1 || index.imports.len() > 2048 {
        return Err(HookImportStoreError::Corrupt);
    }
    let mut ids = BTreeSet::new();
    for record in &index.imports {
        if !scope_accepts(scope, record.scope)
            || record.import_id != import_id(&record.source)
            || !ids.insert(&record.import_id)
            || !safe_component(&record.import_id)
            || !safe_component(&record.bundle_digest)
            || !safe_component(&record.content_digest)
        {
            return Err(HookImportStoreError::Corrupt);
        }
    }
    Ok(())
}

fn snapshot_from_loaded(
    root: &Path,
    scope: ExternalSourceScope,
    loaded: &LoadedIndex,
) -> Result<HookImportStoreSnapshot, HookImportStoreError> {
    match loaded {
        LoadedIndex::Missing => Ok(HookImportStoreSnapshot::default()),
        LoadedIndex::Ready(index) => match snapshot_from_index(root, scope, index) {
            Ok(snapshot) => Ok(snapshot),
            Err(HookImportStoreError::Corrupt) => Ok(HookImportStoreSnapshot {
                corrupt_marker: Some(invalid_index_marker(index)),
                ..HookImportStoreSnapshot::default()
            }),
            Err(error) => Err(error),
        },
        LoadedIndex::Corrupt(marker) => Ok(HookImportStoreSnapshot {
            corrupt_marker: Some(marker.clone()),
            ..HookImportStoreSnapshot::default()
        }),
    }
}

fn invalid_index_marker(index: &StoreIndexV1) -> String {
    let bytes = serde_json::to_vec(index).unwrap_or_default();
    format!("sha256:{}", hex::encode(Sha256::digest(&bytes)))
}

fn snapshot_from_index(
    root: &Path,
    scope: ExternalSourceScope,
    index: &StoreIndexV1,
) -> Result<HookImportStoreSnapshot, HookImportStoreError> {
    validate_index(scope, index)?;
    Ok(HookImportStoreSnapshot {
        generation: index.generation,
        imports: index
            .imports
            .iter()
            .map(|record| HookImportRecord {
                import_id: record.import_id.clone(),
                source: ExternalHookSource {
                    key: record.source.clone(),
                    ecosystem_id: record.ecosystem_id.clone(),
                    display_name: record.display_name.clone(),
                    source_kind: record.source_kind,
                    scope: record.scope,
                    location_hint: record.location_hint.clone(),
                    health: ExternalSourceHealth::Available,
                    content_version: record.catalog_content_version.clone(),
                    diagnostics: Vec::new(),
                },
                enabled: record.enabled,
                behavior_version: record.behavior_version.clone(),
                bundle_path: root
                    .join("bundles")
                    .join(&record.import_id)
                    .join(&record.bundle_digest),
                content_digest: record.content_digest.clone(),
                bundle_valid: false,
            })
            .collect(),
        corrupt_marker: None,
    })
}

async fn load_index(path: &Path) -> Result<LoadedIndex, HookImportStoreError> {
    let metadata = match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(LoadedIndex::Missing)
        }
        Err(error) => return Err(HookImportStoreError::Io(error.to_string())),
    };
    if !metadata.is_file() || is_unsupported_link(&metadata) || metadata.len() > MAX_INDEX_BYTES {
        return Ok(LoadedIndex::Corrupt(format!(
            "sha256:{}",
            hex::encode(Sha256::digest(
                format!("invalid:{}", metadata.len()).as_bytes()
            ))
        )));
    }
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|error| HookImportStoreError::Io(error.to_string()))?;
    match serde_json::from_slice::<StoreIndexV1>(&bytes) {
        Ok(index) => Ok(LoadedIndex::Ready(index)),
        Err(_) => Ok(LoadedIndex::Corrupt(format!(
            "sha256:{}",
            hex::encode(Sha256::digest(&bytes))
        ))),
    }
}

async fn read_bounded(
    path: &Path,
    max_bytes: u64,
) -> Result<Option<Vec<u8>>, HookImportStoreError> {
    let metadata = match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(HookImportStoreError::Io(error.to_string())),
    };
    if !metadata.is_file() || is_unsupported_link(&metadata) || metadata.len() > max_bytes {
        return Ok(None);
    }
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|error| HookImportStoreError::Io(error.to_string()))?;
    (bytes.len() as u64 <= max_bytes)
        .then_some(bytes)
        .map(Some)
        .ok_or(HookImportStoreError::InvalidInput("file grew past budget"))
}

async fn index_fingerprint(path: &Path) -> Result<Option<IndexFingerprint>, HookImportStoreError> {
    match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) => Ok(Some(IndexFingerprint {
            len: metadata.len(),
            modified_nanos: metadata
                .modified()
                .ok()
                .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |value| value.as_nanos()),
        })),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(HookImportStoreError::Io(error.to_string())),
    }
}

async fn remove_owned_path(root: &Path, path: &Path) -> Result<(), HookImportStoreError> {
    let parent = path
        .parent()
        .ok_or(HookImportStoreError::InvalidInput("managed path"))?;
    validate_owned_directory(root, parent).await?;
    let metadata = match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(io_error(error)),
    };
    if is_unsupported_link(&metadata) {
        return Err(HookImportStoreError::InvalidInput("managed path link"));
    }
    if !metadata.is_dir() {
        tokio::fs::remove_file(path).await.map_err(io_error)
    } else {
        tokio::fs::remove_dir_all(path).await.map_err(io_error)
    }
}

fn bundle_digest(write: &HookImportWrite) -> String {
    behavior_directory(&write.behavior_version)
}

fn behavior_directory(behavior_version: &str) -> String {
    hex::encode(Sha256::digest(behavior_version.as_bytes()))
}

fn reset_generation() -> u64 {
    let value = uuid::Uuid::new_v4().as_u128() as u64 & (u64::MAX >> 1);
    value.max(1)
}

fn import_id(source: &SourceKey) -> String {
    format!(
        "hook-{}",
        &hex::encode(Sha256::digest(source.stable_key().as_bytes()))[..24]
    )
}

fn safe_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|value| value.is_ascii_alphanumeric() || value == b'-')
}

fn hook_scope(scope: ExternalSourceScope) -> AgentHookScope {
    match scope {
        ExternalSourceScope::UserGlobal => AgentHookScope::User,
        _ => AgentHookScope::Project,
    }
}

fn scope_accepts(store_scope: ExternalSourceScope, source_scope: ExternalSourceScope) -> bool {
    match store_scope {
        ExternalSourceScope::UserGlobal => source_scope == ExternalSourceScope::UserGlobal,
        ExternalSourceScope::Project | ExternalSourceScope::WorkspaceLocal => matches!(
            source_scope,
            ExternalSourceScope::Project | ExternalSourceScope::WorkspaceLocal
        ),
        _ => source_scope == store_scope,
    }
}

fn io_error(error: impl std::fmt::Display) -> HookImportStoreError {
    HookImportStoreError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_product_domains::external_hook_catalog::{
        ExternalHookSource, ExternalHookSourceKind,
    };
    use bitfun_product_domains::external_hook_import::PreparedExternalHookAsset;
    use bitfun_product_domains::external_sources::{
        EcosystemId, ExternalSourceHealth, ExternalSourceScope, SourceKey,
    };
    use tempfile::tempdir;

    fn source(scope: ExternalSourceScope) -> ExternalHookSource {
        ExternalHookSource {
            key: SourceKey::new("codex.hooks", "user-hooks-json").unwrap(),
            ecosystem_id: EcosystemId::new("codex").unwrap(),
            display_name: "Codex user hooks".to_string(),
            source_kind: ExternalHookSourceKind::HooksFile,
            scope,
            location_hint: "~/.codex/hooks.json".to_string(),
            health: ExternalSourceHealth::Available,
            content_version: "sha256:catalog".to_string(),
            diagnostics: Vec::new(),
        }
    }

    fn write(scope: ExternalSourceScope, command: &str) -> HookImportWrite {
        HookImportWrite {
            source: source(scope),
            behavior_version: format!("sha256:{command}"),
            hooks_json: format!(
                r#"{{"hooks":{{"PreToolUse":[{{"hooks":[{{"type":"command","command":"{command}"}}]}}]}}}}"#
            )
            .into_bytes(),
            assets: vec![PreparedExternalHookAsset {
                relative_path: "hooks/check.py".into(),
                bytes: b"print('ok')".to_vec(),
            }],
        }
    }

    #[tokio::test]
    async fn apply_is_generation_fenced_idempotent_and_restart_safe() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("hook-imports");
        let store = HookImportStore::open(root.clone(), ExternalSourceScope::UserGlobal)
            .await
            .unwrap();
        assert_eq!(store.snapshot().await.unwrap().generation, 0);

        let applied = store
            .apply(0, write(ExternalSourceScope::UserGlobal, "check"))
            .await
            .unwrap();
        assert!(matches!(applied, HookImportApply::Applied));
        let snapshot = store.snapshot().await.unwrap();
        assert_eq!(snapshot.generation, 1);
        assert_eq!(snapshot.imports.len(), 1);
        assert_eq!(store.enabled_layers().await.unwrap().len(), 1);

        let unchanged = store
            .apply(1, write(ExternalSourceScope::UserGlobal, "check"))
            .await
            .unwrap();
        assert!(matches!(unchanged, HookImportApply::Unchanged));
        assert_eq!(store.snapshot().await.unwrap().generation, 1);

        let reopened = HookImportStore::open(root, ExternalSourceScope::UserGlobal)
            .await
            .unwrap();
        assert_eq!(reopened.snapshot().await.unwrap().generation, 1);
        assert_eq!(reopened.enabled_layers().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn corrupt_index_fails_closed_until_explicit_reset() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("hook-imports");
        tokio::fs::create_dir_all(&root).await.unwrap();
        tokio::fs::write(root.join("index.json"), b"{not-json")
            .await
            .unwrap();
        let store = HookImportStore::open(root.clone(), ExternalSourceScope::UserGlobal)
            .await
            .unwrap();
        let corrupt = store.snapshot().await.unwrap();
        assert!(corrupt.corrupt_marker.is_some());
        assert!(store.enabled_layers().await.unwrap().is_empty());
        assert!(matches!(
            store
                .apply(0, write(ExternalSourceScope::UserGlobal, "check"))
                .await,
            Err(HookImportStoreError::Corrupt)
        ));

        let reset = store.reset_corrupt().await.unwrap();
        assert_ne!(reset.generation, 0);
        assert!(reset.corrupt_marker.is_none());
        assert!(tokio::fs::metadata(root.join("index.json")).await.is_ok());
        assert!(matches!(
            store
                .apply(0, write(ExternalSourceScope::UserGlobal, "check"))
                .await,
            Err(HookImportStoreError::StaleGeneration)
        ));
    }

    #[tokio::test]
    async fn stale_generation_and_invalid_scope_never_publish() {
        let temp = tempdir().unwrap();
        let store = HookImportStore::open(
            temp.path().join("hook-imports"),
            ExternalSourceScope::UserGlobal,
        )
        .await
        .unwrap();
        assert!(matches!(
            store
                .apply(1, write(ExternalSourceScope::UserGlobal, "check"))
                .await,
            Err(HookImportStoreError::StaleGeneration)
        ));
        assert!(matches!(
            store
                .apply(0, write(ExternalSourceScope::Project, "check"))
                .await,
            Err(HookImportStoreError::InvalidInput(_))
        ));
        assert_eq!(store.snapshot().await.unwrap().generation, 0);
    }

    #[tokio::test]
    async fn missing_active_bundle_is_repaired_instead_of_reported_unchanged() {
        let temp = tempdir().unwrap();
        let store = HookImportStore::open(
            temp.path().join("hook-imports"),
            ExternalSourceScope::UserGlobal,
        )
        .await
        .unwrap();
        store
            .apply(0, write(ExternalSourceScope::UserGlobal, "check"))
            .await
            .unwrap();
        let snapshot = store.snapshot().await.unwrap();
        tokio::fs::remove_dir_all(&snapshot.imports[0].bundle_path)
            .await
            .unwrap();

        let repaired = store
            .apply(1, write(ExternalSourceScope::UserGlobal, "check"))
            .await
            .unwrap();
        assert!(matches!(repaired, HookImportApply::Applied));
        assert_eq!(store.enabled_layers().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn valid_but_modified_bundle_is_rejected_and_repaired() {
        let temp = tempdir().unwrap();
        let store = HookImportStore::open(
            temp.path().join("hook-imports"),
            ExternalSourceScope::UserGlobal,
        )
        .await
        .unwrap();
        store
            .apply(0, write(ExternalSourceScope::UserGlobal, "check"))
            .await
            .unwrap();
        let snapshot = store.snapshot().await.unwrap();
        let bundle = &snapshot.imports[0].bundle_path;
        tokio::fs::write(
            bundle.join("hooks.json"),
            br#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"unreviewed"}]}]}}"#,
        )
        .await
        .unwrap();

        let reopened = HookImportStore::open(
            temp.path().join("hook-imports"),
            ExternalSourceScope::UserGlobal,
        )
        .await
        .unwrap();
        assert!(reopened.enabled_layers().await.unwrap().is_empty());

        let repaired = reopened
            .apply(1, write(ExternalSourceScope::UserGlobal, "check"))
            .await
            .unwrap();
        assert!(matches!(repaired, HookImportApply::Applied));
        assert_eq!(reopened.enabled_layers().await.unwrap().len(), 1);

        tokio::fs::write(bundle.join("hooks/check.py"), b"print('unreviewed')")
            .await
            .unwrap();
        let reopened_after_asset_change = HookImportStore::open(
            temp.path().join("hook-imports"),
            ExternalSourceScope::UserGlobal,
        )
        .await
        .unwrap();
        assert!(reopened_after_asset_change
            .enabled_layers()
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn same_process_bundle_change_is_rejected_before_runtime_load() {
        let temp = tempdir().unwrap();
        let store = HookImportStore::open(
            temp.path().join("hook-imports"),
            ExternalSourceScope::UserGlobal,
        )
        .await
        .unwrap();
        store
            .apply(0, write(ExternalSourceScope::UserGlobal, "check"))
            .await
            .unwrap();
        let snapshot = store.snapshot().await.unwrap();
        tokio::fs::write(
            snapshot.imports[0].bundle_path.join("hooks.json"),
            br#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"unreviewed"}]}]}}"#,
        )
        .await
        .unwrap();

        assert!(store.enabled_layers().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn failed_same_path_repair_preserves_the_indexed_bundle() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("hook-imports");
        let store = HookImportStore::open(root.clone(), ExternalSourceScope::UserGlobal)
            .await
            .unwrap();
        let original = write(ExternalSourceScope::UserGlobal, "check");
        store.apply(0, original.clone()).await.unwrap();
        let snapshot = store.snapshot().await.unwrap();
        let bundle_path = snapshot.imports[0].bundle_path.clone();
        tokio::fs::remove_dir(root.join(".staging")).await.unwrap();
        tokio::fs::write(root.join(".staging"), b"not-a-directory")
            .await
            .unwrap();
        let mut changed = original;
        changed.hooks_json =
            br#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"changed"}]}]}}"#
                .to_vec();

        assert!(store.apply(1, changed).await.is_err());
        assert!(tokio::fs::metadata(bundle_path.join("hooks.json"))
            .await
            .is_ok());
        assert_eq!(store.enabled_layers().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn same_path_publication_can_rollback_before_index_commit() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("hook-imports");
        let store = HookImportStore::open(root.clone(), ExternalSourceScope::UserGlobal)
            .await
            .unwrap();
        let original = write(ExternalSourceScope::UserGlobal, "check");
        store.apply(0, original.clone()).await.unwrap();
        let bundle_path = store.snapshot().await.unwrap().imports[0]
            .bundle_path
            .clone();
        let original_bytes = tokio::fs::read(bundle_path.join("hooks.json"))
            .await
            .unwrap();
        let mut changed = original;
        changed.hooks_json =
            br#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"changed"}]}]}}"#
                .to_vec();
        let changed_digest = bundle_content_digest(&changed);

        let publication = publish_bundle(&root, &bundle_path, &changed, &changed_digest)
            .await
            .unwrap();
        assert_ne!(
            tokio::fs::read(bundle_path.join("hooks.json"))
                .await
                .unwrap(),
            original_bytes
        );
        publication.rollback().await.unwrap();

        assert_eq!(
            tokio::fs::read(bundle_path.join("hooks.json"))
                .await
                .unwrap(),
            original_bytes
        );
    }

    #[tokio::test]
    async fn successful_same_path_publication_removes_retired_bundle() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("hook-imports");
        let store = HookImportStore::open(root.clone(), ExternalSourceScope::UserGlobal)
            .await
            .unwrap();
        let mut changed = write(ExternalSourceScope::UserGlobal, "check");
        store.apply(0, changed.clone()).await.unwrap();
        changed.hooks_json =
            br#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"changed"}]}]}}"#
                .to_vec();

        store.apply(1, changed).await.unwrap();

        let mut staging = tokio::fs::read_dir(root.join(".staging")).await.unwrap();
        assert!(staging.next_entry().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn remove_deletes_only_the_removed_digest_directory() {
        let temp = tempdir().unwrap();
        let store = HookImportStore::open(
            temp.path().join("hook-imports"),
            ExternalSourceScope::UserGlobal,
        )
        .await
        .unwrap();
        store
            .apply(0, write(ExternalSourceScope::UserGlobal, "check"))
            .await
            .unwrap();
        let snapshot = store.snapshot().await.unwrap();
        let import_root = snapshot.imports[0].bundle_path.parent().unwrap();
        let concurrent_bundle = import_root.join("concurrent-reimport");
        tokio::fs::create_dir_all(&concurrent_bundle).await.unwrap();
        let import_id = snapshot.imports[0].import_id.clone();

        store.remove(1, &import_id).await.unwrap();

        assert!(tokio::fs::metadata(&concurrent_bundle).await.is_ok());
    }

    #[cfg(any(unix, windows))]
    #[tokio::test]
    async fn managed_bundle_ancestors_cannot_be_links_or_reparse_points() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("hook-imports");
        let outside = temp.path().join("outside");
        tokio::fs::create_dir_all(&root).await.unwrap();
        tokio::fs::create_dir_all(&outside).await.unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, root.join("bundles")).unwrap();
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;

            assert!(std::process::Command::new("cmd")
                .args(["/c", "mklink", "/J"])
                .arg(root.join("bundles"))
                .arg(&outside)
                .creation_flags(0x0800_0000)
                .output()
                .expect("create junction")
                .status
                .success());
        }
        let store = HookImportStore::open(root, ExternalSourceScope::UserGlobal)
            .await
            .unwrap();

        assert!(store
            .apply(0, write(ExternalSourceScope::UserGlobal, "check"))
            .await
            .is_err());
        assert!(tokio::fs::read_dir(&outside)
            .await
            .unwrap()
            .next_entry()
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn missing_bundle_cannot_be_reenabled_without_repair() {
        let temp = tempdir().unwrap();
        let store = HookImportStore::open(
            temp.path().join("hook-imports"),
            ExternalSourceScope::UserGlobal,
        )
        .await
        .unwrap();
        store
            .apply(0, write(ExternalSourceScope::UserGlobal, "check"))
            .await
            .unwrap();
        let import_id =
            HookImportStore::stable_import_id(&source(ExternalSourceScope::UserGlobal).key);
        let disabled = store.set_enabled(1, &import_id, false).await.unwrap();
        tokio::fs::remove_dir_all(&disabled.imports[0].bundle_path)
            .await
            .unwrap();

        assert!(matches!(
            store.set_enabled(2, &import_id, true).await,
            Err(HookImportStoreError::InvalidInput(
                "bundle missing or invalid"
            ))
        ));
        let unchanged = store.snapshot().await.unwrap();
        assert_eq!(unchanged.generation, 2);
        assert!(!unchanged.imports[0].enabled);
    }

    #[tokio::test]
    async fn management_ids_cannot_escape_the_managed_bundle_root() {
        let temp = tempdir().unwrap();
        let store = HookImportStore::open(
            temp.path().join("hook-imports"),
            ExternalSourceScope::UserGlobal,
        )
        .await
        .unwrap();
        assert!(matches!(
            store.remove(0, "../outside").await,
            Err(HookImportStoreError::InvalidInput("import id"))
        ));
    }
}
