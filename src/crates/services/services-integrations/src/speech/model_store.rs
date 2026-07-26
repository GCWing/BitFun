use super::model_catalog::builtin_speech_model_manifests;
use super::types::{
    InstalledSpeechModelArtifactRecord, InstalledSpeechModelFileRecord, InstalledSpeechModelRecord,
    SpeechModelArtifact, SpeechModelInstallState, SpeechModelManifest, SpeechModelStatus,
};
use super::{BitFunError, BitFunResult, SpeechStoragePaths};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncReadExt;

const INSTALL_RECORD_FILE: &str = "bitfun-model-install.json";

#[derive(Deserialize)]
struct SpeechModelInstallEnvelope {
    install: InstalledSpeechModelRecord,
}

#[derive(Clone)]
pub(super) struct SpeechModelStore {
    paths: SpeechStoragePaths,
}

impl SpeechModelStore {
    pub(super) fn new(paths: SpeechStoragePaths) -> Self {
        Self { paths }
    }

    pub(super) fn paths(&self) -> &SpeechStoragePaths {
        &self.paths
    }

    pub(super) fn model_dir(&self, manifest: &SpeechModelManifest) -> PathBuf {
        self.paths
            .models_dir()
            .join(&manifest.id)
            .join(&manifest.version)
    }

    pub(super) fn artifact_download_path(
        &self,
        manifest: &SpeechModelManifest,
        artifact: &SpeechModelArtifact,
    ) -> PathBuf {
        self.paths
            .downloads_dir()
            .join(&manifest.id)
            .join(&manifest.version)
            .join(&artifact.file_name)
    }

    pub(super) fn artifact_partial_path(
        &self,
        manifest: &SpeechModelManifest,
        artifact: &SpeechModelArtifact,
    ) -> PathBuf {
        self.artifact_download_path(manifest, artifact)
            .with_file_name(format!("{}.partial", artifact.file_name))
    }

    pub(super) async fn list_statuses(&self) -> BitFunResult<Vec<SpeechModelStatus>> {
        let mut statuses = Vec::new();
        for manifest in builtin_speech_model_manifests() {
            statuses.push(self.status_for_manifest(&manifest).await?);
        }
        Ok(statuses)
    }

    pub(super) async fn status_for_manifest(
        &self,
        manifest: &SpeechModelManifest,
    ) -> BitFunResult<SpeechModelStatus> {
        let model_dir = self.model_dir(manifest);
        let installed_bytes = dir_size(&model_dir).await?;
        let installed = self.has_required_files(manifest).await;
        let state = if installed {
            SpeechModelInstallState::Installed
        } else if model_dir.exists() {
            SpeechModelInstallState::Corrupt
        } else {
            SpeechModelInstallState::NotInstalled
        };

        Ok(SpeechModelStatus {
            model_id: manifest.id.clone(),
            display_name: manifest.display_name.clone(),
            version: manifest.version.clone(),
            state,
            installed_path: installed.then_some(model_dir),
            installed_bytes,
            expected_bytes: manifest.expected_bytes(),
            progress: None,
            error: None,
            provider: manifest.provider.clone(),
            description: manifest.description.clone(),
            languages: manifest.languages.clone(),
        })
    }

    pub(super) async fn has_required_files(&self, manifest: &SpeechModelManifest) -> bool {
        let model_dir = self.model_dir(manifest);
        if !model_dir.is_dir() {
            return false;
        }

        manifest
            .required_files
            .iter()
            .all(|relative| model_dir.join(relative).is_file())
    }

    pub(super) async fn verify_model(
        &self,
        manifest: &SpeechModelManifest,
    ) -> BitFunResult<SpeechModelStatus> {
        let mut status = self.status_for_manifest(manifest).await?;
        if !self.has_required_files(manifest).await {
            status.state = SpeechModelInstallState::Corrupt;
            status.error = Some("Required model files are missing".to_string());
            return Ok(status);
        }

        if let Err(error) = self.verify_install_record(manifest).await {
            status.state = SpeechModelInstallState::Corrupt;
            status.error = Some(error);
        }
        Ok(status)
    }

