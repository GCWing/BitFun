//! Mapping helpers from `rmcp` protocol models into BitFun MCP contracts.

use super::types::{
    InitializeResult, MCPAnnotations, MCPCapability, MCPPrompt, MCPPromptArgument,
    MCPPromptMessage, MCPPromptMessageContent, MCPPromptMessageContentBlock, MCPResource,
    MCPResourceContent, MCPResourceIcon, MCPServerInfo, MCPTool, MCPToolAnnotations, MCPToolResult,
    MCPToolResultContent, PromptsCapability, ResourcesCapability, ToolsCapability,
};
use rmcp::model::{Content, ResourceContents};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::HashMap;

pub fn map_rmcp_initialize_result(info: &rmcp::model::ServerInfo) -> InitializeResult {
    InitializeResult {
        protocol_version: info.protocol_version.to_string(),
        capabilities: map_rmcp_server_capabilities(&info.capabilities),
        server_info: MCPServerInfo {
            name: info.server_info.name.clone(),
            version: info.server_info.version.clone(),
            description: info.server_info.title.clone().or(info.instructions.clone()),
            vendor: None,
        },
    }
}

fn map_rmcp_server_capabilities(cap: &rmcp::model::ServerCapabilities) -> MCPCapability {
    MCPCapability {
        resources: cap.resources.as_ref().map(|r| ResourcesCapability {
            subscribe: r.subscribe.unwrap_or(false),
            list_changed: r.list_changed.unwrap_or(false),
        }),
        prompts: cap.prompts.as_ref().map(|p| PromptsCapability {
            list_changed: p.list_changed.unwrap_or(false),
        }),
        tools: cap.tools.as_ref().map(|t| ToolsCapability {
            list_changed: t.list_changed.unwrap_or(false),
        }),
        logging: cap.logging.as_ref().map(|o| Value::Object(o.clone())),
    }
}

pub fn map_rmcp_tool(tool: rmcp::model::Tool) -> MCPTool {
    let schema = Value::Object((*tool.input_schema).clone());
    MCPTool {
        name: tool.name.to_string(),
        title: tool.title,
        description: tool.description.map(|d| d.to_string()),
        input_schema: schema,
        output_schema: tool
            .output_schema
            .map(|schema| Value::Object((*schema).clone())),
        icons: map_rmcp_icons(tool.icons.as_ref()),
        annotations: tool.annotations.map(map_rmcp_tool_annotations),
        meta: map_optional_via_json(tool.meta.as_ref()),
    }
}

pub fn map_rmcp_resource(resource: rmcp::model::Resource) -> MCPResource {
    MCPResource {
        uri: resource.uri.clone(),
        name: resource.name.clone(),
        title: resource.title.clone(),
        description: resource.description.clone(),
        mime_type: resource.mime_type.clone(),
        icons: map_rmcp_icons(resource.icons.as_ref()),
        size: resource.size.map(u64::from),
        annotations: map_rmcp_annotations(resource.annotations.as_ref()),
        metadata: map_rmcp_meta_to_hash_map(resource.meta.as_ref()),
    }
}

pub fn map_rmcp_resource_content(contents: ResourceContents) -> MCPResourceContent {
    match contents {
        ResourceContents::TextResourceContents {
            uri,
            mime_type,
            text,
            meta,
            ..
        } => MCPResourceContent {
            uri,
            content: Some(text),
            blob: None,
            mime_type,
            annotations: None,
            meta: map_optional_via_json(meta.as_ref()),
        },
        ResourceContents::BlobResourceContents {
            uri,
            mime_type,
            blob,
            meta,
            ..
        } => MCPResourceContent {
            uri,
            content: None,
            blob: Some(blob),
            mime_type,
            annotations: None,
            meta: map_optional_via_json(meta.as_ref()),
        },
    }
}

pub fn map_rmcp_prompt(prompt: rmcp::model::Prompt) -> MCPPrompt {
    MCPPrompt {
        name: prompt.name,
        title: prompt.title,
        description: prompt.description,
        arguments: prompt.arguments.map(|args| {
            args.into_iter()
                .map(|a| MCPPromptArgument {
                    name: a.name,
                    title: a.title,
                    description: a.description,
                    required: a.required.unwrap_or(false),
                })
                .collect()
        }),
        icons: map_rmcp_icons(prompt.icons.as_ref()),
    }
}

