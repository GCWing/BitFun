use super::model_store::{validate_relative_archive_path, SpeechModelStore};
use super::types::{
    SpeechModelArtifact, SpeechModelArtifactKind, SpeechModelManifest, SpeechModelProgress,
    SpeechModelStatus,
};
use super::{BitFunError, BitFunResult};
use bzip2::read::BzDecoder;
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tar::Archive;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub(super) async fn download_and_install_model<F>(
    store: &SpeechModelStore,
    manifest: &SpeechModelManifest,
    cancel: CancellationToken,
    on_progress: F,
) -> BitFunResult<SpeechModelStatus>
where
    F: Fn(SpeechModelProgress) + Send + Sync,
{
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(30 * 60))
        .build()
        .map_err(|error| BitFunError::Http(error.to_string()))?;
    let total_bytes = manifest.expected_bytes();
    let mut completed_bytes = 0u64;
    let mut downloaded_artifacts = Vec::with_capacity(manifest.artifacts.len());

    for artifact in &manifest.artifacts {
        let artifact_path = ensure_artifact_downloaded(
            store,
            manifest,
            artifact,
            &client,
            completed_bytes,
            total_bytes,
            cancel.clone(),
            &on_progress,
        )
        .await?;
        completed_bytes = completed_bytes.saturating_add(artifact.size_bytes);
        downloaded_artifacts.push((artifact.clone(), artifact_path));
    }

    on_progress(SpeechModelProgress {
        model_id: manifest.id.clone(),
        downloaded_bytes: total_bytes,
        total_bytes,
        percent: 100.0,
    });
    install_artifacts(store, manifest, &downloaded_artifacts).await?;
    store.status_for_manifest(manifest).await
}

async fn ensure_artifact_downloaded<F>(
    store: &SpeechModelStore,
    manifest: &SpeechModelManifest,
    artifact: &SpeechModelArtifact,
    client: &reqwest::Client,
    completed_bytes: u64,
    total_bytes: u64,
    cancel: CancellationToken,
    on_progress: &F,
) -> BitFunResult<PathBuf>
where
    F: Fn(SpeechModelProgress) + Send + Sync,
{
    let artifact_path = store.artifact_download_path(manifest, artifact);
    let partial_path = store.artifact_partial_path(manifest, artifact);
    if let Some(parent) = partial_path.parent() {
        fs::create_dir_all(parent).await?;
    }

    if artifact_path.exists() {
        let actual_hash = sha256_file(&artifact_path).await?;
        if actual_hash == artifact.sha256 {
            return Ok(artifact_path);
        }
        fs::remove_file(&artifact_path).await?;
    }

    let sources = std::iter::once(&artifact.source_url)
        .chain(artifact.fallback_source_urls.iter())
        .collect::<Vec<_>>();
    let mut source_errors = Vec::with_capacity(sources.len());

    for (source_index, source_url) in sources.iter().enumerate() {
        if cancel.is_cancelled() {
            let _ = fs::remove_file(&partial_path).await;
            return Err(download_cancelled_error(manifest));
        }
        let _ = fs::remove_file(&partial_path).await;
        on_progress(SpeechModelProgress {
            model_id: manifest.id.clone(),
            downloaded_bytes: completed_bytes,
            total_bytes,
            percent: progress_percent(completed_bytes, total_bytes),
        });

        match download_source(
            client,
            source_url,
            &partial_path,
            manifest,
            artifact,
            completed_bytes,
            total_bytes,
            &cancel,
            on_progress,
        )
        .await
        {
            Ok(()) => {
                if artifact_path.exists() {
                    fs::remove_file(&artifact_path).await?;
                }
                fs::rename(&partial_path, &artifact_path).await?;
                return Ok(artifact_path);
            }
            Err(error) => {
                let _ = fs::remove_file(&partial_path).await;
                if cancel.is_cancelled() {
                    return Err(download_cancelled_error(manifest));
                }
                log::warn!(
                    "Speech model download source failed: model_id={}, artifact_id={}, source_index={}, error={}",
                    manifest.id,
                    artifact.id,
                    source_index,
                    error
                );
                source_errors.push(error.to_string());
            }
        }
    }
    Err(BitFunError::Http(format!(
        "Speech model artifact download failed from all {} configured sources: {}",
        sources.len(),
        source_errors.join("; ")
    )))
}

