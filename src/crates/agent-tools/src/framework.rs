use bitfun_core_types::ToolImageAttachment;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

/// Dynamic MCP tool subtype metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DynamicMcpToolInfo {
    pub server_id: String,
    pub server_name: String,
    pub tool_name: String,
}

/// Dynamic tool provider metadata used by registry and boundary adapters.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DynamicToolInfo {
    pub provider_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp: Option<DynamicMcpToolInfo>,
}

/// Tool result rendering options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolRenderOptions {
    pub verbose: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolPathBackend {
    Local,
    RemoteWorkspace,
}

#[derive(Debug, Clone)]
pub struct ToolPathResolution {
    pub requested_path: String,
    pub logical_path: String,
    pub resolved_path: String,
    pub backend: ToolPathBackend,
    pub runtime_scope: Option<String>,
    pub runtime_root: Option<PathBuf>,
}

impl ToolPathResolution {
    pub fn uses_remote_workspace_backend(&self) -> bool {
        matches!(self.backend, ToolPathBackend::RemoteWorkspace)
    }

    pub fn is_runtime_artifact(&self) -> bool {
        self.runtime_scope.is_some()
    }

    pub fn logical_child_path(&self, absolute_child_path: &Path) -> Option<String> {
        let scope = self.runtime_scope.as_deref()?;
        let root = self.runtime_root.as_ref()?;
        let relative = absolute_child_path.strip_prefix(root).ok()?;
        let relative_str = relative.to_string_lossy().replace('\\', "/");
        build_bitfun_runtime_uri(scope, &relative_str)
    }
}

fn build_bitfun_runtime_uri(workspace_scope: &str, relative_path: &str) -> Option<String> {
    let scope = workspace_scope.trim();
    if scope.is_empty() {
        return None;
    }

    Some(format!(
        "bitfun://runtime/{}/{}",
        scope,
        normalize_runtime_relative_path(relative_path)?
    ))
}

fn normalize_runtime_relative_path(path: &str) -> Option<String> {
    let normalized = path.trim().replace('\\', "/");
    let trimmed = normalized.trim_matches('/');
    if trimmed.is_empty() {
        return None;
    }

    let mut segments = Vec::new();
    for part in trimmed.split('/') {
        match part {
            "" | "." => continue,
            ".." => return None,
            value => segments.push(value.to_string()),
        }
    }

    if segments.is_empty() {
        return None;
    }

    Some(segments.join("/"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolPathOperation {
    Write,
    Edit,
    Delete,
}

impl ToolPathOperation {
    pub fn verb(self) -> &'static str {
        match self {
            Self::Write => "write",
            Self::Edit => "edit",
            Self::Delete => "delete",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolPathPolicy {
    #[serde(default)]
    pub write_roots: Vec<String>,
    #[serde(default)]
    pub edit_roots: Vec<String>,
    #[serde(default)]
    pub delete_roots: Vec<String>,
}

impl ToolPathPolicy {
    pub fn roots_for(&self, operation: ToolPathOperation) -> &[String] {
        match operation {
            ToolPathOperation::Write => &self.write_roots,
            ToolPathOperation::Edit => &self.edit_roots,
            ToolPathOperation::Delete => &self.delete_roots,
        }
    }

    pub fn is_restricted(&self, operation: ToolPathOperation) -> bool {
        !self.roots_for(operation).is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolRuntimeRestrictions {
    #[serde(default)]
    pub allowed_tool_names: BTreeSet<String>,
    #[serde(default)]
    pub denied_tool_names: BTreeSet<String>,
    #[serde(default)]
    pub path_policy: ToolPathPolicy,
}

impl ToolRuntimeRestrictions {
    pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
        (self.allowed_tool_names.is_empty() || self.allowed_tool_names.contains(tool_name))
            && !self.denied_tool_names.contains(tool_name)
    }

    pub fn ensure_tool_allowed(&self, tool_name: &str) -> Result<(), ToolRestrictionError> {
        if self.denied_tool_names.contains(tool_name) {
            return Err(ToolRestrictionError::Denied {
                tool_name: tool_name.to_string(),
            });
        }

        if !self.allowed_tool_names.is_empty() && !self.allowed_tool_names.contains(tool_name) {
            return Err(ToolRestrictionError::NotAllowed {
                tool_name: tool_name.to_string(),
            });
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolRestrictionError {
    Denied { tool_name: String },
    NotAllowed { tool_name: String },
}

impl fmt::Display for ToolRestrictionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Denied { tool_name } => write!(
                formatter,
                "Tool '{}' is denied by runtime restrictions",
                tool_name
            ),
            Self::NotAllowed { tool_name } => write!(
                formatter,
                "Tool '{}' is not allowed by runtime restrictions",
                tool_name
            ),
        }
    }
}

impl std::error::Error for ToolRestrictionError {}

/// Validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub result: bool,
    pub message: Option<String>,
    pub error_code: Option<i32>,
    pub meta: Option<Value>,
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self {
            result: true,
            message: None,
            error_code: None,
            meta: None,
        }
    }
}

/// Tool execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolResult {
    #[serde(rename = "result")]
    Result {
        data: Value,
        #[serde(default)]
        result_for_assistant: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        image_attachments: Option<Vec<ToolImageAttachment>>,
    },
    #[serde(rename = "progress")]
    Progress {
        content: Value,
        normalized_messages: Option<Vec<Value>>,
        tools: Option<Vec<String>>,
    },
    #[serde(rename = "stream_chunk")]
    StreamChunk {
        data: Value,
        chunk_index: usize,
        is_final: bool,
    },
}

impl ToolResult {
    /// Get content (for display)
    pub fn content(&self) -> Value {
        match self {
            ToolResult::Result { data, .. } => data.clone(),
            ToolResult::Progress { content, .. } => content.clone(),
            ToolResult::StreamChunk { data, .. } => data.clone(),
        }
    }

    /// Standard tool success without images.
    pub fn ok(data: Value, result_for_assistant: Option<String>) -> Self {
        Self::Result {
            data,
            result_for_assistant,
            image_attachments: None,
        }
    }

    /// Tool success with optional images for multimodal tool results (Anthropic).
    pub fn ok_with_images(
        data: Value,
        result_for_assistant: Option<String>,
        image_attachments: Vec<ToolImageAttachment>,
    ) -> Self {
        Self::Result {
            data,
            result_for_assistant,
            image_attachments: Some(image_attachments),
        }
    }
}
