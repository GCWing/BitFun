//! Mobile-web relay upload helpers for Remote Connect.
//!
//! The relay upload protocol is a reusable integration detail. Product
//! assembly supplies the relay URL and room id; this module owns file
//! collection, hashing, incremental upload checks, and HTTP upload fallback.

use anyhow::{anyhow, Result};
use log::info;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

const MAX_UPLOAD_BATCH_BASE64_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct MobileWebUploadManifestEntry {
    path: String,
    hash: String,
    size: u64,
}

/// Collected file data ready for upload.
struct CollectedMobileWebFile {
    rel_path: String,
    content: Vec<u8>,
    hash: String,
}

fn mobile_web_upload_manifest(
    files: &[CollectedMobileWebFile],
) -> Vec<MobileWebUploadManifestEntry> {
    files
        .iter()
        .map(|file| MobileWebUploadManifestEntry {
            path: file.rel_path.clone(),
            hash: file.hash.clone(),
            size: file.content.len() as u64,
        })
        .collect()
}

/// Upload mobile-web assets to a relay server.
pub async fn upload_mobile_web_to_relay(
    relay_url: &str,
    room_id: &str,
    web_dir: &str,
) -> Result<()> {
    let all_files = collect_mobile_web_files(Path::new(web_dir))?;

    info!(
        "Collected {} mobile-web files ({} bytes total) for room {room_id}",
        all_files.len(),
        all_files
            .iter()
            .map(|file| file.content.len())
            .sum::<usize>()
    );

    let client = reqwest::Client::new();
    let relay_base = relay_url.trim_end_matches('/');

    let manifest = mobile_web_upload_manifest(&all_files);

    let check_url = format!("{relay_base}/api/rooms/{room_id}/check-web-files");
    let check_result = client
        .post(&check_url)
        .json(&serde_json::json!({ "files": manifest }))
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await;

    match check_result {
        Ok(resp) if resp.status().is_success() => {
            let body: serde_json::Value = resp
                .json()
                .await
                .map_err(|error| anyhow!("parse check-web-files response: {error}"))?;
            let needed: Vec<String> = body["needed"]
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|value| value.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            let existing = body["existing_count"].as_u64().unwrap_or(0);
            let total = body["total_count"].as_u64().unwrap_or(0);
            if needed.is_empty() {
                info!("All {total} files already exist on relay server, no upload needed");
                return Ok(());
            }

            info!(
                "Incremental upload: {existing}/{total} files already on server, uploading {} needed",
                needed.len()
            );

            upload_needed_files(&client, relay_base, room_id, &all_files, &needed).await
        }
        Ok(resp) if resp.status().as_u16() == 404 => {
            info!("Relay server does not support incremental upload, falling back to full upload");
            upload_all_files(&client, relay_base, room_id, &all_files).await
        }
        Ok(resp) => {
            let status = resp.status();
            info!("check-web-files returned HTTP {status}, falling back to full upload");
            upload_all_files(&client, relay_base, room_id, &all_files).await
        }
        Err(error) => {
            info!("check-web-files request failed ({error}), falling back to full upload");
            upload_all_files(&client, relay_base, room_id, &all_files).await
        }
    }
}

fn collect_mobile_web_files(base: &Path) -> Result<Vec<CollectedMobileWebFile>> {
    if !base.join("index.html").exists() {
        return Err(anyhow!(
            "mobile-web dir missing index.html: {}",
            base.display()
        ));
    }

    let mut all_files = Vec::new();
    collect_files_with_hash(base, base, &mut all_files)?;
    Ok(all_files)
}

async fn upload_needed_files(
    client: &reqwest::Client,
    relay_base: &str,
    room_id: &str,
    all_files: &[CollectedMobileWebFile],
    needed: &[String],
) -> Result<()> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};

    let needed_set: HashSet<&str> = needed.iter().map(String::as_str).collect();

    let mut files_payload: Vec<(String, serde_json::Value, usize)> = Vec::new();
    for file in all_files {
        if needed_set.contains(file.rel_path.as_str()) {
            let encoded = B64.encode(&file.content);
            let encoded_len = encoded.len();
            files_payload.push((
                file.rel_path.clone(),
                serde_json::json!({
                    "content": encoded,
                    "hash": file.hash,
                }),
                encoded_len,
            ));
        }
    }

    let url = format!("{relay_base}/api/rooms/{room_id}/upload-web-files");
    let total_b64_bytes: usize = files_payload.iter().map(|(_, _, len)| *len).sum();

    info!(
        "Uploading {} needed files ({} bytes base64) to {url}",
        files_payload.len(),
        total_b64_bytes
    );

    let mut current_batch: HashMap<String, serde_json::Value> = HashMap::new();
    let mut current_batch_b64_bytes = 0usize;
    let mut batch_index = 0usize;
    for (path, entry, entry_len) in files_payload {
        let should_flush = !current_batch.is_empty()
            && current_batch_b64_bytes + entry_len > MAX_UPLOAD_BATCH_BASE64_BYTES;
        if should_flush {
            upload_web_files_batch(
                client,
                &url,
                batch_index,
                &current_batch,
                current_batch_b64_bytes,
            )
            .await?;
            batch_index += 1;
            current_batch = HashMap::new();
            current_batch_b64_bytes = 0;
        }
        current_batch.insert(path, entry);
        current_batch_b64_bytes += entry_len;
    }

    if !current_batch.is_empty() {
        upload_web_files_batch(
            client,
            &url,
            batch_index,
            &current_batch,
            current_batch_b64_bytes,
        )
        .await?;
    }

    Ok(())
}

