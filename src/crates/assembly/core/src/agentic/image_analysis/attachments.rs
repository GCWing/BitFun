//! Durable attachment preparation shared by every Agent input surface.

use super::{decode_data_url, ImageContextData};
use crate::agentic::tools::framework::{build_tool_runtime_artifact_reference, ToolUseContext};
use crate::util::errors::{OpenBitFunError, OpenBitFunResult};
use openbitfun_services_core::json_store::JsonFileStore;
use sha2::{Digest, Sha256};
use std::path::Path;

/// Inline pixels belong to the receiving Runtime, even in an SSH workspace.
/// Publish a stable artifact before admitting the message so text-only models
/// can call `analyze_image` and restored native multimodal turns can reload it.
/// Keep the inline payload for the transcript and for older message consumers.
pub(crate) async fn prepare_inline_image_attachments(
    images: &mut [ImageContextData],
    context: &ToolUseContext,
) -> OpenBitFunResult<()> {
    if !images.iter().any(|image| image.data_url.is_some()) {
        return Ok(());
    }
    let runtime_root = context.current_workspace_runtime_root()?;
    prepare_in_runtime_root(
        images,
        &runtime_root,
        context.current_workspace_scope().as_deref(),
        context.should_emit_runtime_uri(),
    )
    .await
}

async fn prepare_in_runtime_root(
    images: &mut [ImageContextData],
    runtime_root: &Path,
    workspace_scope: Option<&str>,
    emit_runtime_uri: bool,
) -> OpenBitFunResult<()> {
    for image in images {
        let Some(data_url) = image.data_url.as_deref() else {
            continue;
        };
        let (bytes, _) = decode_data_url(data_url)?;
        let format = image::guess_format(&bytes).map_err(|error| {
            OpenBitFunError::validation(format!("Invalid image attachment {}: {error}", image.id))
        })?;
        let (extension, mime_type) = match format {
            image::ImageFormat::Png => ("png", "image/png"),
            image::ImageFormat::Jpeg => ("jpg", "image/jpeg"),
            image::ImageFormat::Gif => ("gif", "image/gif"),
            image::ImageFormat::WebP => ("webp", "image/webp"),
            image::ImageFormat::Bmp => ("bmp", "image/bmp"),
            _ => {
                return Err(OpenBitFunError::validation(format!(
                "Unsupported image attachment format: {format:?}. Use PNG, JPEG, GIF, WebP or BMP."
            )))
            }
        };
        let relative_path = format!(
            "attachments/images/{:x}.{extension}",
            Sha256::digest(&bytes)
        );
        let path = runtime_root.join(&relative_path);
        // A replay or queued dispatch can prepare the same pixels twice. The
        // content address lets it reuse a complete, unchanged artifact.
        if tokio::fs::read(&path).await.ok().as_deref() != Some(bytes.as_slice()) {
            persist_attachment(&path, bytes).await?;
        }
        // Never reuse a controller-side path when pixels were supplied. A
        // runtime URI explicitly selects host artifact IO in remote workspaces.
        image.image_path = Some(
            build_tool_runtime_artifact_reference(
                &relative_path,
                Some(runtime_root),
                workspace_scope,
                emit_runtime_uri,
            )
            .map_err(|error| OpenBitFunError::validation(error.to_string()))?,
        );
        image.mime_type = mime_type.to_string();
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn test_image() -> ImageContextData {
    use base64::Engine;
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(8, 6)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    ImageContextData {
        id: "image-1".into(),
        image_path: None,
        data_url: Some(format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes.into_inner())
        )),
        mime_type: "image/png".into(),
        metadata: Some(serde_json::json!({"name": "screenshot.png"})),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn inline_pixels_survive_redaction_and_reuse_the_same_artifact_on_retry() {
        let root = tempfile::tempdir().unwrap();
        let mut images = vec![test_image()];
        images[0].image_path = Some("/controller-only/wrong.png".into());
        images[0].id = "../../not-a-filename".into();
        let original = images[0].data_url.clone();
        prepare_in_runtime_root(&mut images, root.path(), None, false)
            .await
            .unwrap();
        let path = images[0].image_path.clone().unwrap();
        assert!(Path::new(&path).starts_with(root.path()));
        assert_eq!(images[0].data_url, original);
        assert_eq!(
            images[0].metadata.as_ref().unwrap()["name"],
            "screenshot.png"
        );
        prepare_in_runtime_root(&mut images, root.path(), None, false)
            .await
            .unwrap();
        assert_eq!(images[0].image_path.as_deref(), Some(path.as_str()));

        images[0].data_url = None;
        let restored: Vec<ImageContextData> =
            serde_json::from_value(serde_json::to_value(images).unwrap()).unwrap();
        let processed =
            super::super::process_image_contexts_for_provider(&restored, "openai", None)
                .await
                .unwrap();
        assert_eq!((processed[0].width, processed[0].height), (8, 6));
    }

    #[tokio::test]
    async fn remote_upload_references_runtime_artifacts_without_exposing_host_paths() {
        let root = tempfile::tempdir().unwrap();
        let mut images = vec![test_image(), test_image()];
        images[1].id = "image-2".into();
        prepare_in_runtime_root(&mut images, root.path(), Some("remote-workspace"), true)
            .await
            .unwrap();
        let reference = images[0].image_path.as_deref().unwrap();
        assert!(reference.starts_with("openbitfun://runtime/remote-workspace/attachments/images/"));
        assert!(!reference.contains(root.path().to_str().unwrap()));
        assert_eq!(images[0].image_path, images[1].image_path);
        assert_ne!(images[0].id, images[1].id);
        let parsed =
            crate::agentic::tools::workspace_paths::parse_openbitfun_runtime_uri(reference)
                .unwrap();
        assert!(root.path().join(parsed.relative_path).is_file());
    }

    #[tokio::test]
    async fn malformed_inline_payload_is_rejected_without_using_a_supplied_path() {
        let root = tempfile::tempdir().unwrap();
        for data_url in [
            "data:image/png,hello",
            "data:image/png;base64,???",
            "data:image/png;base64,aGVsbG8=",
        ] {
            let mut images = vec![test_image()];
            images[0].image_path = Some("/valid-looking/file.png".into());
            images[0].data_url = Some(data_url.into());
            assert!(
                prepare_in_runtime_root(&mut images, root.path(), None, false)
                    .await
                    .is_err()
            );
        }
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }
}

async fn persist_attachment(path: &Path, bytes: Vec<u8>) -> OpenBitFunResult<()> {
    JsonFileStore
        .write_bytes_atomic_strict(path, bytes)
        .await
        .map_err(|error| OpenBitFunError::io(format!("Failed to save image attachment: {error}")))
}
