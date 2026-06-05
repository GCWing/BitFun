use agent_client_protocol::schema::{
    Annotations, BlobResourceContents, ContentBlock, EmbeddedResourceResource, ImageContent,
    ResourceLink, Role, TextResourceContents,
};
use bitfun_core::agentic::image_analysis::ImageContextData;

pub(super) struct ParsedPrompt {
    pub(super) user_message: String,
    pub(super) original_user_message: Option<String>,
    pub(super) image_contexts: Vec<ImageContextData>,
}

pub(super) fn parse_prompt_blocks(session_id: &str, blocks: Vec<ContentBlock>) -> ParsedPrompt {
    let mut text_parts = Vec::new();
    let mut original_text_parts = Vec::new();
    let mut image_contexts = Vec::new();

    for (index, block) in blocks.into_iter().enumerate() {
        match block {
            ContentBlock::Text(text) => {
                if is_user_only(text.annotations.as_ref()) {
                    continue;
                }
                original_text_parts.push(text.text.clone());
                text_parts.push(text.text);
            }
            ContentBlock::Image(image) => {
                if is_user_only(image.annotations.as_ref()) {
                    continue;
                }
                if let Some(context) = image_to_context(session_id, index, image) {
                    text_parts.push(format!("[Attached image: {}]", context.id));
                    image_contexts.push(context);
                }
            }
            ContentBlock::ResourceLink(link) => {
                if is_user_only(link.annotations.as_ref()) {
                    continue;
                }
                text_parts.push(resource_link_text(&link));
            }
            ContentBlock::Resource(resource) => {
                if is_user_only(resource.annotations.as_ref()) {
                    continue;
                }
                match resource.resource {
                    EmbeddedResourceResource::TextResourceContents(text) => {
                        text_parts.push(text_resource_text(&text));
                    }
                    EmbeddedResourceResource::BlobResourceContents(blob) => {
                        if let Some(context) =
                            blob_resource_to_image_context(session_id, index, &blob)
                        {
                            text_parts.push(format!("[Attached image resource: {}]", context.id));
                            image_contexts.push(context);
                        } else {
                            text_parts.push(blob_resource_text(&blob));
                        }
                    }
                    _ => {
                        text_parts.push(
                            "[Embedded resource omitted: unsupported resource type]".to_string(),
                        );
                    }
                }
            }
            ContentBlock::Audio(audio) => {
                if is_user_only(audio.annotations.as_ref()) {
                    continue;
                }
                text_parts.push(format!(
                    "[Audio attachment omitted: mime_type={}, bytes={}]",
                    audio.mime_type,
                    audio.data.len()
                ));
            }
            _ => {}
        }
    }

    let user_message = join_prompt_parts(text_parts);
    let original_user_message = if original_text_parts.is_empty() {
        None
    } else {
        Some(join_prompt_parts(original_text_parts))
    };

    ParsedPrompt {
        user_message,
        original_user_message,
        image_contexts,
    }
}

fn is_user_only(annotations: Option<&Annotations>) -> bool {
    matches!(
        annotations.and_then(|a| a.audience.as_ref()),
        Some(audience) if audience.len() == 1 && matches!(audience.first(), Some(Role::User))
    )
}

fn image_to_context(
    session_id: &str,
    index: usize,
    image: ImageContent,
) -> Option<ImageContextData> {
    if image.data.trim().is_empty() {
        return image.uri.clone().map(|uri| ImageContextData {
            id: prompt_context_id(session_id, "image", index),
            image_path: file_uri_to_path(&uri).or(Some(uri)),
            data_url: None,
            mime_type: image.mime_type,
            metadata: Some(serde_json::json!({
                "source": "acp",
                "uri": image.uri,
            })),
        });
    }

    Some(ImageContextData {
        id: prompt_context_id(session_id, "image", index),
        image_path: None,
        data_url: Some(format!("data:{};base64,{}", image.mime_type, image.data)),
        mime_type: image.mime_type,
        metadata: Some(serde_json::json!({
            "source": "acp",
            "uri": image.uri,
        })),
    })
}

fn blob_resource_to_image_context(
    session_id: &str,
    index: usize,
    blob: &BlobResourceContents,
) -> Option<ImageContextData> {
    let mime_type = blob
        .mime_type
        .clone()
        .unwrap_or_else(|| "application/octet-stream".to_string());
    if !mime_type.to_ascii_lowercase().starts_with("image/") {
        return None;
    }

    Some(ImageContextData {
        id: prompt_context_id(session_id, "resource_image", index),
        image_path: None,
        data_url: Some(format!("data:{};base64,{}", mime_type, blob.blob)),
        mime_type,
        metadata: Some(serde_json::json!({
            "source": "acp_resource",
            "uri": blob.uri,
        })),
    })
}

fn resource_link_text(link: &ResourceLink) -> String {
    let mut lines = vec![
        "[Attached resource link]".to_string(),
        format!("name: {}", link.name),
        format!("uri: {}", link.uri),
    ];
    if let Some(title) = &link.title {
        lines.push(format!("title: {}", title));
    }
    if let Some(description) = &link.description {
        lines.push(format!("description: {}", description));
    }
    if let Some(mime_type) = &link.mime_type {
        lines.push(format!("mime_type: {}", mime_type));
    }
    lines.join("\n")
}

fn text_resource_text(resource: &TextResourceContents) -> String {
    let language = resource
        .mime_type
        .as_deref()
        .and_then(markdown_language_for_mime)
        .unwrap_or("");
    format!(
        "[Embedded resource]\nuri: {}\nmime_type: {}\n```{}\n{}\n```",
        resource.uri,
        resource.mime_type.as_deref().unwrap_or("text/plain"),
        language,
        resource.text
    )
}

fn blob_resource_text(resource: &BlobResourceContents) -> String {
    format!(
        "[Embedded binary resource]\nuri: {}\nmime_type: {}\nbase64_bytes: {}",
        resource.uri,
        resource
            .mime_type
            .as_deref()
            .unwrap_or("application/octet-stream"),
        resource.blob.len()
    )
}

fn markdown_language_for_mime(mime_type: &str) -> Option<&'static str> {
    match mime_type.split(';').next()?.trim() {
        "application/json" => Some("json"),
        "application/javascript" | "text/javascript" => Some("javascript"),
        "text/css" => Some("css"),
        "text/html" => Some("html"),
        "text/markdown" => Some("markdown"),
        "text/x-python" => Some("python"),
        "text/x-rust" => Some("rust"),
        "text/x-typescript" => Some("typescript"),
        _ => None,
    }
}

fn join_prompt_parts(parts: Vec<String>) -> String {
    parts
        .into_iter()
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn prompt_context_id(session_id: &str, kind: &str, index: usize) -> String {
    let sanitized = session_id
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>();
    format!("acp_{}_{}_{}", kind, sanitized, index)
}

fn file_uri_to_path(uri: &str) -> Option<String> {
    uri.strip_prefix("file://").map(|path| path.to_string())
}