async fn download_source<F>(
    client: &reqwest::Client,
    source_url: &str,
    partial_path: &Path,
    manifest: &SpeechModelManifest,
    artifact: &SpeechModelArtifact,
    completed_bytes: u64,
    model_total_bytes: u64,
    cancel: &CancellationToken,
    on_progress: &F,
) -> BitFunResult<()>
where
    F: Fn(SpeechModelProgress) + Send + Sync,
{
    let response_request = client
        .get(source_url)
        .header(reqwest::header::USER_AGENT, "BitFun")
        .send();
    let response = tokio::select! {
        _ = cancel.cancelled() => {
            return Err(download_cancelled_error(manifest));
        }
        response = response_request => response,
    }
    .map_err(|error| BitFunError::Http(error.to_string()))?
    .error_for_status()
    .map_err(|error| BitFunError::Http(error.to_string()))?;

    let total_bytes = response.content_length().unwrap_or(artifact.size_bytes);
    let mut stream = response.bytes_stream();
    let mut file = fs::File::create(&partial_path).await?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0u64;

    loop {
        let next_chunk = tokio::select! {
            _ = cancel.cancelled() => {
                drop(file);
                let _ = fs::remove_file(&partial_path).await;
                return Err(download_cancelled_error(manifest));
            }
            chunk = stream.next() => chunk,
        };
        let Some(chunk) = next_chunk else {
            break;
        };

        let chunk = chunk.map_err(|error| BitFunError::Http(error.to_string()))?;
        file.write_all(&chunk).await?;
        hasher.update(&chunk);
        downloaded += chunk.len() as u64;
        let percent = if total_bytes > 0 {
            progress_percent(
                completed_bytes.saturating_add(downloaded),
                model_total_bytes,
            )
        } else {
            0.0
        };
        on_progress(SpeechModelProgress {
            model_id: manifest.id.clone(),
            downloaded_bytes: completed_bytes.saturating_add(downloaded),
            total_bytes: model_total_bytes,
            percent,
        });
    }
    file.flush().await?;
    drop(file);

    if cancel.is_cancelled() {
        let _ = fs::remove_file(&partial_path).await;
        return Err(download_cancelled_error(manifest));
    }

    let actual_hash = format!("{:x}", hasher.finalize());
    if actual_hash != artifact.sha256 {
        let _ = fs::remove_file(&partial_path).await;
        return Err(BitFunError::validation(format!(
            "Speech model checksum mismatch: expected={}, actual={}",
            artifact.sha256, actual_hash
        )));
    }

    Ok(())
}

fn progress_percent(downloaded_bytes: u64, total_bytes: u64) -> f64 {
    if total_bytes > 0 {
        downloaded_bytes as f64 / total_bytes as f64 * 100.0
    } else {
        0.0
    }
}

fn download_cancelled_error(manifest: &SpeechModelManifest) -> BitFunError {
    BitFunError::Cancelled(format!("Speech model download cancelled: {}", manifest.id))
}

async fn install_artifacts(
    store: &SpeechModelStore,
    manifest: &SpeechModelManifest,
    artifacts: &[(SpeechModelArtifact, PathBuf)],
) -> BitFunResult<()> {
    let final_dir = store.model_dir(manifest);
    let parent = final_dir.parent().ok_or_else(|| {
        BitFunError::service(format!(
            "Speech model path has no parent: {}",
            final_dir.display()
        ))
    })?;
    fs::create_dir_all(parent).await?;

    let staging = parent.join(format!(".installing-{}", Uuid::new_v4().simple()));
    if staging.exists() {
        fs::remove_dir_all(&staging).await?;
    }
    fs::create_dir_all(&staging).await?;

    let install_result =
        install_artifacts_into_staging(store, manifest, artifacts, &staging, &final_dir).await;
    if install_result.is_err() && staging.exists() {
        let _ = fs::remove_dir_all(&staging).await;
    }
    install_result
}

async fn install_artifacts_into_staging(
    store: &SpeechModelStore,
    manifest: &SpeechModelManifest,
    artifacts: &[(SpeechModelArtifact, PathBuf)],
    staging: &Path,
    final_dir: &Path,
) -> BitFunResult<()> {
    for (artifact, path) in artifacts {
        match artifact.kind {
            SpeechModelArtifactKind::TarBz2 => {
                let archive_path = path.to_path_buf();
                let staging_for_extract = staging.to_path_buf();
                tokio::task::spawn_blocking(move || {
                    extract_tar_bz2(&archive_path, &staging_for_extract)
                })
                .await
                .map_err(|e| {
                    BitFunError::service(format!("Speech model extraction task failed: {e}"))
                })??;
            }
            SpeechModelArtifactKind::File => {
                let target_relative = artifact
                    .install_path
                    .as_deref()
                    .unwrap_or(&artifact.file_name);
                let target =
                    staging.join(validate_relative_archive_path(Path::new(target_relative))?);
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).await?;
                }
                fs::copy(path, target).await?;
            }
        }
    }

    let payload_dir = find_payload_dir(&staging, &manifest.required_files).await?;
    if final_dir.exists() {
        fs::remove_dir_all(&final_dir).await?;
    }

    if payload_dir == staging {
        fs::rename(&staging, &final_dir).await?;
    } else {
        fs::rename(&payload_dir, &final_dir).await?;
        if staging.exists() {
            fs::remove_dir_all(&staging).await?;
        }
    }

    store.write_install_record(manifest, &final_dir).await?;
    Ok(())
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

fn extract_tar_bz2(archive_path: &Path, destination: &Path) -> BitFunResult<()> {
    let file = File::open(archive_path)?;
    let decoder = BzDecoder::new(file);
    let mut archive = Archive::new(decoder);
    for entry in archive.entries()? {
        let mut entry = entry?;
        let relative = validate_relative_archive_path(&entry.path()?)?;
        let target = destination.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        entry.unpack(&target)?;
    }
    Ok(())
}

async fn find_payload_dir(staging: &Path, required_files: &[String]) -> BitFunResult<PathBuf> {
    if has_required_files_at(staging, required_files) {
        return Ok(staging.to_path_buf());
    }

    let mut entries = fs::read_dir(staging).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_dir() && has_required_files_at(&path, required_files) {
            return Ok(path);
        }
    }

    Err(BitFunError::validation(
        "Downloaded speech model archive does not contain the required model files",
    ))
}

fn has_required_files_at(path: &Path, required_files: &[String]) -> bool {
    required_files
        .iter()
        .all(|relative| path.join(relative).is_file())
}
