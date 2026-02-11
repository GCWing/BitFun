use crate::service::snapshot::types::{
    FileMetadata, FileSnapshot, OptimizedContent, SnapshotError, SnapshotResult, SnapshotType,
    StorageStats,
};
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Baseline snapshot cache
pub struct BaselineCache {
    /// In-memory cache: file_path -> snapshot_id
    /// `None` indicates it has been queried but does not exist.
    cache: Arc<RwLock<HashMap<PathBuf, Option<String>>>>,

    /// Baseline metadata directory
    baseline_dir: PathBuf,
}

impl BaselineCache {
    /// Creates a new baseline cache.
    pub fn new(bitfun_dir: &Path) -> Self {
        let baseline_dir = bitfun_dir.join("snapshots").join("baselines");

        if let Err(e) = std::fs::create_dir_all(&baseline_dir) {
            warn!(
                "Failed to create baseline directory: path={} error={}",
                baseline_dir.display(),
                e
            );
        }

        debug!(
            "BaselineCache initialized: directory={}",
            baseline_dir.display()
        );

        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            baseline_dir,
        }
    }

    /// Gets the baseline snapshot ID for a file.
    ///
    /// Strategy: check the in-memory map first; if missing, check the directory, then cache the result.
    pub async fn get(&self, file_path: &Path) -> Option<String> {
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(file_path) {
                debug!("Baseline cache hit: file_path={:?}", file_path);
                return cached.clone();
            }
        }

        debug!("Querying baseline directory: file_path={:?}", file_path);
        if let Some(snapshot_id) = self.query_directory(file_path).await {
            debug!(
                "Found baseline snapshot: file_path={:?} snapshot_id={}",
                file_path, snapshot_id
            );

            {
                let mut cache = self.cache.write().await;
                cache.insert(file_path.to_path_buf(), Some(snapshot_id.clone()));
            }

            return Some(snapshot_id);
        }

        debug!("Baseline snapshot not found: file_path={:?}", file_path);
        {
            let mut cache = self.cache.write().await;
            cache.insert(file_path.to_path_buf(), None);
        }
        None
    }

    /// Queries baseline snapshots from the directory.
    async fn query_directory(&self, file_path: &Path) -> Option<String> {
        let entries = fs::read_dir(&self.baseline_dir).ok()?;

        let mut found_snapshots: Vec<(SystemTime, String)> = Vec::new();

        for entry in entries.flatten() {
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            let content = fs::read_to_string(&path).ok()?;
            let metadata: FileSnapshot = serde_json::from_str(&content).ok()?;

            if metadata.snapshot_type == SnapshotType::Baseline && metadata.file_path == file_path {
                found_snapshots.push((metadata.timestamp, metadata.snapshot_id));
            }
        }

        if found_snapshots.is_empty() {
            return None;
        }

        found_snapshots.sort_by(|a, b| b.0.cmp(&a.0));
        let (timestamp, snapshot_id) = &found_snapshots[0];

        debug!(
            "Found {} baseline snapshots, using latest: snapshot_id={} timestamp={:?}",
            found_snapshots.len(),
            snapshot_id,
            timestamp
        );

        Some(snapshot_id.clone())
    }

    /// Creates a baseline from a "before" snapshot.
    pub async fn create_from_snapshot(
        &self,
        file_path: &Path,
        before_snapshot_id: &str,
        active_snapshots: &HashMap<String, FileSnapshot>,
    ) -> SnapshotResult<String> {
        debug!(
            "Creating baseline snapshot: file_path={:?} before_snapshot_id={}",
            file_path, before_snapshot_id
        );

        let before_snapshot = active_snapshots
            .get(before_snapshot_id)
            .ok_or_else(|| SnapshotError::SnapshotNotFound(before_snapshot_id.to_string()))?;

        let baseline_id = format!("baseline_{}", Uuid::new_v4());

        let baseline_metadata = FileSnapshot {
            snapshot_id: baseline_id.clone(),
            file_path: file_path.to_path_buf(),
            content_hash: before_snapshot.content_hash.clone(),
            snapshot_type: SnapshotType::Baseline,
            compressed_content: before_snapshot.compressed_content.clone(),
            timestamp: SystemTime::now(),
            metadata: before_snapshot.metadata.clone(),
        };

        let baseline_meta_path = self.baseline_dir.join(format!("{}.json", baseline_id));
        let metadata_json = serde_json::to_string_pretty(&baseline_metadata)?;
        fs::write(&baseline_meta_path, metadata_json)?;

        debug!(
            "Created baseline snapshot: file_path={:?} baseline_id={} metadata_path={}",
            file_path,
            baseline_id,
            baseline_meta_path.display()
        );

        {
            let mut cache = self.cache.write().await;
            cache.insert(file_path.to_path_buf(), Some(baseline_id.clone()));
        }

        Ok(baseline_id)
    }
}