pub fn map_rmcp_prompt_message(message: rmcp::model::PromptMessage) -> MCPPromptMessage {
    let role = match message.role {
        rmcp::model::PromptMessageRole::User => "user",
        rmcp::model::PromptMessageRole::Assistant => "assistant",
    }
    .to_string();

    let content = match message.content {
        rmcp::model::PromptMessageContent::Text { text } => {
            MCPPromptMessageContent::Block(Box::new(MCPPromptMessageContentBlock::Text { text }))
        }
        rmcp::model::PromptMessageContent::Image { image } => {
            MCPPromptMessageContent::Block(Box::new(MCPPromptMessageContentBlock::Image {
                data: image.data.clone(),
                mime_type: image.mime_type.clone(),
            }))
        }
        rmcp::model::PromptMessageContent::Resource { resource } => {
            let mut mapped = map_rmcp_resource_content(resource.resource.clone());
            if mapped.meta.is_none() {
                mapped.meta = map_optional_via_json(resource.meta.as_ref());
            }
            mapped.annotations = map_rmcp_annotations(resource.annotations.as_ref());
            MCPPromptMessageContent::Block(Box::new(MCPPromptMessageContentBlock::Resource {
                resource: Box::new(mapped),
            }))
        }
        rmcp::model::PromptMessageContent::ResourceLink { link } => {
            MCPPromptMessageContent::Block(Box::new(MCPPromptMessageContentBlock::ResourceLink {
                uri: link.uri.clone(),
                name: Some(link.name.clone()),
                description: link.description.clone(),
                mime_type: link.mime_type.clone(),
            }))
        }
    };

    MCPPromptMessage { role, content }
}

pub fn map_rmcp_tool_result(result: rmcp::model::CallToolResult) -> MCPToolResult {
    let mapped: Vec<MCPToolResultContent> = result
        .content
        .into_iter()
        .filter_map(map_rmcp_content_block)
        .collect();

    MCPToolResult {
        content: if mapped.is_empty() {
            None
        } else {
            Some(mapped)
        },
        is_error: result.is_error.unwrap_or(false),
        structured_content: result.structured_content,
        meta: map_optional_json_value(result.meta.as_ref()),
    }
}

fn map_rmcp_content_block(content: Content) -> Option<MCPToolResultContent> {
    match content.raw {
        rmcp::model::RawContent::Text(text) => Some(MCPToolResultContent::Text { text: text.text }),
        rmcp::model::RawContent::Image(image) => Some(MCPToolResultContent::Image {
            data: image.data,
            mime_type: image.mime_type,
        }),
        rmcp::model::RawContent::Resource(resource) => Some(MCPToolResultContent::Resource {
            resource: Box::new(map_rmcp_resource_content(resource.resource)),
        }),
        rmcp::model::RawContent::Audio(audio) => Some(MCPToolResultContent::Audio {
            data: audio.data,
            mime_type: audio.mime_type,
        }),
        rmcp::model::RawContent::ResourceLink(link) => Some(MCPToolResultContent::ResourceLink {
            uri: link.uri,
            name: Some(link.name),
            description: link.description,
            mime_type: link.mime_type,
        }),
    }
}

fn map_rmcp_icons(icons: Option<&Vec<rmcp::model::Icon>>) -> Option<Vec<MCPResourceIcon>> {
    icons.map(|icons| {
        icons
            .iter()
            .map(|icon| MCPResourceIcon {
                src: icon.src.clone(),
                mime_type: icon.mime_type.clone(),
                sizes: icon.sizes.as_ref().map(|sizes| {
                    Value::Array(sizes.iter().cloned().map(Value::String).collect::<Vec<_>>())
                }),
            })
            .collect()
    })
}

fn map_rmcp_annotations(annotations: Option<&rmcp::model::Annotations>) -> Option<MCPAnnotations> {
    annotations.map(|annotations| MCPAnnotations {
        audience: annotations
            .audience
            .as_ref()
            .map(|audience| audience.iter().map(map_rmcp_role).collect()),
        priority: annotations.priority.map(f64::from),
        last_modified: annotations
            .last_modified
            .map(|timestamp| timestamp.to_rfc3339()),
    })
}

fn map_rmcp_tool_annotations(annotations: rmcp::model::ToolAnnotations) -> MCPToolAnnotations {
    MCPToolAnnotations {
        title: annotations.title,
        read_only_hint: annotations.read_only_hint,
        destructive_hint: annotations.destructive_hint,
        idempotent_hint: annotations.idempotent_hint,
        open_world_hint: annotations.open_world_hint,
    }
}

fn map_rmcp_role(role: &rmcp::model::Role) -> String {
    match role {
        rmcp::model::Role::User => "user",
        rmcp::model::Role::Assistant => "assistant",
    }
    .to_string()
}

fn map_rmcp_meta_to_hash_map(meta: Option<&rmcp::model::Meta>) -> Option<HashMap<String, Value>> {
    meta.and_then(|meta| match serde_json::to_value(meta.clone()).ok()? {
        Value::Object(map) => Some(map.into_iter().collect()),
        _ => None,
    })
}

fn map_optional_json_value<T>(value: Option<&T>) -> Option<Value>
where
    T: serde::Serialize,
{
    value.and_then(|value| serde_json::to_value(value).ok())
}

fn map_optional_via_json<T, U>(value: Option<&T>) -> Option<U>
where
    T: serde::Serialize,
    U: DeserializeOwned,
{
    value
        .and_then(|value| serde_json::to_value(value).ok())
        .and_then(|value| serde_json::from_value(value).ok())
}
