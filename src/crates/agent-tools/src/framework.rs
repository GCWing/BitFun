use crate::{
    DynamicToolDescriptor, DynamicToolProvider, PortError, PortErrorKind, PortResult, ToolDecorator,
};
use async_trait::async_trait;
use bitfun_core_types::ToolImageAttachment;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolWorkspaceKind {
    Local,
    Remote,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolContextFacts {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialog_turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_kind: Option<ToolWorkspaceKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<String>,
    #[serde(default)]
    pub runtime_tool_restrictions: ToolRuntimeRestrictions,
}

pub trait PortableToolContextProvider: Send + Sync {
    fn tool_context_facts(&self) -> ToolContextFacts;
}

pub const GET_TOOL_SPEC_TOOL_NAME: &str = "GetToolSpec";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolExposure {
    Expanded,
    Collapsed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolManifestDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

impl ToolManifestDefinition {
    pub fn new(name: impl Into<String>, description: impl Into<String>, parameters: Value) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolManifestPolicyTool {
    pub name: String,
    pub default_exposure: ToolExposure,
    pub available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolManifestPolicyResolution {
    pub allowed_tool_names: Vec<String>,
    pub expanded_tool_names: Vec<String>,
    pub collapsed_tool_names: Vec<String>,
}

pub fn resolve_tool_manifest_policy(
    tool_snapshot: &[ToolManifestPolicyTool],
    allowed_tools: &[String],
    exposure_overrides: &IndexMap<String, ToolExposure>,
    get_tool_spec_tool_name: &str,
) -> ToolManifestPolicyResolution {
    let allowed_set = allowed_tools
        .iter()
        .map(String::as_str)
        .collect::<std::collections::HashSet<_>>();
    let mut allowed_tool_names = allowed_tools.to_vec();
    let mut expanded_tool_names = Vec::new();
    let mut collapsed_tool_names = Vec::new();

    for tool in tool_snapshot {
        if !tool.available || !allowed_set.contains(tool.name.as_str()) {
            continue;
        }

        let exposure = exposure_overrides
            .get(&tool.name)
            .copied()
            .unwrap_or(tool.default_exposure);
        match exposure {
            ToolExposure::Expanded => expanded_tool_names.push(tool.name.clone()),
            ToolExposure::Collapsed => collapsed_tool_names.push(tool.name.clone()),
        }
    }

    if !collapsed_tool_names.is_empty() {
        if !allowed_tool_names
            .iter()
            .any(|name| name == get_tool_spec_tool_name)
        {
            allowed_tool_names.push(get_tool_spec_tool_name.to_string());
        }
        if tool_snapshot
            .iter()
            .any(|tool| tool.name == get_tool_spec_tool_name)
        {
            expanded_tool_names.push(get_tool_spec_tool_name.to_string());
        }
    }

    ToolManifestPolicyResolution {
        allowed_tool_names,
        expanded_tool_names,
        collapsed_tool_names,
    }
}

pub fn build_collapsed_tool_stub_definition(
    tool_name: &str,
    short_description: &str,
) -> ToolManifestDefinition {
    ToolManifestDefinition::new(
        tool_name,
        format!(
            "{} [This tool is collapsed. Do not call `{}` directly yet. First call `GetToolSpec` with {{\"tool_name\":\"{}\"}} to load its full description and input schema, then retry `{}` using the returned schema.]",
            short_description, tool_name, tool_name, tool_name
        ),
        serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "tool_name": {
                    "type": "string",
                    "description": format!(
                        "Do not supply {} arguments here while the tool is collapsed. Use GetToolSpec with {{\"tool_name\":\"{}\"}} first.",
                        tool_name,
                        tool_name
                    )
                }
            }
        }),
    )
}

pub fn build_get_tool_spec_collapsed_tool_entry(
    tool_name: &str,
    short_description: &str,
) -> String {
    format!("- {}: {}", tool_name, short_description)
}

pub fn build_get_tool_spec_description(collapsed_tools_list: &str) -> String {
    format!(
        r#"Read usage instructions for additional tools.

You have access to the additional tools listed below. These tools are collapsed:
their names may appear in the tool list, but you must not call them directly
until you have loaded their definition with GetToolSpec.

<collapsed_tools>
{}
</collapsed_tools>

Before using one of these tools, first call GetToolSpec with its exact tool name
to read its full description and input schema. If a direct call to a collapsed
tool fails with a message like "Tool 'Git' is collapsed", make the next tool
call `GetToolSpec` with `{{"tool_name":"Git"}}`, then retry the real tool after
reading the returned schema.

After reading the returned definition, call the real tool directly using its own name.

Do not call GetToolSpec again for a tool whose definition is already loaded in the current conversation.

Example:
- Suppose the catalog includes a tool named `GetWeather` and you need to use it.
- First call `GetToolSpec` with `{{"tool_name":"GetWeather"}}`
- Then read the returned schema and call `GetWeather` itself with the appropriate arguments
"#,
        collapsed_tools_list
    )
}

pub fn get_tool_spec_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["tool_name"],
        "properties": {
            "tool_name": {
                "type": "string",
                "description": "Exact collapsed tool name to load, using the tool's canonical casing from the catalog (for example, \"Git\"). Do not pass a command such as \"git status\" or an operation such as \"status\" here."
            }
        }
    })
}

