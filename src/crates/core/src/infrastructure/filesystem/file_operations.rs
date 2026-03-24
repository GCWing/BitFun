//! File operation service
//!
//! Provides safe file read/write and operations

use crate::util::errors::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::fs;

/// Same rules as web `normalizeTextForDiskSyncComparison` (BOM strip, CRLF/CR → LF).
pub fn normalize_text_for_editor_disk_sync(text: &str) -> String {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

pub struct FileOperationService {
    max_file_size_mb: u64,
    allowed_extensions: Option<Vec<String>>,
    restricted_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct FileOperationOptions {
    pub max_file_size_mb: u64,
    pub allowed_extensions: Option<Vec<String>>,
    pub restricted_paths: Vec<PathBuf>,
    pub backup_on_overwrite: bool,
}

impl Default for FileOperationOptions {
    fn default() -> Self {
        Self {
            max_file_size_mb: 100,
            allowed_extensions: None,
            // Only restrict critical system directories; do not restrict the whole root or C drive
            restricted_paths: vec![
                PathBuf::from("C:\\Windows\\System32"),
                PathBuf::from("C:\\Windows\\SysWOW64"),
                PathBuf::from("/System"),
                PathBuf::from("/bin"),
                PathBuf::from("/sbin"),
                PathBuf::from("/usr/bin"),
                PathBuf::from("/usr/sbin"),
                PathBuf::from("/etc"),
                PathBuf::from("/boot"),
            ],
            backup_on_overwrite: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub is_directory: bool,
    pub is_readonly: bool,
    pub created_at: Option<String>,
    pub modified_at: Option<String>,
    pub accessed_at: Option<String>,
    pub extension: Option<String>,
    pub mime_type: Option<String>,
    pub permissions: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReadResult {
    pub content: String,
    pub encoding: String,
    pub size: u64,
    pub is_binary: bool,
    pub line_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWriteResult {
    pub bytes_written: u64,
    pub backup_created: bool,
    pub backup_path: Option<String>,
}

impl Default for FileOperationService {
    fn default() -> Self {
        Self::new(FileOperationOptions::default())
    }
}

impl FileOperationService {
    pub fn new(options: FileOperationOptions) -> Self {
        Self {
            max_file_size_mb: options.max_file_size_mb,
            allowed_extensions: options.allowed_extensions,
            restricted_paths: options.restricted_paths,
        }
    }

    pub async fn read_file(&self, file_path: &str) -> BitFunResult<FileReadResult> {
        let path = Path::new(file_path);

        self.validate_file_access(path, false).await?;

        if !path.exists() {
            return Err(BitFunError::service(format!(
                "File does not exist: {}",
                file_path
            )));
        }

        if path.is_dir() {
            return Err(BitFunError::service(format!(
                "Path is a directory: {}",
                file_path
            )));
        }

        let metadata = fs::metadata(path)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to read file metadata: {}", e)))?;

        let file_size = metadata.len();
        if file_size > self.max_file_size_mb * 1024 * 1024 {
            return Err(BitFunError::service(format!(
                "File too large: {}MB (max: {}MB)",
                file_size / (1024 * 1024),
                self.max_file_size_mb
            )));
        }

        match fs::read_to_string(path).await {
            Ok(content) => {
                let line_count = content.lines().count();
                Ok(FileReadResult {
                    content,
                    encoding: "UTF-8".to_string(),
                    size: file_size,
                    is_binary: false,
                    line_count: Some(line_count),
                })
            }
            Err(_) => {
                let bytes = fs::read(path)
                    .await
                    .map_err(|e| BitFunError::service(format!("Failed to read file: {}", e)))?;

                let is_binary = self.is_binary_content(&bytes);

                if is_binary {
                    use base64::Engine;
                    let engine = base64::engine::general_purpose::STANDARD;
                    Ok(FileReadResult {
                        content: engine.encode(&bytes),
                        encoding: "base64".to_string(),
                        size: file_size,
                        is_binary: true,
                        line_count: None,
                    })
                } else {
                    let content = String::from_utf8_lossy(&bytes).to_string();
                    Ok(FileReadResult {
                        content: content.clone(),
                        encoding: "UTF-8-lossy".to_string(),
                        size: file_size,
                        is_binary: false,
                        line_count: Some(content.lines().count()),
                    })
                }
            }
        }
    }

    /// SHA-256 (hex, lowercase) of `bytes` using the same normalization as the web editor sync check,
    /// or raw-byte hash when content is treated as binary (matches `read_file` heuristics).
    pub fn editor_sync_sha256_hex_from_raw_bytes(&self, bytes: &[u8]) -> String {
        if self.is_binary_content(bytes) {
            sha256_hex(bytes)
        } else {
            let content = String::from_utf8_lossy(bytes);
            let normalized = normalize_text_for_editor_disk_sync(content.as_ref());
            sha256_hex(normalized.as_bytes())
        }
    }

    /// Reads the file from disk and returns the editor-sync hash (see `editor_sync_sha256_hex_from_raw_bytes`).
    pub async fn editor_sync_content_sha256_hex(&self, file_path: &str) -> BitFunResult<String> {
        let path = Path::new(file_path);

        self.validate_file_access(path, false).await?;

        if !path.exists() {
            return Err(BitFunError::service(format!(
                "File does not exist: {}",
                file_path
            )));
        }

        if path.is_dir() {
            return Err(BitFunError::service(format!(
                "Path is a directory: {}",
                file_path
            )));
        }

        let metadata = fs::metadata(path)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to read file metadata: {}", e)))?;

        let file_size = metadata.len();
        if file_size > self.max_file_size_mb * 1024 * 1024 {
            return Err(BitFunError::service(format!(
                "File too large: {}MB (max: {}MB)",
                file_size / (1024 * 1024),
                self.max_file_size_mb
            )));
        }

        let bytes = fs::read(path)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to read file: {}", e)))?;

        Ok(self.editor_sync_sha256_hex_from_raw_bytes(&bytes))
    }

    pub async fn write_file(
        &self,
        file_path: &str,
        content: &str,
        options: FileOperationOptions,
    ) -> BitFunResult<FileWriteResult> {
        let path = Path::new(file_path);

        self.validate_file_access(path, true).await?;

        let mut backup_created = false;
        let mut backup_path = None;

        if options.backup_on_overwrite && path.exists() {
            let backup_file_path = self.create_backup(path).await?;
            backup_created = true;
            backup_path = Some(backup_file_path);
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                BitFunError::service(format!("Failed to create parent directory: {}", e))
            })?;
        }

        fs::write(path, content)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to write file: {}", e)))?;

        let bytes_written = content.len() as u64;

        Ok(FileWriteResult {
            bytes_written,
            backup_created,
            backup_path,
        })
    }

    pub async fn write_binary_file(
        &self,
        file_path: &str,
        data: &[u8],
        options: FileOperationOptions,
    ) -> BitFunResult<FileWriteResult> {
        let path = Path::new(file_path);

        self.validate_file_access(path, true).await?;

        let mut backup_created = false;
        let mut backup_path = None;

        if options.backup_on_overwrite && path.exists() {
            let backup_file_path = self.create_backup(path).await?;
            backup_created = true;
            backup_path = Some(backup_file_path);
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                BitFunError::service(format!("Failed to create parent directory: {}", e))
            })?;
        }

        fs::write(path, data)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to write binary file: {}", e)))?;

        let bytes_written = data.len() as u64;

        Ok(FileWriteResult {
            bytes_written,
            backup_created,
            backup_path,
        })
    }

    pub async fn copy_file(&self, from: &str, to: &str) -> BitFunResult<u64> {
        let from_path = Path::new(from);
        let to_path = Path::new(to);

        self.validate_file_access(from_path, false).await?;
        self.validate_file_access(to_path, true).await?;

        if !from_path.exists() {
            return Err(BitFunError::service(format!(
                "Source file does not exist: {}",
                from
            )));
        }

        if from_path.is_dir() {
            return Err(BitFunError::service(
                "Cannot copy directory as file".to_string(),
            ));
        }

        if let Some(parent) = to_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                BitFunError::service(format!("Failed to create target directory: {}", e))
            })?;
        }

        let bytes_copied = fs::copy(from_path, to_path)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to copy file: {}", e)))?;

        Ok(bytes_copied)
    }

    pub async fn move_file(&self, from: &str, to: &str) -> BitFunResult<()> {
        let from_path = Path::new(from);
        let to_path = Path::new(to);

        self.validate_file_access(from_path, true).await?;
        self.validate_file_access(to_path, true).await?;

        if !from_path.exists() {
            return Err(BitFunError::service(format!(
                "Source file does not exist: {}",
                from
            )));
        }

        if let Some(parent) = to_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                BitFunError::service(format!("Failed to create target directory: {}", e))
            })?;
        }

        fs::rename(from_path, to_path)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to move file: {}", e)))?;

        Ok(())
    }

    pub async fn delete_file(&self, file_path: &str) -> BitFunResult<()> {
        let path = Path::new(file_path);

        self.validate_file_access(path, true).await?;

        if !path.exists() {
            return Err(BitFunError::service(format!(
                "File does not exist: {}",
                file_path
            )));
        }

        if path.is_dir() {
            return Err(BitFunError::service(
                "Cannot delete directory as file".to_string(),
            ));
        }

        fs::remove_file(path)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to delete file: {}", e)))?;

        Ok(())
    }

    pub async fn get_file_info(&self, file_path: &str) -> BitFunResult<FileInfo> {
        let path = Path::new(file_path);

        self.validate_file_access(path, false).await?;

        if !path.exists() {
            return Err(BitFunError::service(format!(
                "File does not exist: {}",
                file_path
            )));
        }

        let metadata = fs::metadata(path)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to read file metadata: {}", e)))?;

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_string());

        let mime_type = if !metadata.is_dir() {
            self.detect_mime_type(path)
        } else {
            None
        };

        let created_at = metadata.created().ok().map(|t| {
            let datetime: chrono::DateTime<chrono::Utc> = t.into();
            datetime.format("%Y-%m-%d %H:%M:%S").to_string()
        });

        let modified_at = metadata.modified().ok().map(|t| {
            let datetime: chrono::DateTime<chrono::Utc> = t.into();
            datetime.format("%Y-%m-%d %H:%M:%S").to_string()
        });

        let accessed_at = metadata.accessed().ok().map(|t| {
            let datetime: chrono::DateTime<chrono::Utc> = t.into();
            datetime.format("%Y-%m-%d %H:%M:%S").to_string()
        });

        let permissions = self.get_permissions_string(path).await;

        Ok(FileInfo {
            path: file_path.to_string(),
            name: file_name,
            size: metadata.len(),
            is_directory: metadata.is_dir(),
            is_readonly: metadata.permissions().readonly(),
            created_at,
            modified_at,
            accessed_at,
            extension,
            mime_type,
            permissions,
        })
    }

    pub async fn create_directory(&self, dir_path: &str) -> BitFunResult<()> {
        let path = Path::new(dir_path);

        self.validate_file_access(path, true).await?;

        fs::create_dir_all(path)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to create directory: {}", e)))?;

        Ok(())
    }

    pub async fn delete_directory(&self, dir_path: &str, recursive: bool) -> BitFunResult<()> {
        let path = Path::new(dir_path);

        self.validate_file_access(path, true).await?;

        if !path.exists() {
            return Err(BitFunError::service(format!(
                "Directory does not exist: {}",
                dir_path
            )));
        }

        if !path.is_dir() {
            return Err(BitFunError::service("Path is not a directory".to_string()));
        }

        if recursive {
            fs::remove_dir_all(path).await.map_err(|e| {
                BitFunError::service(format!("Failed to delete directory recursively: {}", e))
            })?;
        } else {
            fs::remove_dir(path)
                .await
                .map_err(|e| BitFunError::service(format!("Failed to delete directory: {}", e)))?;
        }

        Ok(())
    }

    async fn validate_file_access(&self, path: &Path, is_write: bool) -> BitFunResult<()> {
        for restricted in &self.restricted_paths {
            if path.starts_with(restricted) {
                return Err(BitFunError::service(format!(
                    "Access denied: path is in restricted list: {:?}",
                    path
                )));
            }
        }

        if let Some(allowed_extensions) = &self.allowed_extensions {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if !allowed_extensions.contains(&ext.to_lowercase()) {
                    return Err(BitFunError::service(format!(
                        "File extension not allowed: {}",
                        ext
                    )));
                }
            }
        }

        if is_write {
            if let Some(parent) = path.parent() {
                if parent.exists() {
                    let metadata = fs::metadata(parent).await.map_err(|e| {
                        BitFunError::service(format!(
                            "Failed to check parent directory permissions: {}",
                            e
                        ))
                    })?;

                    if metadata.permissions().readonly() {
                        return Err(BitFunError::service(
                            "Parent directory is read-only".to_string(),
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    async fn create_backup(&self, path: &Path) -> BitFunResult<String> {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let file_name = path.file_name().ok_or_else(|| {
            BitFunError::service(format!(
                "Failed to create backup: path has no file name: {}",
                path.display()
            ))
        })?;
        let backup_name = format!("{}.backup_{}", file_name.to_string_lossy(), timestamp);

        let backup_path = if let Some(parent) = path.parent() {
            parent.join(backup_name)
        } else {
            PathBuf::from(backup_name)
        };

        fs::copy(path, &backup_path)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to create backup: {}", e)))?;

        Ok(backup_path.to_string_lossy().to_string())
    }

    fn is_binary_content(&self, data: &[u8]) -> bool {
        const SAMPLE_SIZE: usize = 512;
        let sample = if data.len() > SAMPLE_SIZE {
            &data[..SAMPLE_SIZE]
        } else {
            data
        };

        if sample.contains(&0) {
            return true;
        }

        let non_printable_count = sample
            .iter()
            .filter(|&&b| b < 32 && b != 9 && b != 10 && b != 13)
            .count();

        let non_printable_ratio = non_printable_count as f64 / sample.len() as f64;
        non_printable_ratio > 0.1
    }

    fn detect_mime_type(&self, path: &Path) -> Option<String> {
        if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
            match extension.to_lowercase().as_str() {
                "txt" | "md" | "rst" => Some("text/plain".to_string()),
                "html" | "htm" => Some("text/html".to_string()),
                "css" => Some("text/css".to_string()),
                "js" => Some("application/javascript".to_string()),
                "json" => Some("application/json".to_string()),
                "xml" => Some("application/xml".to_string()),
                "yaml" | "yml" => Some("application/yaml".to_string()),
                "rs" => Some("text/rust".to_string()),
                "py" => Some("text/python".to_string()),
                "java" => Some("text/java".to_string()),
                "cpp" | "cc" | "cxx" => Some("text/cpp".to_string()),
                "c" => Some("text/c".to_string()),
                "h" | "hpp" => Some("text/c-header".to_string()),
                "png" => Some("image/png".to_string()),
                "jpg" | "jpeg" => Some("image/jpeg".to_string()),
                "gif" => Some("image/gif".to_string()),
                "svg" => Some("image/svg+xml".to_string()),
                "pdf" => Some("application/pdf".to_string()),
                "zip" => Some("application/zip".to_string()),
                _ => None,
            }
        } else {
            None
        }
    }

    async fn get_permissions_string(&self, path: &Path) -> Option<String> {
        if let Ok(metadata) = fs::metadata(path).await {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = metadata.permissions();
                let mode = perms.mode();

                let user = format!(
                    "{}{}{}",
                    if mode & 0o400 != 0 { "r" } else { "-" },
                    if mode & 0o200 != 0 { "w" } else { "-" },
                    if mode & 0o100 != 0 { "x" } else { "-" }
                );
                let group = format!(
                    "{}{}{}",
                    if mode & 0o040 != 0 { "r" } else { "-" },
                    if mode & 0o020 != 0 { "w" } else { "-" },
                    if mode & 0o010 != 0 { "x" } else { "-" }
                );
                let other = format!(
                    "{}{}{}",
                    if mode & 0o004 != 0 { "r" } else { "-" },
                    if mode & 0o002 != 0 { "w" } else { "-" },
                    if mode & 0o001 != 0 { "x" } else { "-" }
                );

                Some(format!("{}{}{}", user, group, other))
            }

            #[cfg(windows)]
            {
                let readonly = metadata.permissions().readonly();
                Some(if readonly { "r--" } else { "rw-" }.to_string())
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod editor_sync_hash_tests {
    use super::*;

    #[test]
    fn normalize_matches_web_contract() {
        assert_eq!(normalize_text_for_editor_disk_sync("\u{FEFF}a\r\nb"), "a\nb");
        assert_eq!(normalize_text_for_editor_disk_sync("x\ry"), "x\ny");
    }

    #[test]
    fn hello_utf8_hash_matches_known_sha256() {
        let svc = FileOperationService::default();
        let h = svc.editor_sync_sha256_hex_from_raw_bytes(b"hello");
        assert_eq!(
            h,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }
}