/// Simplified file snapshot system
///
/// Only stores snapshots of file content; does not manage a change queue.
pub struct FileSnapshotSystem {
    snapshot_dir: PathBuf,
    hash_to_path: HashMap<String, PathBuf>,
    active_snapshots: HashMap<String, FileSnapshot>,
    compression_enabled: bool,
    dedup_enabled: bool,
    baseline_cache: BaselineCache,
}

impl FileSnapshotSystem {
    /// Creates a new file snapshot system.
    pub fn new(bitfun_dir: &Path) -> Self {
        let snapshot_dir = bitfun_dir.join("snapshots");

        Self {
            snapshot_dir,
            hash_to_path: HashMap::new(),
            active_snapshots: HashMap::new(),
            compression_enabled: true,
            dedup_enabled: true,
            baseline_cache: BaselineCache::new(bitfun_dir),
        }
    }

    /// Initializes the snapshot system.
    pub async fn initialize(&mut self) -> SnapshotResult<()> {
        info!("Initializing file snapshot system");

        self.ensure_directories().await?;

        self.load_snapshot_index().await?;

        info!(
            "File snapshot system initialized: loaded_snapshots={}",
            self.active_snapshots.len()
        );
        Ok(())
    }

    /// Ensures required directories exist.
    async fn ensure_directories(&self) -> SnapshotResult<()> {
        let directories = [
            &self.snapshot_dir,
            &self.snapshot_dir.join("by_hash"),
            &self.snapshot_dir.join("metadata"),
        ];

        for dir in &directories {
            if !dir.exists() {
                fs::create_dir_all(dir)?;
                debug!("Created snapshot directory: path={}", dir.display());
            }
        }

        Ok(())
    }

    /// Loads the existing snapshot index.
    async fn load_snapshot_index(&mut self) -> SnapshotResult<()> {
        let metadata_dir = self.snapshot_dir.join("metadata");

        if !metadata_dir.exists() {
            return Ok(());
        }

        let mut loaded_count = 0;

        for entry in fs::read_dir(&metadata_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                match self.load_snapshot_metadata(&path).await {
                    Ok(snapshot) => {
                        self.hash_to_path.insert(
                            snapshot.content_hash.clone(),
                            self.get_content_path(&snapshot.content_hash),
                        );
                        self.active_snapshots
                            .insert(snapshot.snapshot_id.clone(), snapshot);
                        loaded_count += 1;
                    }
                    Err(e) => {
                        warn!(
                            "Failed to load snapshot metadata: path={} error={}",
                            path.display(),
                            e
                        );
                    }
                }
            }
        }