pub fn validate_get_tool_spec_input(input: &Value) -> ValidationResult {
    let Some(tool_name) = input.get("tool_name").and_then(|value| value.as_str()) else {
        return ValidationResult {
            result: false,
            message: Some("tool_name is required and cannot be empty".to_string()),
            error_code: Some(400),
            meta: None,
        };
    };

    if tool_name.is_empty() {
        return ValidationResult {
            result: false,
            message: Some("tool_name is required and cannot be empty".to_string()),
            error_code: Some(400),
            meta: None,
        };
    }

    ValidationResult::default()
}

pub fn build_get_tool_spec_duplicate_load_hint(tool_name: &str) -> String {
    format!(
        "Tool '{}' is already loaded in the current conversation. Do not call GetToolSpec again for it. Use '{}' directly.",
        tool_name, tool_name
    )
}

pub fn build_get_tool_spec_assistant_detail(description: &str, input_schema: &Value) -> String {
    format!(
        "<description>\n{}\n</description>\n<input_schema>\n{}\n</input_schema>",
        escape_get_tool_spec_xml_text(description),
        escape_get_tool_spec_xml_text(&input_schema.to_string())
    )
}

fn escape_get_tool_spec_xml_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn tool_manifest_sort_rank(tool_name: &str) -> usize {
    match tool_name {
        "Task" => 1,
        "Bash" => 2,
        "TerminalControl" => 3,
        "Glob" => 4,
        "Grep" => 5,
        "Read" => 6,
        "Edit" => 7,
        "Write" => 8,
        "Delete" => 9,
        "WebFetch" => 10,
        "WebSearch" => 11,
        "TodoWrite" => 12,
        "Skill" => 13,
        "Log" => 14,
        GET_TOOL_SPEC_TOOL_NAME => 15,
        "ControlHub" => 16,
        _ => 100,
    }
}

pub fn sort_tool_manifest_definitions(tool_definitions: &mut [ToolManifestDefinition]) {
    tool_definitions.sort_by_key(|tool| tool_manifest_sort_rank(&tool.name));
}

#[async_trait]
pub trait ToolRegistryItem: Send + Sync {
    fn name(&self) -> &str;

    async fn description(&self) -> Result<String, String>;

    fn input_schema(&self) -> Value;

    async fn input_schema_for_model(&self) -> Value {
        self.input_schema()
    }

    fn dynamic_provider_id(&self) -> Option<&str> {
        None
    }

    fn dynamic_tool_info(&self) -> Option<DynamicToolInfo> {
        self.dynamic_provider_id()
            .map(|provider_id| DynamicToolInfo {
                provider_id: provider_id.to_string(),
                provider_kind: None,
                mcp: None,
            })
    }
}

#[derive(Debug, Clone)]
struct DynamicToolMetadata {
    provider_id: String,
    info: DynamicToolInfo,
}

struct IdentityToolDecorator;

impl<Tool> ToolDecorator<Tool> for IdentityToolDecorator {
    fn decorate(&self, tool: Tool) -> Tool {
        tool
    }
}

pub type ToolRef<Tool> = Arc<Tool>;
pub type ToolDecoratorRef<Tool> = Arc<dyn ToolDecorator<ToolRef<Tool>>>;

pub trait StaticToolProvider<Tool: ?Sized>: Send + Sync {
    fn provider_id(&self) -> &'static str;

    fn tools(&self) -> Vec<ToolRef<Tool>>;
}

pub struct StaticToolProviderGroup<Tool: ?Sized> {
    provider_id: &'static str,
    tools: Vec<ToolRef<Tool>>,
}