async fn upload_all_files(
    client: &reqwest::Client,
    relay_base: &str,
    room_id: &str,
    all_files: &[CollectedMobileWebFile],
) -> Result<()> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};

    let mut files: Vec<(String, String, usize)> = Vec::new();
    for file in all_files {
        let encoded = B64.encode(&file.content);
        let encoded_len = encoded.len();
        files.push((file.rel_path.clone(), encoded, encoded_len));
    }

    let url = format!("{relay_base}/api/rooms/{room_id}/upload-web");

    info!(
        "Full upload: {} files ({} bytes base64) to {url}",
        files.len(),
        files.iter().map(|(_, _, len)| *len).sum::<usize>()
    );

    let mut current_batch: HashMap<String, String> = HashMap::new();
    let mut current_batch_b64_bytes = 0usize;
    let mut batch_index = 0usize;
    for (path, encoded, encoded_len) in files {
        let should_flush = !current_batch.is_empty()
            && current_batch_b64_bytes + encoded_len > MAX_UPLOAD_BATCH_BASE64_BYTES;
        if should_flush {
            upload_web_legacy_batch(
                client,
                &url,
                batch_index,
                &current_batch,
                current_batch_b64_bytes,
            )
            .await?;
            batch_index += 1;
            current_batch = HashMap::new();
            current_batch_b64_bytes = 0;
        }
        current_batch.insert(path, encoded);
        current_batch_b64_bytes += encoded_len;
    }

    if !current_batch.is_empty() {
        upload_web_legacy_batch(
            client,
            &url,
            batch_index,
            &current_batch,
            current_batch_b64_bytes,
        )
        .await?;
    }

    Ok(())
}

async fn upload_web_files_batch(
    client: &reqwest::Client,
    url: &str,
    batch_index: usize,
    files_payload: &HashMap<String, serde_json::Value>,
    _total_b64_bytes: usize,
) -> Result<()> {
    let resp = client
        .post(url)
        .json(&serde_json::json!({ "files": files_payload }))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|error| anyhow!("upload-web-files batch {batch_index}: {error}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!(
            "upload-web-files batch {batch_index} failed: HTTP {status} \u{2014} {body}"
        ));
    }
    Ok(())
}

async fn upload_web_legacy_batch(
    client: &reqwest::Client,
    url: &str,
    batch_index: usize,
    files_payload: &HashMap<String, String>,
    _total_b64_bytes: usize,
) -> Result<()> {
    let resp = client
        .post(url)
        .json(&serde_json::json!({ "files": files_payload }))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|error| anyhow!("upload mobile-web batch {batch_index}: {error}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!(
            "upload mobile-web batch {batch_index} failed: HTTP {status} \u{2014} {body}"
        ));
    }
    Ok(())
}

fn collect_files_with_hash(
    base: &Path,
    dir: &Path,
    out: &mut Vec<CollectedMobileWebFile>,
) -> Result<()> {
    use sha2::{Digest, Sha256};

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_with_hash(base, &path, out)?;
        } else if path.is_file() {
            let rel = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let content = std::fs::read(&path)?;
            let mut hasher = Sha256::new();
            hasher.update(&content);
            let hash = format!("{:x}", hasher.finalize());
            out.push(CollectedMobileWebFile {
                rel_path: rel,
                content,
                hash,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_preserves_forward_slash_paths_and_hashes() {
        let base = std::env::temp_dir().join(format!(
            "bitfun-remote-mobile-web-manifest-{}",
            uuid::Uuid::new_v4()
        ));
        let assets = base.join("assets");
        std::fs::create_dir_all(&assets).unwrap();
        std::fs::write(base.join("index.html"), b"<html></html>").unwrap();
        std::fs::write(assets.join("app.js"), b"console.log('ok');").unwrap();

        let files = collect_mobile_web_files(&base).unwrap();
        let mut manifest = mobile_web_upload_manifest(&files);
        manifest.sort_by(|left, right| left.path.cmp(&right.path));

        assert_eq!(manifest.len(), 2);
        assert_eq!(manifest[0].path, "assets/app.js");
        assert_eq!(manifest[0].size, b"console.log('ok');".len() as u64);
        assert_eq!(
            manifest[0].hash,
            "16ba942cc0730b9c1416eb532c015b5d26bf8419618e315abe2544b87ae63a16"
        );
        assert_eq!(manifest[1].path, "index.html");
        assert_eq!(manifest[1].size, b"<html></html>".len() as u64);

        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn manifest_rejects_missing_index_html() {
        let base = std::env::temp_dir().join(format!(
            "bitfun-remote-mobile-web-missing-index-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&base).unwrap();

        let error = match collect_mobile_web_files(&base) {
            Ok(_) => panic!("missing index.html should be rejected"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("missing index.html"));
        let _ = std::fs::remove_dir_all(base);
    }
}