        debug!("Loaded {} snapshots", loaded_count);
        Ok(())
    }

    /// Loads snapshot metadata.
    async fn load_snapshot_metadata(&self, path: &Path) -> SnapshotResult<FileSnapshot> {
        let content = fs::read_to_string(path)?;
        let snapshot: FileSnapshot = serde_json::from_str(&content)?;
        Ok(snapshot)
    }

    /// Creates a file snapshot.
    pub async fn create_snapshot(&mut self, file_path: &Path) -> SnapshotResult<String> {
        debug!("Creating snapshot: file_path={}", file_path.display());

        if !file_path.exists() {
            error!(
                "File not found for snapshot: file_path={}",
                file_path.display()
            );
            return Err(SnapshotError::FileNotFound(file_path.to_path_buf()));
        }

        let content = match fs::read(file_path) {
            Ok(data) => data,
            Err(e) => {
                error!(
                    "Failed to read file for snapshot: file_path={} error={}",
                    file_path.display(),
                    e
                );
                return Err(SnapshotError::Io(e));
            }
        };

        let metadata = self.extract_file_metadata(file_path).await?;

        let content_hash = self.calculate_content_hash(&content);

        if self.dedup_enabled && self.hash_to_path.contains_key(&content_hash) {
            debug!(
                "Found duplicate content, reusing existing snapshot: content_hash={}",
                content_hash
            );
            return Ok(self.find_snapshot_by_hash(&content_hash)?);
        }

        let optimized_content = self.optimize_content(&content);

        let snapshot = FileSnapshot {
            snapshot_id: Uuid::new_v4().to_string(),
            file_path: file_path.to_path_buf(),
            content_hash: content_hash.clone(),
            snapshot_type: SnapshotType::Before,
            compressed_content: match optimized_content {
                OptimizedContent::Raw(data) => data,
                OptimizedContent::Compressed(data) => data,
                OptimizedContent::Reference(_) => unreachable!(),
            },
            timestamp: SystemTime::now(),
            metadata,
        };

        self.store_snapshot(&snapshot).await?;

        self.hash_to_path
            .insert(content_hash, self.get_content_path(&snapshot.content_hash));
        let snapshot_id = snapshot.snapshot_id.clone();
        self.active_snapshots.insert(snapshot_id.clone(), snapshot);

        debug!("Snapshot created successfully: snapshot_id={}", snapshot_id);
        Ok(snapshot_id)
    }

    /// Extracts file metadata.
    async fn extract_file_metadata(&self, file_path: &Path) -> SnapshotResult<FileMetadata> {
        let metadata = fs::metadata(file_path)?;

        let permissions = {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                Some(metadata.permissions().mode())
            }
            #[cfg(not(unix))]
            {
                None
            }
        };

        let encoding = self
            .detect_file_encoding(file_path)
            .await
            .unwrap_or_else(|| "utf-8".to_string());

        Ok(FileMetadata {
            size: metadata.len(),
            permissions,
            last_modified: metadata.modified()?,
            encoding,
        })
    }

    /// Detects file encoding.
    async fn detect_file_encoding(&self, file_path: &Path) -> Option<String> {
        match fs::read(file_path) {
            Ok(bytes) => {
                if bytes.is_ascii() {
                    Some("ascii".to_string())
                } else if String::from_utf8(bytes).is_ok() {
                    Some("utf-8".to_string())
                } else {
                    Some("binary".to_string())
                }
            }
            Err(_) => None,
        }
    }

    /// Computes content hash.
    fn calculate_content_hash(&self, content: &[u8]) -> String {
        format!("{:x}", md5::compute(content))
    }

    /// Optimizes content storage.
    fn optimize_content(&self, content: &[u8]) -> OptimizedContent {
        if self.dedup_enabled {
            let hash = self.calculate_content_hash(content);
            if self.hash_to_path.contains_key(&hash) {
                return OptimizedContent::Reference(hash);
            }
        }

        if self.compression_enabled && content.len() > 1024 {
            match self.compress_content(content) {
                Ok(compressed) => {
                    if compressed.len() < content.len() * 4 / 5 {
                        return OptimizedContent::Compressed(compressed);
                    }
                }
                Err(e) => {
                    warn!("Content compression failed: error={}", e);
                }
            }
        }

        OptimizedContent::Raw(content.to_vec())
    }

    /// Compresses content.
    fn compress_content(&self, content: &[u8]) -> Result<Vec<u8>, std::io::Error> {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(content)?;
        encoder.finish()
    }

    /// Decompresses content.
    fn decompress_content(&self, compressed: &[u8]) -> Result<Vec<u8>, std::io::Error> {
        use flate2::read::GzDecoder;
        use std::io::Read;

        let mut decoder = GzDecoder::new(compressed);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;
        Ok(decompressed)
    }

    /// Stores a snapshot.
    async fn store_snapshot(&self, snapshot: &FileSnapshot) -> SnapshotResult<()> {
        let content_path = self.get_content_path(&snapshot.content_hash);
        if !content_path.exists() {
            fs::write(&content_path, &snapshot.compressed_content)?;
        }

        let metadata_path = self.get_metadata_path(&snapshot.snapshot_id);
        let metadata_json = serde_json::to_string_pretty(snapshot)?;
        fs::write(&metadata_path, metadata_json)?;

        debug!(
            "Snapshot stored: snapshot_id={} content_path={}",
            snapshot.snapshot_id,
            content_path.display()
        );
        Ok(())
    }

    /// Returns the content file path.
    fn get_content_path(&self, content_hash: &str) -> PathBuf {
        self.snapshot_dir
            .join("by_hash")
            .join(format!("{}.snap", content_hash))
    }

    /// Returns the metadata file path.
    fn get_metadata_path(&self, snapshot_id: &str) -> PathBuf {
        if snapshot_id.starts_with("baseline_") {
            self.snapshot_dir
                .join("baselines")
                .join(format!("{}.json", snapshot_id))
        } else {
            self.snapshot_dir
                .join("metadata")
                .join(format!("{}.json", snapshot_id))
        }
    }

    /// Finds a snapshot ID by hash.
    fn find_snapshot_by_hash(&self, content_hash: &str) -> SnapshotResult<String> {
        for (snapshot_id, snapshot) in &self.active_snapshots {
            if snapshot.content_hash == content_hash {
                return Ok(snapshot_id.clone());
            }
        }

        Err(SnapshotError::SnapshotNotFound(format!(
            "hash: {}",
            content_hash
        )))
    }

    /// Loads snapshot metadata from disk (without using in-memory cache).
    async fn load_snapshot_from_disk(&self, snapshot_id: &str) -> SnapshotResult<FileSnapshot> {
        debug!(
            "Loading snapshot metadata from disk: snapshot_id={}",
            snapshot_id
        );
        let metadata_path = self.get_metadata_path(snapshot_id);

        if !metadata_path.exists() {
            return Err(SnapshotError::SnapshotNotFound(snapshot_id.to_string()));
        }

        let snapshot = self.load_snapshot_metadata(&metadata_path).await?;
        debug!(
            "Snapshot metadata loaded successfully: snapshot_id={}",
            snapshot_id
        );
        Ok(snapshot)
    }

    /// Gets snapshot content (string), read directly from disk.
    pub async fn get_snapshot_content(&self, snapshot_id: &str) -> SnapshotResult<String> {
        let content_bytes = self.restore_snapshot_content(snapshot_id).await?;
        String::from_utf8(content_bytes).map_err(|e| {
            SnapshotError::ConfigError(format!("Snapshot content is not valid UTF-8: {}", e))
        })
    }

    /// Restores snapshot content (read directly from disk, without using in-memory cache).
    pub async fn restore_snapshot_content(&self, snapshot_id: &str) -> SnapshotResult<Vec<u8>> {
        let snapshot = self.load_snapshot_from_disk(snapshot_id).await?;

        if !snapshot.compressed_content.is_empty() {
            return self.extract_content_from_snapshot(&snapshot);
        }

        let content_path = self.get_content_path(&snapshot.content_hash);
        if !content_path.exists() {
            return Err(SnapshotError::SnapshotNotFound(format!(
                "content file not found: {}",
                content_path.display()
            )));
        }

        let compressed_content = fs::read(&content_path)?;

        match self.decompress_content(&compressed_content) {
            Ok(decompressed) => Ok(decompressed),
            Err(_) => Ok(compressed_content),
        }
    }

    /// Extracts content from a snapshot.
    fn extract_content_from_snapshot(&self, snapshot: &FileSnapshot) -> SnapshotResult<Vec<u8>> {
        if snapshot.compressed_content.is_empty() {
            return Err(SnapshotError::SnapshotNotFound(
                "snapshot content is empty".to_string(),
            ));
        }

        match self.decompress_content(&snapshot.compressed_content) {
            Ok(decompressed) => Ok(decompressed),
            Err(_) => Ok(snapshot.compressed_content.clone()),
        }
    }

    /// Restores a file to the specified path (reads snapshot directly from disk).
    pub async fn restore_file(&self, snapshot_id: &str, target_path: &Path) -> SnapshotResult<()> {
        info!(
            "Restoring file from snapshot: snapshot_id={} target_path={}",
            snapshot_id,
            target_path.display()
        );

        let snapshot = self.load_snapshot_from_disk(snapshot_id).await?;
        let metadata = snapshot.metadata.clone();

        let content = self.restore_snapshot_content(snapshot_id).await?;

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(target_path, content)?;

        self.restore_file_metadata(target_path, &metadata).await?;

        info!(
            "File restored successfully: target_path={}",
            target_path.display()
        );
        Ok(())
    }

    /// Restores file metadata.
    async fn restore_file_metadata(
        &self,
        file_path: &Path,
        metadata: &FileMetadata,
    ) -> SnapshotResult<()> {
        #[cfg(unix)]
        {
            if let Some(permissions) = metadata.permissions {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(permissions);
                fs::set_permissions(file_path, perms)?;
            }
        }

        let filetime = filetime::FileTime::from_system_time(metadata.last_modified);
        if let Err(e) = filetime::set_file_mtime(file_path, filetime) {
            warn!("Failed to restore file modification time: error={}", e);
        }

        Ok(())
    }

    /// Deletes a snapshot.
    pub async fn delete_snapshot(&mut self, snapshot_id: &str) -> SnapshotResult<()> {
        info!("Deleting snapshot: snapshot_id={}", snapshot_id);

        let snapshot = self
            .active_snapshots
            .remove(snapshot_id)
            .ok_or_else(|| SnapshotError::SnapshotNotFound(snapshot_id.to_string()))?;

        let content_still_used = self
            .active_snapshots
            .values()
            .any(|s| s.content_hash == snapshot.content_hash);

        if !content_still_used {
            let content_path = self.get_content_path(&snapshot.content_hash);
            if content_path.exists() {
                fs::remove_file(&content_path)?;
            }

            self.hash_to_path.remove(&snapshot.content_hash);
        }

        let metadata_path = self.get_metadata_path(snapshot_id);
        if metadata_path.exists() {
            fs::remove_file(&metadata_path)?;
        }

        debug!("Snapshot deleted successfully: snapshot_id={}", snapshot_id);
        Ok(())
    }

    /// Returns storage statistics.
    pub async fn get_storage_stats(&self) -> SnapshotResult<StorageStats> {
        let mut total_size_bytes = 0;
        let mut compressed_size_bytes = 0;

        let total_snapshots = self.active_snapshots.len();

        let content_dir = self.snapshot_dir.join("by_hash");
        if content_dir.exists() {
            for entry in fs::read_dir(&content_dir)? {
                let entry = entry?;
                if let Ok(metadata) = entry.metadata() {
                    compressed_size_bytes += metadata.len();
                }
            }
        }

        for snapshot in self.active_snapshots.values() {
            total_size_bytes += snapshot.metadata.size;
        }

        let compression_ratio = if total_size_bytes > 0 {
            compressed_size_bytes as f32 / total_size_bytes as f32
        } else {
            1.0
        };

        let dedup_savings_bytes = if self.dedup_enabled {
            let unique_hashes = self.hash_to_path.len() as u64;
            let total_hashes = total_snapshots as u64;
            if total_hashes > unique_hashes {
                (total_size_bytes * (total_hashes - unique_hashes)) / total_hashes
            } else {
                0
            }
        } else {
            0
        };

        Ok(StorageStats {
            total_snapshots,
            total_size_bytes,
            compressed_size_bytes,
            compression_ratio,
            dedup_savings_bytes,
        })
    }

    /// Cleans up orphaned snapshots.
    pub async fn cleanup_orphaned_snapshots(&mut self) -> SnapshotResult<usize> {
        info!("Cleaning up orphaned snapshots");

        let mut cleaned_count = 0;
        let content_dir = self.snapshot_dir.join("by_hash");

        if !content_dir.exists() {
            return Ok(0);
        }

        let mut content_files = Vec::new();
        for entry in fs::read_dir(&content_dir)? {
            let entry = entry?;
            if entry.path().extension().and_then(|s| s.to_str()) == Some("snap") {
                content_files.push(entry.path());
            }
        }

        for content_file in content_files {
            if let Some(file_stem) = content_file.file_stem().and_then(|s| s.to_str()) {
                let content_hash = file_stem;

                let is_referenced = self
                    .active_snapshots
                    .values()
                    .any(|snapshot| snapshot.content_hash == content_hash);

                if !is_referenced {
                    fs::remove_file(&content_file)?;
                    self.hash_to_path.remove(content_hash);
                    cleaned_count += 1;
                    debug!(
                        "Deleted orphaned content file: path={}",
                        content_file.display()
                    );
                }
            }
        }

        info!("Cleaned up {} orphaned snapshots", cleaned_count);
        Ok(cleaned_count)
    }

    /// Lists all snapshots.
    pub fn list_snapshots(&self) -> Vec<&FileSnapshot> {
        self.active_snapshots.values().collect()
    }

    /// Returns the snapshot count.
    pub fn snapshot_count(&self) -> usize {
        self.active_snapshots.len()
    }

    /// Gets the baseline snapshot ID for a file.
    ///
    /// Returns: Option<String> - `None` means this file has no baseline
    pub async fn get_baseline_snapshot_id(&self, file_path: &Path) -> Option<String> {
        self.baseline_cache.get(file_path).await
    }

    /// Creates a baseline snapshot.
    ///
    /// Creates a baseline from the specified "before" snapshot.
    /// If a baseline already exists, it will not be created again.
    ///
    /// # Parameters
    /// - file_path: File path
    /// - before_snapshot_id: before snapshot ID
    ///
    /// # Returns
    /// Baseline snapshot ID
    pub async fn create_baseline_from_snapshot(
        &self,
        file_path: &Path,
        before_snapshot_id: &str,
    ) -> SnapshotResult<String> {
        debug!(
            "Creating baseline snapshot: file_path={:?} before_snapshot_id={}",
            file_path, before_snapshot_id
        );

        if let Some(existing_id) = self.get_baseline_snapshot_id(file_path).await {
            debug!(
                "Baseline snapshot already exists: baseline_id={}",
                existing_id
            );
            return Ok(existing_id);
        }

        self.baseline_cache
            .create_from_snapshot(file_path, before_snapshot_id, &self.active_snapshots)
            .await
    }

    /// Checks whether the file has a baseline.
    pub async fn has_baseline(&self, file_path: &Path) -> bool {
        self.get_baseline_snapshot_id(file_path).await.is_some()
    }
}