    async fn verify_install_record(&self, manifest: &SpeechModelManifest) -> Result<(), String> {
        let model_dir = self.model_dir(manifest);
        let record_path = model_dir.join(INSTALL_RECORD_FILE);
        let payload = fs::read(&record_path)
            .await
            .map_err(|error| format!("Failed to read speech model install record: {error}"))?;
        let envelope: SpeechModelInstallEnvelope = serde_json::from_slice(&payload)
            .map_err(|error| format!("Failed to parse speech model install record: {error}"))?;
        let record = envelope.install;

        if record.id != manifest.id || record.version != manifest.version {
            return Err(
                "Speech model install record does not match the model manifest".to_string(),
            );
        }
        if record.artifacts.is_empty() {
            let Some(first_artifact) = manifest.artifacts.first() else {
                return Err("Speech model manifest does not contain artifacts".to_string());
            };
            if record.archive_sha256 != first_artifact.sha256 {
                return Err(
                    "Speech model install record does not match the model artifact".to_string(),
                );
            }
        } else {
            for artifact in &manifest.artifacts {
                let expected = record
                    .artifacts
                    .iter()
                    .find(|installed| installed.id == artifact.id)
                    .ok_or_else(|| {
                        format!("Missing install data for model artifact: {}", artifact.id)
                    })?;
                if expected.file_name != artifact.file_name
                    || expected.size_bytes != artifact.size_bytes
                    || expected.sha256 != artifact.sha256
                {
                    return Err(format!(
                        "Speech model install record does not match artifact: {}",
                        artifact.id
                    ));
                }
            }
        }
        if record.files.is_empty() {
            return Err(
                "Speech model install record does not contain file integrity data".to_string(),
            );
        }

        for relative in &manifest.required_files {
            let expected = record
                .files
                .iter()
                .find(|file| file.path == *relative)
                .ok_or_else(|| format!("Missing integrity data for required file: {relative}"))?;
            let path = model_dir.join(relative);
            let metadata = fs::metadata(&path).await.map_err(|error| {
                format!("Failed to inspect required model file {relative}: {error}")
            })?;
            if metadata.len() != expected.size_bytes {
                return Err(format!("Speech model file size mismatch: {relative}"));
            }
            let actual_hash = sha256_file(&path).await.map_err(|error| {
                format!("Failed to hash required model file {relative}: {error}")
            })?;
            if actual_hash != expected.sha256 {
                return Err(format!("Speech model file checksum mismatch: {relative}"));
            }
        }
        Ok(())
    }

    pub(super) async fn write_install_record(
        &self,
        manifest: &SpeechModelManifest,
        model_dir: &Path,
    ) -> BitFunResult<()> {
        let mut files = Vec::with_capacity(manifest.required_files.len());
        for relative in &manifest.required_files {
            let path = model_dir.join(relative);
            let metadata = fs::metadata(&path).await?;
            files.push(InstalledSpeechModelFileRecord {
                path: relative.clone(),
                size_bytes: metadata.len(),
                sha256: sha256_file(&path).await?,
            });
        }
        let artifacts = manifest
            .artifacts
            .iter()
            .map(|artifact| InstalledSpeechModelArtifactRecord {
                id: artifact.id.clone(),
                file_name: artifact.file_name.clone(),
                size_bytes: artifact.size_bytes,
                sha256: artifact.sha256.clone(),
            })
            .collect::<Vec<_>>();
        let first_artifact = manifest.artifacts.first();
        let record = InstalledSpeechModelRecord {
            id: manifest.id.clone(),
            version: manifest.version.clone(),
            installed_at_ms: Utc::now().timestamp_millis(),
            source_url: first_artifact
                .map(|artifact| artifact.source_url.clone())
                .unwrap_or_default(),
            archive_sha256: first_artifact
                .map(|artifact| artifact.sha256.clone())
                .unwrap_or_default(),
            artifacts,
            files,
        };
        let payload = serde_json::to_vec_pretty(&json!({
            "model": manifest,
            "install": record,
        }))?;
        fs::write(model_dir.join(INSTALL_RECORD_FILE), payload).await?;
        Ok(())
    }

