//! Target-side assembly for output queries; does not initialize a live session.

use crate::agentic::WorkspaceBinding;
use crate::service_agent_runtime::CoreServiceAgentRuntime;
use base64::Engine as _;
use openbitfun_services_core::dispatch_contract::{
    DispatchFileChunk, DispatchFileChunkRequest, DISPATCH_FILE_CHUNK_MAX_BYTES,
};
use std::path::Path;

/// The target runner supplies the workspace and session from its durable job.
/// Paths and IO reuse the same session artifact routing as Remote Connect.
pub async fn read_dispatch_output_chunk(
    workspace: &Path,
    session_id: &str,
    reference: &str,
    request: &DispatchFileChunkRequest,
) -> Result<DispatchFileChunk, String> {
    if request.limit == 0 || request.limit > DISPATCH_FILE_CHUNK_MAX_BYTES {
        return Err("Invalid dispatch output chunk size".into());
    }
    if request.offset > 0 && request.expected_revision.is_none() {
        return Err("A file revision is required to resume this download".into());
    }
    let target = CoreServiceAgentRuntime::file_target_for_binding(
        reference,
        Some(session_id),
        WorkspaceBinding::new(None, workspace.to_path_buf()),
    )
    .await?;
    let (chunk, revision) = target
        .read_chunk_with_revision(
            request.offset,
            request.limit,
            request.expected_revision.as_deref(),
        )
        .await?;
    if revision.is_empty() {
        return Err("Output file revision is unavailable".into());
    }
    Ok(DispatchFileChunk {
        file_path: target.path,
        name: chunk.name,
        mime_type: chunk.mime_type.to_string(),
        total_size: chunk.total_size,
        offset: chunk.offset,
        chunk_size: chunk.chunk_size,
        content_base64: base64::engine::general_purpose::STANDARD.encode(chunk.bytes),
        revision,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn binary_output_is_resumable_confined_and_bound_to_its_revision() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path().join("job");
        std::fs::create_dir(&workspace).unwrap();
        let bytes = [137, 80, 78, 71, 0, 255, 0, 128];
        std::fs::write(workspace.join("preview 图.png"), bytes).unwrap();
        let first = read_dispatch_output_chunk(
            &workspace,
            "session",
            "computer://preview%20%E5%9B%BE.png",
            &DispatchFileChunkRequest {
                offset: 0,
                limit: 3,
                expected_revision: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(first.mime_type, "image/png");
        assert_eq!(first.name, "preview 图.png");
        assert_eq!(first.total_size, 8);
        let next_request = DispatchFileChunkRequest {
            offset: 3,
            limit: 5,
            expected_revision: Some(first.revision.clone()),
        };
        let second =
            read_dispatch_output_chunk(&workspace, "session", "preview 图.png", &next_request)
                .await
                .unwrap();
        let mut actual = base64::engine::general_purpose::STANDARD
            .decode(first.content_base64)
            .unwrap();
        actual.extend(
            base64::engine::general_purpose::STANDARD
                .decode(second.content_base64)
                .unwrap(),
        );
        assert_eq!(actual, bytes);
        std::fs::write(workspace.join("preview 图.png"), b"changed").unwrap();
        assert!(
            read_dispatch_output_chunk(&workspace, "session", "preview 图.png", &next_request)
                .await
                .unwrap_err()
                .contains("changed")
        );
        std::fs::write(tmp.path().join("private.png"), bytes).unwrap();
        assert!(read_dispatch_output_chunk(
            &workspace,
            "session",
            "../private.png",
            &DispatchFileChunkRequest {
                offset: 0,
                limit: 3,
                expected_revision: None
            }
        )
        .await
        .is_err());
        assert!(read_dispatch_output_chunk(
            &workspace,
            "session",
            "preview 图.png",
            &DispatchFileChunkRequest {
                offset: 3,
                limit: 3,
                expected_revision: None
            }
        )
        .await
        .is_err());
    }
}
