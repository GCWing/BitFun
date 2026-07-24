//! Disk cache for per-session facet extraction (fingerprint-invalidated).

use crate::agentic::insights::types::{SessionFacet, SessionTranscript};
use crate::infrastructure::get_path_manager_arc;
use crate::util::errors::BitFunResult;
use log::debug;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::fs;

const CACHE_SUBDIR: &str = "insights-facet-cache";

#[derive(Serialize, Deserialize)]
struct CachedFacetFile {
    fingerprint: String,
    facet: SessionFacet,
}

pub fn compute_fingerprint(transcript: &SessionTranscript) -> String {
    let mut hasher = Sha256::new();
    if let Some(workspace_path) = transcript.workspace_path.as_deref() {
        hasher.update(workspace_path.as_bytes());
    }
    hasher.update(b"|");
    hasher.update(transcript.session_id.as_bytes());
    hasher.update(b"|");
    hasher.update(transcript.last_activity_unix_secs.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(transcript.turn_count.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(transcript.transcript.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn cache_file_path(transcript: &SessionTranscript) -> BitFunResult<std::path::PathBuf> {
    let pm = get_path_manager_arc();
    let safe = transcript
        .session_id
        .chars()
        .map(|c| if "/\\:*?\"<>|".contains(c) { '_' } else { c })
        .collect::<String>();
    let mut workspace_hasher = Sha256::new();
    if let Some(workspace_path) = transcript.workspace_path.as_deref() {
        workspace_hasher.update(workspace_path.as_bytes());
    }
    let workspace_hash = format!("{:x}", workspace_hasher.finalize());
    Ok(pm
        .user_data_dir()
        .join(CACHE_SUBDIR)
        .join(format!("{}-{safe}.json", &workspace_hash[..16])))
}

pub async fn try_load_cached_facet(
    transcript: &SessionTranscript,
) -> BitFunResult<Option<SessionFacet>> {
    let path = match cache_file_path(transcript) {
        Ok(p) => p,
        Err(_) => return Ok(None),
    };
    let json = match fs::read_to_string(&path).await {
        Ok(s) => s,
        Err(_) => return Ok(None),
    };
    let parsed: CachedFacetFile = match serde_json::from_str(&json) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let want = compute_fingerprint(transcript);
    if parsed.fingerprint != want {
        return Ok(None);
    }
    Ok(Some(parsed.facet))
}

pub async fn save_cached_facet(
    transcript: &SessionTranscript,
    facet: &SessionFacet,
) -> BitFunResult<()> {
    let path = cache_file_path(transcript)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }
    let payload = CachedFacetFile {
        fingerprint: compute_fingerprint(transcript),
        facet: facet.clone(),
    };
    let json = serde_json::to_string_pretty(&payload)?;
    fs::write(&path, json).await?;
    debug!("Saved facet cache {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transcript(workspace_path: &str) -> SessionTranscript {
        SessionTranscript {
            session_id: "same-session".to_string(),
            agent_type: "Agentic".to_string(),
            session_name: "Session".to_string(),
            workspace_path: Some(workspace_path.to_string()),
            last_activity_unix_secs: 10,
            duration_minutes: 1,
            message_count: 2,
            turn_count: 1,
            created_at: "2026-07-16T00:00:00Z".to_string(),
            transcript: "[User]: test".to_string(),
            tool_names: vec![],
            has_errors: false,
        }
    }

    #[test]
    fn fingerprint_is_scoped_by_workspace() {
        assert_ne!(
            compute_fingerprint(&transcript("workspace-a")),
            compute_fingerprint(&transcript("workspace-b"))
        );
    }
}