    pub(super) async fn delete_model(
        &self,
        manifest: &SpeechModelManifest,
    ) -> BitFunResult<SpeechModelStatus> {
        let root = self.paths.models_dir().to_path_buf();
        let target = self.model_dir(manifest);
        if !target.exists() {
            return self.status_for_manifest(manifest).await;
        }

        let root = canonical_or_create(&root).await?;
        let resolved = target.canonicalize().map_err(|e| {
            BitFunError::service(format!("Failed to resolve speech model path: {e}"))
        })?;
        if !resolved.starts_with(&root) {
            return Err(BitFunError::validation(
                "Refusing to delete path outside managed speech models directory",
            ));
        }

        fs::remove_dir_all(&resolved).await?;
        self.cleanup_download(manifest).await?;
        self.status_for_manifest(manifest).await
    }

    pub(super) async fn cleanup_download(
        &self,
        manifest: &SpeechModelManifest,
    ) -> BitFunResult<()> {
        let dir = self
            .paths
            .downloads_dir()
            .join(&manifest.id)
            .join(&manifest.version);
        if dir.exists() {
            fs::remove_dir_all(dir).await?;
        }
        Ok(())
    }
}

async fn sha256_file(path: &Path) -> BitFunResult<String> {
    let mut file = fs::File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub(super) fn validate_relative_archive_path(path: &Path) -> BitFunResult<PathBuf> {
    let mut sanitized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => sanitized.push(part),
            Component::CurDir => {}
            _ => {
                return Err(BitFunError::validation(format!(
                    "Archive entry contains unsafe path: {}",
                    path.display()
                )));
            }
        }
    }
    if sanitized.as_os_str().is_empty() {
        return Err(BitFunError::validation("Archive entry path is empty"));
    }
    Ok(sanitized)
}

pub(super) async fn dir_size(path: &Path) -> BitFunResult<u64> {
    fn inner(
        path: &Path,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = BitFunResult<u64>> + Send + '_>> {
        Box::pin(async move {
            if !path.exists() {
                return Ok(0);
            }
            let metadata = fs::metadata(path).await?;
            if metadata.is_file() {
                return Ok(metadata.len());
            }

            let mut total = 0u64;
            let mut entries = fs::read_dir(path).await?;
            while let Some(entry) = entries.next_entry().await? {
                total += inner(&entry.path()).await?;
            }
            Ok(total)
        })
    }

    inner(path).await
}

async fn canonical_or_create(path: &Path) -> BitFunResult<PathBuf> {
    fs::create_dir_all(path).await?;
    path.canonicalize()
        .map_err(|e| BitFunError::service(format!("Failed to resolve directory: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::speech::model_catalog::sensevoice_small_int8_manifest;
    use uuid::Uuid;

    #[tokio::test]
    async fn verify_model_detects_same_size_file_corruption() {
        let root = std::env::temp_dir().join(format!(
            "bitfun-speech-model-verify-test-{}",
            Uuid::new_v4().simple()
        ));
        let paths = SpeechStoragePaths::new(
            root.join("models"),
            root.join("downloads"),
            root.join("input"),
        );
        let store = SpeechModelStore::new(paths);
        let mut manifest = sensevoice_small_int8_manifest();
        manifest.id = "verify-test-model".to_string();
        manifest.version = "test-version".to_string();
        manifest.required_files = vec!["model.onnx".to_string(), "tokens.txt".to_string()];

        let model_dir = store.model_dir(&manifest);
        fs::create_dir_all(&model_dir).await.unwrap();
        fs::write(model_dir.join("model.onnx"), b"model-good")
            .await
            .unwrap();
        fs::write(model_dir.join("tokens.txt"), b"token-good")
            .await
            .unwrap();
        store
            .write_install_record(&manifest, &model_dir)
            .await
            .unwrap();

        let verified = store.verify_model(&manifest).await.unwrap();
        assert_eq!(verified.state, SpeechModelInstallState::Installed);

        fs::write(model_dir.join("model.onnx"), b"model-baad")
            .await
            .unwrap();
        let corrupt = store.verify_model(&manifest).await.unwrap();
        assert_eq!(corrupt.state, SpeechModelInstallState::Corrupt);
        assert!(corrupt
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("checksum mismatch"));

        let _ = fs::remove_dir_all(root).await;
    }
}