impl<Tool: ?Sized> StaticToolProviderGroup<Tool> {
    pub fn new(provider_id: &'static str, tools: Vec<ToolRef<Tool>>) -> Self {
        Self { provider_id, tools }
    }
}

impl<Tool: ?Sized + Send + Sync> StaticToolProvider<Tool> for StaticToolProviderGroup<Tool> {
    fn provider_id(&self) -> &'static str {
        self.provider_id
    }

    fn tools(&self) -> Vec<ToolRef<Tool>> {
        self.tools.clone()
    }
}

pub struct ToolRegistry<Tool: ToolRegistryItem + ?Sized> {
    tools: IndexMap<String, ToolRef<Tool>>,
    dynamic_tools: IndexMap<String, DynamicToolMetadata>,
    tool_decorator: ToolDecoratorRef<Tool>,
}

impl<Tool: ToolRegistryItem + ?Sized> Default for ToolRegistry<Tool> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Tool: ToolRegistryItem + ?Sized> ToolRegistry<Tool> {
    pub fn new() -> Self {
        Self::with_tool_decorator(Arc::new(IdentityToolDecorator))
    }

    pub fn with_tool_decorator(tool_decorator: ToolDecoratorRef<Tool>) -> Self {
        Self {
            tools: IndexMap::new(),
            dynamic_tools: IndexMap::new(),
            tool_decorator,
        }
    }

    pub fn register_tool(&mut self, tool: ToolRef<Tool>) {
        let tool = self.tool_decorator.decorate(tool);
        let name = tool.name().to_string();
        let dynamic_info = tool.dynamic_tool_info().and_then(|info| {
            if info.provider_id.trim().is_empty() {
                None
            } else {
                Some(info)
            }
        });

        if let Some(info) = dynamic_info {
            self.dynamic_tools.insert(
                name.clone(),
                DynamicToolMetadata {
                    provider_id: info.provider_id.clone(),
                    info,
                },
            );
        } else {
            self.dynamic_tools.shift_remove(&name);
        }
        self.tools.insert(name, tool);
    }

    pub fn install_static_provider<Provider>(&mut self, provider: &Provider)
    where
        Provider: StaticToolProvider<Tool> + ?Sized,
    {
        for tool in provider.tools() {
            self.register_tool(tool);
        }
    }

    pub fn unregister_mcp_server_tools(&mut self, server_id: &str) {
        let to_remove = self
            .dynamic_tools
            .iter()
            .filter(|(_, metadata)| {
                metadata
                    .info
                    .mcp
                    .as_ref()
                    .is_some_and(|info| info.server_id == server_id)
            })
            .map(|(tool_name, _)| tool_name.clone())
            .collect::<Vec<_>>();

        for key in to_remove {
            self.tools.shift_remove(&key);
            self.dynamic_tools.shift_remove(&key);
        }
    }

    pub fn unregister_tools_by_prefix(&mut self, prefix: &str) -> usize {
        let to_remove = self
            .tools
            .keys()
            .filter(|key| key.starts_with(prefix))
            .cloned()
            .collect::<Vec<_>>();
        let count = to_remove.len();

        for key in to_remove {
            self.tools.shift_remove(&key);
            self.dynamic_tools.shift_remove(&key);
        }

        count
    }

    pub fn get_tool(&self, name: &str) -> Option<ToolRef<Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn get_dynamic_tool_info(&self, name: &str) -> Option<DynamicToolInfo> {
        self.dynamic_tools
            .get(name)
            .map(|metadata| metadata.info.clone())
    }

    pub fn get_tool_names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    pub fn get_all_tools(&self) -> Vec<ToolRef<Tool>> {
        self.tools.values().cloned().collect()
    }
}

#[async_trait]
impl<Tool: ToolRegistryItem + ?Sized> DynamicToolProvider for ToolRegistry<Tool> {
    async fn list_dynamic_tools(&self) -> PortResult<Vec<DynamicToolDescriptor>> {
        let mut descriptors = Vec::new();

        for (name, tool) in self.tools.iter() {
            let Some(metadata) = self.dynamic_tools.get(name) else {
                continue;
            };
            let description = tool
                .description()
                .await
                .map_err(|error| PortError::new(PortErrorKind::Backend, error))?;

            descriptors.push(DynamicToolDescriptor {
                name: tool.name().to_string(),
                description,
                input_schema: tool.input_schema_for_model().await,
                provider_id: Some(metadata.provider_id.clone()),
            });
        }

        Ok(descriptors)
    }
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
