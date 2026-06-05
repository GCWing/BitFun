use crate::{
    DynamicToolDescriptor, DynamicToolProvider, PortError, PortErrorKind, PortResult, ToolDecorator,
};
use async_trait::async_trait;
use bitfun_core_types::ToolImageAttachment;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt;
use std::marker::PhantomData;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollapsedToolUsageError {
    RequiresGetToolSpec {
        tool_name: String,
        get_tool_spec_tool_name: String,
    },
}

impl fmt::Display for CollapsedToolUsageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequiresGetToolSpec {
                tool_name,
                get_tool_spec_tool_name,
            } => write!(
                formatter,
                "Tool '{tool_name}' is collapsed. Call {get_tool_spec_tool_name} first with {{\"tool_name\":\"{tool_name}\"}} to read its full usage instructions and input schema, then try again."
            ),
        }
    }
}

impl std::error::Error for CollapsedToolUsageError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolExecutionAccessError {
    NotInAllowedList {
        tool_name: String,
        allowed_tools: Vec<String>,
    },
}

impl fmt::Display for ToolExecutionAccessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInAllowedList {
                tool_name,
                allowed_tools,
            } => write!(
                formatter,
                "Tool '{tool_name}' is not in the allowed list: {allowed_tools:?}"
            ),
        }
    }
}

impl std::error::Error for ToolExecutionAccessError {}

pub fn validate_tool_allowed_by_list(
    tool_name: &str,
    allowed_tools: &[String],
) -> Result<(), ToolExecutionAccessError> {
    if allowed_tools.is_empty() || allowed_tools.iter().any(|allowed| allowed == tool_name) {
        return Ok(());
    }

    Err(ToolExecutionAccessError::NotInAllowedList {
        tool_name: tool_name.to_string(),
        allowed_tools: allowed_tools.to_vec(),
    })
}

pub fn validate_collapsed_tool_usage(
    tool_name: &str,
    collapsed_tools: &[String],
    loaded_collapsed_tools: &[String],
    get_tool_spec_tool_name: &str,
) -> Result<(), CollapsedToolUsageError> {
    if tool_name == get_tool_spec_tool_name {
        return Ok(());
    }

    if !collapsed_tools
        .iter()
        .any(|collapsed_tool| collapsed_tool == tool_name)
    {
        return Ok(());
    }

    if loaded_collapsed_tools
        .iter()
        .any(|loaded_tool| loaded_tool == tool_name)
    {
        return Ok(());
    }

    Err(CollapsedToolUsageError::RequiresGetToolSpec {
        tool_name: tool_name.to_string(),
        get_tool_spec_tool_name: get_tool_spec_tool_name.to_string(),
    })
}

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

#[derive(Debug, Clone, PartialEq)]
pub enum PromptVisibleToolManifestItem {
    Expanded(ToolManifestDefinition),
    Collapsed {
        name: String,
        short_description: String,
    },
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

#[derive(Clone)]
pub struct ContextualVisibleTools<Tool: ?Sized> {
    pub allowed_tool_names: Vec<String>,
    pub expanded_tools: Vec<ToolRef<Tool>>,
    pub collapsed_tool_names: Vec<String>,
    pub collapsed_tools: Vec<ToolRef<Tool>>,
}

#[derive(Clone)]
pub struct ContextualToolManifest<Tool: ?Sized> {
    pub allowed_tool_names: Vec<String>,
    pub expanded_tools: Vec<ToolRef<Tool>>,
    pub collapsed_tool_names: Vec<String>,
    pub collapsed_tools: Vec<ToolRef<Tool>>,
    pub tool_definitions: Vec<ToolManifestDefinition>,
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

pub fn build_tool_manifest_policy_tools<Tool: ToolRegistryItem + ?Sized>(
    tool_snapshot: &[ToolRef<Tool>],
    available_tool_names: &HashSet<String>,
) -> Vec<ToolManifestPolicyTool> {
    tool_snapshot
        .iter()
        .map(|tool| {
            let name = tool.name().to_string();
            ToolManifestPolicyTool {
                available: available_tool_names.contains(&name),
                default_exposure: tool.default_exposure(),
                name,
            }
        })
        .collect()
}

fn tools_by_name<Tool: ToolRegistryItem + ?Sized>(
    tool_snapshot: &[ToolRef<Tool>],
    tool_names: &[String],
) -> Vec<ToolRef<Tool>> {
    tool_names
        .iter()
        .filter_map(|name| {
            tool_snapshot
                .iter()
                .find(|tool| tool.name() == name)
                .cloned()
        })
        .collect()
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetToolSpecCollapsedToolSummary {
    pub name: String,
    pub short_description: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GetToolSpecDetail {
    pub tool_name: String,
    pub description: String,
    pub input_schema: Value,
}

impl GetToolSpecDetail {
    pub fn to_value(&self) -> Value {
        serde_json::json!({
            "tool_name": self.tool_name.clone(),
            "description": self.description.clone(),
            "input_schema": self.input_schema.clone(),
        })
    }
}

pub fn build_get_tool_spec_catalog_description(
    collapsed_tools: &[GetToolSpecCollapsedToolSummary],
) -> String {
    let collapsed_tools_list = if collapsed_tools.is_empty() {
        "No additional tools are available.".to_string()
    } else {
        collapsed_tools
            .iter()
            .map(|tool| {
                build_get_tool_spec_collapsed_tool_entry(&tool.name, &tool.short_description)
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    build_get_tool_spec_description(&collapsed_tools_list)
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

pub fn get_tool_spec_short_description() -> String {
    "Discover collapsed tools and read their detailed definitions.".to_string()
}

pub fn get_tool_spec_is_readonly() -> bool {
    true
}

pub fn get_tool_spec_is_concurrency_safe(_input: Option<&Value>) -> bool {
    true
}

pub fn get_tool_spec_needs_permissions(_input: Option<&Value>) -> bool {
    false
}

pub fn render_get_tool_spec_tool_use_message(input: &Value) -> String {
    let tool_name = input
        .get("tool_name")
        .and_then(|value| value.as_str())
        .unwrap_or("?");
    format!("Reading tool spec for '{}'.", tool_name)
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

pub fn build_get_tool_spec_duplicate_load_result(tool_name: &str) -> ToolResult {
    ToolResult::Result {
        data: serde_json::json!({
            "tool_name": tool_name,
            "already_loaded": true
        }),
        result_for_assistant: Some(build_get_tool_spec_duplicate_load_hint(tool_name)),
        image_attachments: None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GetToolSpecExecutionError {
    MissingToolName,
    Detail(String),
}

impl fmt::Display for GetToolSpecExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GetToolSpecExecutionError::MissingToolName => write!(f, "tool_name is required"),
            GetToolSpecExecutionError::Detail(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for GetToolSpecExecutionError {}

#[derive(Debug, Clone)]
pub enum GetToolSpecExecutionPlan<'a> {
    DuplicateLoad(ToolResult),
    LoadDetail { tool_name: &'a str },
}

pub fn resolve_get_tool_spec_execution_plan<'a>(
    input: &'a Value,
    loaded_collapsed_tools: &[String],
) -> Result<GetToolSpecExecutionPlan<'a>, GetToolSpecExecutionError> {
    let tool_name = input
        .get("tool_name")
        .and_then(|value| value.as_str())
        .ok_or(GetToolSpecExecutionError::MissingToolName)?;

    if loaded_collapsed_tools
        .iter()
        .any(|loaded| loaded == tool_name)
    {
        return Ok(GetToolSpecExecutionPlan::DuplicateLoad(
            build_get_tool_spec_duplicate_load_result(tool_name),
        ));
    }

    Ok(GetToolSpecExecutionPlan::LoadDetail { tool_name })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GetToolSpecLoadObservation<'a> {
    pub tool_name: &'a str,
    pub loaded_tool_name: Option<&'a str>,
    pub is_error: bool,
}

pub fn collect_loaded_collapsed_tool_names(
    observations: &[GetToolSpecLoadObservation<'_>],
    collapsed_tool_names: &[String],
    get_tool_spec_tool_name: &str,
) -> Vec<String> {
    let collapsed_set: HashSet<&str> = collapsed_tool_names.iter().map(String::as_str).collect();
    let mut loaded = BTreeSet::new();

    for observation in observations {
        if observation.is_error || observation.tool_name != get_tool_spec_tool_name {
            continue;
        }

        let Some(tool_name) = observation.loaded_tool_name else {
            continue;
        };

        if collapsed_set.contains(tool_name) {
            loaded.insert(tool_name.to_string());
        }
    }

    loaded.into_iter().collect()
}

pub fn build_get_tool_spec_assistant_detail(description: &str, input_schema: &Value) -> String {
    format!(
        "<description>\n{}\n</description>\n<input_schema>\n{}\n</input_schema>",
        escape_get_tool_spec_xml_text(description),
        escape_get_tool_spec_xml_text(&input_schema.to_string())
    )
}

pub fn build_get_tool_spec_detail_result(detail: &GetToolSpecDetail) -> ToolResult {
    ToolResult::Result {
        data: detail.to_value(),
        result_for_assistant: Some(build_get_tool_spec_assistant_detail(
            &detail.description,
            &detail.input_schema,
        )),
        image_attachments: None,
    }
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

pub fn build_prompt_visible_tool_manifest_definitions(
    items: &[PromptVisibleToolManifestItem],
) -> Vec<ToolManifestDefinition> {
    let mut definitions = items
        .iter()
        .map(|item| match item {
            PromptVisibleToolManifestItem::Expanded(definition) => definition.clone(),
            PromptVisibleToolManifestItem::Collapsed {
                name,
                short_description,
            } => build_collapsed_tool_stub_definition(name, short_description),
        })
        .collect::<Vec<_>>();
    sort_tool_manifest_definitions(&mut definitions);
    definitions
}

#[async_trait]
pub trait ToolRegistryItem: Send + Sync {
    fn name(&self) -> &str;

    async fn description(&self) -> Result<String, String>;

    fn input_schema(&self) -> Value;

    fn short_description(&self) -> String {
        self.name().to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Expanded
    }

    fn is_readonly(&self) -> bool {
        false
    }

    async fn is_enabled(&self) -> bool {
        true
    }

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

#[async_trait]
pub trait ContextualToolManifestItem<Context>: ToolRegistryItem
where
    Context: Sync,
{
    async fn is_available_in_context(&self, _context: &Context) -> bool {
        true
    }

    /// Return the tool description that will be sent to the AI provider.
    ///
    /// # Prefix-cache stability contract
    ///
    /// The byte output of this method must be identical across every round of
    /// the same session for the same logical tool configuration. Any variation
    /// in the returned string invalidates the provider-side prefix cache for
    /// all bytes that follow the tool-spec block, which can significantly
    /// increase per-round cost.
    ///
    /// Acceptable variation:
    /// - Remote vs local workspace, which changes at session start and then stays stable.
    /// - Model capability flags such as vision support, which are stable per session.
    /// - User-initiated config changes such as theme or locale.
    ///
    /// Forbidden variation:
    /// - Timestamps, request IDs, UUIDs, or any non-deterministic data.
    /// - Session-specific paths that change mid-session.
    /// - Anything that varies between API calls within the same session.
    async fn description_with_context(&self, _context: &Context) -> Result<String, String> {
        self.description().await
    }

    /// Return the JSON schema sent to the AI provider.
    ///
    /// Subject to the same prefix-cache stability contract as
    /// [`Self::description_with_context`]: output must be byte-stable across
    /// rounds of the same session for the same tool configuration.
    async fn input_schema_for_model_with_context(&self, _context: &Context) -> Value {
        self.input_schema_for_model().await
    }
}

#[async_trait]
pub trait ToolCatalogSnapshotProvider<Tool: ?Sized>: Send + Sync {
    async fn tool_snapshot(&self) -> Vec<ToolRef<Tool>>;
}

#[async_trait]
pub trait GetToolSpecCatalogProvider<Tool: ?Sized, Context>: Send + Sync
where
    Context: Sync,
{
    async fn collapsed_tools_for_get_tool_spec(
        &self,
        context: Option<&Context>,
    ) -> Result<Vec<ToolRef<Tool>>, String>;
}

pub fn summarize_get_tool_spec_collapsed_tools<Tool: ToolRegistryItem + ?Sized>(
    collapsed_tools: &[ToolRef<Tool>],
) -> Vec<GetToolSpecCollapsedToolSummary> {
    collapsed_tools
        .iter()
        .map(|tool| GetToolSpecCollapsedToolSummary {
            name: tool.name().to_string(),
            short_description: tool.short_description(),
        })
        .collect()
}

pub async fn resolve_readonly_enabled_tools<Tool: ToolRegistryItem + ?Sized>(
    tool_snapshot: &[ToolRef<Tool>],
) -> Vec<ToolRef<Tool>> {
    let mut readonly_tools = Vec::new();

    for tool in tool_snapshot {
        if tool.is_readonly() && tool.is_enabled().await {
            readonly_tools.push(tool.clone());
        }
    }

    readonly_tools
}

pub struct ToolCatalogRuntime<'a, Tool: ?Sized, Context, Provider: ?Sized> {
    provider: &'a Provider,
    get_tool_spec_tool_name: &'a str,
    _marker: PhantomData<fn(&Tool, &Context)>,
}

impl<'a, Tool: ?Sized, Context, Provider: ?Sized> ToolCatalogRuntime<'a, Tool, Context, Provider> {
    pub fn new(provider: &'a Provider, get_tool_spec_tool_name: &'a str) -> Self {
        Self {
            provider,
            get_tool_spec_tool_name,
            _marker: PhantomData,
        }
    }
}

impl<'a, Tool, Context, Provider> ToolCatalogRuntime<'a, Tool, Context, Provider>
where
    Tool: ToolRegistryItem + ?Sized,
    Provider: ToolCatalogSnapshotProvider<Tool> + ?Sized,
{
    pub async fn readonly_enabled_tools(&self) -> Vec<ToolRef<Tool>> {
        let tool_snapshot = self.provider.tool_snapshot().await;
        resolve_readonly_enabled_tools(&tool_snapshot).await
    }
}

impl<'a, Tool, Context, Provider> ToolCatalogRuntime<'a, Tool, Context, Provider>
where
    Tool: ContextualToolManifestItem<Context> + ?Sized,
    Context: Sync,
    Provider: ToolCatalogSnapshotProvider<Tool> + ?Sized,
{
    pub async fn visible_tools(
        &self,
        allowed_tools: &[String],
        exposure_overrides: &IndexMap<String, ToolExposure>,
        context: &Context,
    ) -> ContextualVisibleTools<Tool> {
        resolve_contextual_visible_tools_from_provider(
            self.provider,
            allowed_tools,
            exposure_overrides,
            context,
            self.get_tool_spec_tool_name,
        )
        .await
    }

    pub async fn tool_manifest(
        &self,
        allowed_tools: &[String],
        exposure_overrides: &IndexMap<String, ToolExposure>,
        context: &Context,
    ) -> ContextualToolManifest<Tool> {
        resolve_contextual_tool_manifest_from_provider(
            self.provider,
            allowed_tools,
            exposure_overrides,
            context,
            self.get_tool_spec_tool_name,
        )
        .await
    }
}

pub async fn resolve_get_tool_spec_detail<Tool, Context>(
    collapsed_tools: &[ToolRef<Tool>],
    tool_name: &str,
    context: &Context,
    get_tool_spec_tool_name: &str,
) -> Result<GetToolSpecDetail, String>
where
    Tool: ContextualToolManifestItem<Context> + ?Sized,
    Context: Sync,
{
    let tool = collapsed_tools
        .iter()
        .find(|tool| tool.name() == tool_name)
        .ok_or_else(|| {
            format!("Tool '{tool_name}' is not an available collapsed tool in the current context")
        })?;

    if tool.name() == get_tool_spec_tool_name {
        return Err(format!("Tool '{tool_name}' cannot inspect itself"));
    }

    let description = tool
        .description_with_context(context)
        .await
        .unwrap_or_else(|_| format!("Tool: {}", tool.name()));
    let input_schema = tool.input_schema_for_model_with_context(context).await;

    Ok(GetToolSpecDetail {
        tool_name: tool_name.to_string(),
        description,
        input_schema,
    })
}

pub async fn build_get_tool_spec_catalog_description_from_provider<Tool, Context, Provider>(
    provider: &Provider,
    context: Option<&Context>,
) -> String
where
    Tool: ToolRegistryItem + ?Sized,
    Context: Sync,
    Provider: GetToolSpecCatalogProvider<Tool, Context> + ?Sized,
{
    let summaries = provider
        .collapsed_tools_for_get_tool_spec(context)
        .await
        .map(|tools| summarize_get_tool_spec_collapsed_tools(&tools))
        .unwrap_or_default();

    build_get_tool_spec_catalog_description(&summaries)
}

pub async fn resolve_get_tool_spec_detail_from_provider<Tool, Context, Provider>(
    provider: &Provider,
    tool_name: &str,
    context: &Context,
    get_tool_spec_tool_name: &str,
) -> Result<GetToolSpecDetail, String>
where
    Tool: ContextualToolManifestItem<Context> + ?Sized,
    Context: Sync,
    Provider: GetToolSpecCatalogProvider<Tool, Context> + ?Sized,
{
    let collapsed_tools = provider
        .collapsed_tools_for_get_tool_spec(Some(context))
        .await?;
    resolve_get_tool_spec_detail(
        &collapsed_tools,
        tool_name,
        context,
        get_tool_spec_tool_name,
    )
    .await
}

pub async fn resolve_get_tool_spec_execution_result_from_provider<Tool, Context, Provider>(
    provider: &Provider,
    input: &Value,
    loaded_collapsed_tools: &[String],
    context: &Context,
    get_tool_spec_tool_name: &str,
) -> Result<ToolResult, GetToolSpecExecutionError>
where
    Tool: ContextualToolManifestItem<Context> + ?Sized,
    Context: Sync,
    Provider: GetToolSpecCatalogProvider<Tool, Context> + ?Sized,
{
    match resolve_get_tool_spec_execution_plan(input, loaded_collapsed_tools)? {
        GetToolSpecExecutionPlan::DuplicateLoad(result) => Ok(result),
        GetToolSpecExecutionPlan::LoadDetail { tool_name } => {
            let detail = resolve_get_tool_spec_detail_from_provider(
                provider,
                tool_name,
                context,
                get_tool_spec_tool_name,
            )
            .await
            .map_err(GetToolSpecExecutionError::Detail)?;
            Ok(build_get_tool_spec_detail_result(&detail))
        }
    }
}

pub struct GetToolSpecRuntime<'a, Tool: ?Sized, Context, Provider: ?Sized> {
    provider: &'a Provider,
    tool_name: &'a str,
    _marker: PhantomData<fn(&Tool, &Context)>,
}

impl<'a, Tool: ?Sized, Context, Provider: ?Sized> GetToolSpecRuntime<'a, Tool, Context, Provider> {
    pub fn new(provider: &'a Provider, tool_name: &'a str) -> Self {
        Self {
            provider,
            tool_name,
            _marker: PhantomData,
        }
    }

    pub fn name(&self) -> &str {
        self.tool_name
    }

    pub fn short_description(&self) -> String {
        get_tool_spec_short_description()
    }

    pub fn input_schema(&self) -> Value {
        get_tool_spec_input_schema()
    }

    pub fn is_readonly(&self) -> bool {
        get_tool_spec_is_readonly()
    }

    pub fn is_concurrency_safe(&self, input: Option<&Value>) -> bool {
        get_tool_spec_is_concurrency_safe(input)
    }

    pub fn needs_permissions(&self, input: Option<&Value>) -> bool {
        get_tool_spec_needs_permissions(input)
    }

    pub fn render_tool_use_message(&self, input: &Value) -> String {
        render_get_tool_spec_tool_use_message(input)
    }

    pub fn validate_input(&self, input: &Value) -> ValidationResult {
        validate_get_tool_spec_input(input)
    }
}

impl<'a, Tool, Context, Provider> GetToolSpecRuntime<'a, Tool, Context, Provider>
where
    Tool: ToolRegistryItem + ?Sized,
    Context: Sync,
    Provider: GetToolSpecCatalogProvider<Tool, Context> + ?Sized,
{
    pub async fn catalog_description(&self, context: Option<&Context>) -> String {
        build_get_tool_spec_catalog_description_from_provider(self.provider, context).await
    }
}

impl<'a, Tool, Context, Provider> GetToolSpecRuntime<'a, Tool, Context, Provider>
where
    Tool: ContextualToolManifestItem<Context> + ?Sized,
    Context: Sync,
    Provider: GetToolSpecCatalogProvider<Tool, Context> + ?Sized,
{
    pub async fn execute(
        &self,
        input: &Value,
        loaded_collapsed_tools: &[String],
        context: &Context,
    ) -> Result<ToolResult, GetToolSpecExecutionError> {
        resolve_get_tool_spec_execution_result_from_provider(
            self.provider,
            input,
            loaded_collapsed_tools,
            context,
            self.tool_name,
        )
        .await
    }

    pub async fn call_results(
        &self,
        input: &Value,
        loaded_collapsed_tools: &[String],
        context: &Context,
    ) -> Result<Vec<ToolResult>, GetToolSpecExecutionError> {
        self.execute(input, loaded_collapsed_tools, context)
            .await
            .map(|result| vec![result])
    }
}

pub async fn resolve_contextual_visible_tools_from_provider<Tool, Context, Provider>(
    provider: &Provider,
    allowed_tools: &[String],
    exposure_overrides: &IndexMap<String, ToolExposure>,
    context: &Context,
    get_tool_spec_tool_name: &str,
) -> ContextualVisibleTools<Tool>
where
    Tool: ContextualToolManifestItem<Context> + ?Sized,
    Context: Sync,
    Provider: ToolCatalogSnapshotProvider<Tool> + ?Sized,
{
    let tool_snapshot = provider.tool_snapshot().await;
    resolve_contextual_visible_tools(
        &tool_snapshot,
        allowed_tools,
        exposure_overrides,
        context,
        get_tool_spec_tool_name,
    )
    .await
}

pub async fn resolve_contextual_tool_manifest_from_provider<Tool, Context, Provider>(
    provider: &Provider,
    allowed_tools: &[String],
    exposure_overrides: &IndexMap<String, ToolExposure>,
    context: &Context,
    get_tool_spec_tool_name: &str,
) -> ContextualToolManifest<Tool>
where
    Tool: ContextualToolManifestItem<Context> + ?Sized,
    Context: Sync,
    Provider: ToolCatalogSnapshotProvider<Tool> + ?Sized,
{
    let tool_snapshot = provider.tool_snapshot().await;
    resolve_contextual_tool_manifest(
        &tool_snapshot,
        allowed_tools,
        exposure_overrides,
        context,
        get_tool_spec_tool_name,
    )
    .await
}

pub async fn resolve_contextual_visible_tools<Tool, Context>(
    tool_snapshot: &[ToolRef<Tool>],
    allowed_tools: &[String],
    exposure_overrides: &IndexMap<String, ToolExposure>,
    context: &Context,
    get_tool_spec_tool_name: &str,
) -> ContextualVisibleTools<Tool>
where
    Tool: ContextualToolManifestItem<Context> + ?Sized,
    Context: Sync,
{
    let mut available_tool_names = HashSet::new();
    for tool in tool_snapshot {
        if tool.is_available_in_context(context).await {
            available_tool_names.insert(tool.name().to_string());
        }
    }

    let policy_tools = build_tool_manifest_policy_tools(tool_snapshot, &available_tool_names);
    let policy = resolve_tool_manifest_policy(
        &policy_tools,
        allowed_tools,
        exposure_overrides,
        get_tool_spec_tool_name,
    );
    let expanded_tools = tools_by_name(tool_snapshot, &policy.expanded_tool_names);
    let collapsed_tools = tools_by_name(tool_snapshot, &policy.collapsed_tool_names);

    ContextualVisibleTools {
        allowed_tool_names: policy.allowed_tool_names,
        expanded_tools,
        collapsed_tool_names: policy.collapsed_tool_names,
        collapsed_tools,
    }
}

pub async fn resolve_contextual_tool_manifest<Tool, Context>(
    tool_snapshot: &[ToolRef<Tool>],
    allowed_tools: &[String],
    exposure_overrides: &IndexMap<String, ToolExposure>,
    context: &Context,
    get_tool_spec_tool_name: &str,
) -> ContextualToolManifest<Tool>
where
    Tool: ContextualToolManifestItem<Context> + ?Sized,
    Context: Sync,
{
    let visible_tools = resolve_contextual_visible_tools(
        tool_snapshot,
        allowed_tools,
        exposure_overrides,
        context,
        get_tool_spec_tool_name,
    )
    .await;

    let mut manifest_items = Vec::with_capacity(
        visible_tools.expanded_tools.len() + visible_tools.collapsed_tools.len(),
    );
    for tool in &visible_tools.expanded_tools {
        let description = tool
            .description_with_context(context)
            .await
            .unwrap_or_else(|_| format!("Tool: {}", tool.name()));
        let parameters = tool.input_schema_for_model_with_context(context).await;

        manifest_items.push(PromptVisibleToolManifestItem::Expanded(
            ToolManifestDefinition::new(tool.name().to_string(), description, parameters),
        ));
    }

    for tool in &visible_tools.collapsed_tools {
        manifest_items.push(PromptVisibleToolManifestItem::Collapsed {
            name: tool.name().to_string(),
            short_description: tool.short_description(),
        });
    }

    let tool_definitions = build_prompt_visible_tool_manifest_definitions(&manifest_items);

    ContextualToolManifest {
        allowed_tool_names: visible_tools.allowed_tool_names,
        expanded_tools: visible_tools.expanded_tools,
        collapsed_tool_names: visible_tools.collapsed_tool_names,
        collapsed_tools: visible_tools.collapsed_tools,
        tool_definitions,
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

pub trait SnapshotToolWrapper<Tool: ?Sized>: Send + Sync {
    fn wrap_for_snapshot_tracking(&self, tool: ToolRef<Tool>) -> ToolRef<Tool>;
}

pub type SnapshotToolWrapperRef<Tool> = Arc<dyn SnapshotToolWrapper<Tool>>;

pub struct SnapshotToolDecorator<Tool: ?Sized> {
    wrapper: SnapshotToolWrapperRef<Tool>,
}

impl<Tool: ?Sized> SnapshotToolDecorator<Tool> {
    pub fn new(wrapper: SnapshotToolWrapperRef<Tool>) -> Self {
        Self { wrapper }
    }
}

impl<Tool: ?Sized> ToolDecorator<ToolRef<Tool>> for SnapshotToolDecorator<Tool> {
    fn decorate(&self, tool: ToolRef<Tool>) -> ToolRef<Tool> {
        self.wrapper.wrap_for_snapshot_tracking(tool)
    }
}

pub trait StaticToolProvider<Tool: ?Sized>: Send + Sync {
    fn provider_id(&self) -> &'static str;

    fn tools(&self) -> Vec<ToolRef<Tool>>;
}

pub trait StaticToolProviderPlan {
    fn provider_id(&self) -> &'static str;

    fn tool_names(&self) -> &'static [&'static str];
}

pub trait StaticToolProviderFactory<Tool: ?Sized> {
    fn materialize_tool(&self, tool_name: &str) -> Option<ToolRef<Tool>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaticToolMaterializationError {
    UnknownTool {
        provider_id: &'static str,
        tool_name: &'static str,
    },
}

impl std::fmt::Display for StaticToolMaterializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownTool {
                provider_id,
                tool_name,
            } => write!(
                f,
                "unknown static tool {tool_name} in provider group {provider_id}"
            ),
        }
    }
}

impl std::error::Error for StaticToolMaterializationError {}

pub struct StaticToolProviderGroup<Tool: ?Sized> {
    provider_id: &'static str,
    tools: Vec<ToolRef<Tool>>,
}

impl<Tool: ?Sized> std::fmt::Debug for StaticToolProviderGroup<Tool> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StaticToolProviderGroup")
            .field("provider_id", &self.provider_id)
            .field("tool_count", &self.tools.len())
            .finish()
    }
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

pub fn materialize_static_tool_provider_groups<Tool, Plan, Factory>(
    plans: &[Plan],
    factory: &Factory,
) -> Result<Vec<StaticToolProviderGroup<Tool>>, StaticToolMaterializationError>
where
    Tool: ?Sized,
    Plan: StaticToolProviderPlan,
    Factory: StaticToolProviderFactory<Tool> + ?Sized,
{
    let mut providers = Vec::new();
    for plan in plans {
        let provider_id = plan.provider_id();
        let mut tools = Vec::new();
        for tool_name in plan.tool_names() {
            let tool = factory.materialize_tool(tool_name).ok_or(
                StaticToolMaterializationError::UnknownTool {
                    provider_id,
                    tool_name,
                },
            )?;
            tools.push(tool);
        }
        providers.push(StaticToolProviderGroup::new(provider_id, tools));
    }
    Ok(providers)
}

pub struct ToolRuntimeAssembly<Tool: ToolRegistryItem + ?Sized> {
    tool_decorator: ToolDecoratorRef<Tool>,
}

impl<Tool: ToolRegistryItem + ?Sized> Default for ToolRuntimeAssembly<Tool> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Tool: ToolRegistryItem + ?Sized> ToolRuntimeAssembly<Tool> {
    pub fn new() -> Self {
        Self::with_tool_decorator(Arc::new(IdentityToolDecorator))
    }

    pub fn with_tool_decorator(tool_decorator: ToolDecoratorRef<Tool>) -> Self {
        Self { tool_decorator }
    }

    pub fn create_registry_from_static_providers<Provider>(
        &self,
        providers: &[Provider],
    ) -> ToolRegistry<Tool>
    where
        Provider: StaticToolProvider<Tool>,
    {
        let mut registry = ToolRegistry::with_tool_decorator(self.tool_decorator.clone());
        for provider in providers {
            registry.install_static_provider(provider);
        }
        registry
    }

    pub fn create_registry_from_static_provider_plans<Plan, Factory>(
        &self,
        plans: &[Plan],
        factory: &Factory,
    ) -> Result<ToolRegistry<Tool>, StaticToolMaterializationError>
    where
        Plan: StaticToolProviderPlan,
        Factory: StaticToolProviderFactory<Tool> + ?Sized,
    {
        let providers = materialize_static_tool_provider_groups(plans, factory)?;
        Ok(self.create_registry_from_static_providers(&providers))
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

    pub fn is_tool_collapsed(&self, name: &str) -> bool {
        self.tools
            .get(name)
            .is_some_and(|tool| tool.default_exposure() == ToolExposure::Collapsed)
    }

    pub fn get_collapsed_tool_names(&self) -> Vec<String> {
        self.tools
            .iter()
            .filter_map(|(name, tool)| {
                (tool.default_exposure() == ToolExposure::Collapsed).then(|| name.clone())
            })
            .collect()
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
        build_bitfun_runtime_uri(scope, &relative_str).ok()
    }
}

pub const BITFUN_RUNTIME_URI_PREFIX: &str = "bitfun://runtime/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedBitFunRuntimeUri {
    pub workspace_scope: String,
    pub relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolPathContractError {
    EmptyRuntimeArtifactPath,
    RuntimeArtifactPathEscapesRoot,
    UnsupportedRuntimeUri { uri: String },
    MissingRuntimeUriWorkspaceScope,
    MissingRuntimeUriArtifactPath,
    EmptyRuntimeWorkspaceScope,
    RuntimeUriScopeMismatch { workspace_scope: String },
    MissingRuntimeRoot,
    EmptyPath,
    MissingWorkspaceRoot { path: String },
}

impl fmt::Display for ToolPathContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRuntimeArtifactPath => {
                write!(formatter, "Runtime artifact path cannot be empty")
            }
            Self::RuntimeArtifactPathEscapesRoot => {
                write!(formatter, "Runtime artifact path cannot escape its root")
            }
            Self::UnsupportedRuntimeUri { uri } => {
                write!(formatter, "Unsupported runtime URI: {uri}")
            }
            Self::MissingRuntimeUriWorkspaceScope => {
                write!(formatter, "Runtime URI is missing workspace scope")
            }
            Self::MissingRuntimeUriArtifactPath => {
                write!(formatter, "Runtime URI is missing artifact path")
            }
            Self::EmptyRuntimeWorkspaceScope => {
                write!(formatter, "Runtime URI workspace scope cannot be empty")
            }
            Self::RuntimeUriScopeMismatch { workspace_scope } => {
                write!(
                    formatter,
                    "Runtime URI scope '{workspace_scope}' does not match the current workspace"
                )
            }
            Self::MissingRuntimeRoot => {
                write!(
                    formatter,
                    "A workspace is required to resolve runtime artifacts"
                )
            }
            Self::EmptyPath => write!(formatter, "path cannot be empty"),
            Self::MissingWorkspaceRoot { path } => {
                write!(
                    formatter,
                    "A workspace path is required to resolve relative path: {path}"
                )
            }
        }
    }
}

impl std::error::Error for ToolPathContractError {}

pub fn is_bitfun_runtime_uri(path: &str) -> bool {
    path.trim().starts_with(BITFUN_RUNTIME_URI_PREFIX)
}

pub fn normalize_host_path(path: &str) -> String {
    let path = Path::new(path);
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !components.is_empty() {
                    components.pop();
                }
            }
            component => components.push(component),
        }
    }
    components
        .iter()
        .collect::<PathBuf>()
        .to_string_lossy()
        .to_string()
}

pub fn resolve_host_path_with_workspace(
    path: &str,
    workspace_root: Option<&Path>,
) -> Result<String, ToolPathContractError> {
    if Path::new(path).is_absolute() {
        Ok(normalize_host_path(path))
    } else {
        let base_path =
            workspace_root.ok_or_else(|| ToolPathContractError::MissingWorkspaceRoot {
                path: path.to_string(),
            })?;

        Ok(normalize_host_path(
            base_path.join(path).to_string_lossy().as_ref(),
        ))
    }
}

pub fn resolve_host_path(path: &str) -> Result<String, ToolPathContractError> {
    resolve_host_path_with_workspace(path, None)
}

pub fn resolve_workspace_tool_path(
    path: &str,
    workspace_root: Option<&str>,
    workspace_is_remote: bool,
) -> Result<String, ToolPathContractError> {
    if workspace_is_remote {
        posix_resolve_path_with_workspace(path, workspace_root)
    } else {
        resolve_host_path_with_workspace(path, workspace_root.map(Path::new))
    }
}

pub fn resolve_tool_path_with_context(
    path: &str,
    workspace_root: Option<&str>,
    workspace_is_remote: bool,
    workspace_scope: Option<&str>,
    runtime_root: Option<PathBuf>,
) -> Result<ToolPathResolution, ToolPathContractError> {
    if is_bitfun_runtime_uri(path) {
        let parsed = parse_bitfun_runtime_uri(path)?;
        let scope_matches = parsed.workspace_scope == "current"
            || workspace_scope == Some(parsed.workspace_scope.as_str());
        if !scope_matches {
            return Err(ToolPathContractError::RuntimeUriScopeMismatch {
                workspace_scope: parsed.workspace_scope,
            });
        }

        let runtime_root = runtime_root.ok_or(ToolPathContractError::MissingRuntimeRoot)?;
        let mut resolved_path = runtime_root.clone();
        for segment in parsed.relative_path.split('/') {
            resolved_path.push(segment);
        }

        let effective_scope = workspace_scope
            .map(str::to_string)
            .unwrap_or_else(|| parsed.workspace_scope.clone());
        let logical_path = build_bitfun_runtime_uri(&effective_scope, &parsed.relative_path)?;

        return Ok(ToolPathResolution {
            requested_path: path.to_string(),
            logical_path,
            resolved_path: resolved_path.to_string_lossy().to_string(),
            backend: ToolPathBackend::Local,
            runtime_scope: Some(effective_scope),
            runtime_root: Some(runtime_root),
        });
    }

    let resolved_path = resolve_workspace_tool_path(path, workspace_root, workspace_is_remote)?;
    Ok(ToolPathResolution {
        requested_path: path.to_string(),
        logical_path: resolved_path.clone(),
        resolved_path,
        backend: if workspace_is_remote {
            ToolPathBackend::RemoteWorkspace
        } else {
            ToolPathBackend::Local
        },
        runtime_scope: None,
        runtime_root: None,
    })
}

pub fn tool_path_is_effectively_absolute(path: &str, workspace_is_remote: bool) -> bool {
    if is_bitfun_runtime_uri(path) {
        return true;
    }

    if workspace_is_remote {
        posix_style_path_is_absolute(path)
    } else {
        Path::new(path).is_absolute()
    }
}

pub fn normalize_runtime_relative_path(path: &str) -> Result<String, ToolPathContractError> {
    let normalized = path.trim().replace('\\', "/");
    let trimmed = normalized.trim_matches('/');
    if trimmed.is_empty() {
        return Err(ToolPathContractError::EmptyRuntimeArtifactPath);
    }

    let mut segments = Vec::new();
    for part in trimmed.split('/') {
        match part {
            "" | "." => continue,
            ".." => return Err(ToolPathContractError::RuntimeArtifactPathEscapesRoot),
            value => segments.push(value.to_string()),
        }
    }

    if segments.is_empty() {
        return Err(ToolPathContractError::EmptyRuntimeArtifactPath);
    }

    Ok(segments.join("/"))
}

pub fn parse_bitfun_runtime_uri(
    path: &str,
) -> Result<ParsedBitFunRuntimeUri, ToolPathContractError> {
    let trimmed = path.trim();
    let suffix = trimmed
        .strip_prefix(BITFUN_RUNTIME_URI_PREFIX)
        .ok_or_else(|| ToolPathContractError::UnsupportedRuntimeUri {
            uri: path.to_string(),
        })?;

    let mut parts = suffix.splitn(2, '/');
    let workspace_scope = parts
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(ToolPathContractError::MissingRuntimeUriWorkspaceScope)?
        .to_string();
    let relative_path = parts
        .next()
        .ok_or(ToolPathContractError::MissingRuntimeUriArtifactPath)?;

    Ok(ParsedBitFunRuntimeUri {
        workspace_scope,
        relative_path: normalize_runtime_relative_path(relative_path)?,
    })
}

pub fn build_bitfun_runtime_uri(
    workspace_scope: &str,
    relative_path: &str,
) -> Result<String, ToolPathContractError> {
    let scope = workspace_scope.trim();
    if scope.is_empty() {
        return Err(ToolPathContractError::EmptyRuntimeWorkspaceScope);
    }

    Ok(format!(
        "{}{}/{}",
        BITFUN_RUNTIME_URI_PREFIX,
        scope,
        normalize_runtime_relative_path(relative_path)?
    ))
}

pub fn build_tool_runtime_artifact_reference(
    relative_path: &str,
    runtime_root: Option<&Path>,
    workspace_scope: Option<&str>,
    emit_runtime_uri: bool,
) -> Result<String, ToolPathContractError> {
    let normalized_relative_path = normalize_runtime_relative_path(relative_path)?;
    if emit_runtime_uri {
        return build_bitfun_runtime_uri(
            workspace_scope.unwrap_or("current"),
            &normalized_relative_path,
        );
    }

    let runtime_root = runtime_root.ok_or(ToolPathContractError::MissingRuntimeRoot)?;
    let mut resolved_path = runtime_root.to_path_buf();
    for segment in normalized_relative_path.split('/') {
        resolved_path.push(segment);
    }

    Ok(resolved_path.to_string_lossy().to_string())
}

pub fn build_tool_session_runtime_artifact_reference(
    session_id: &str,
    relative_path: &str,
    runtime_root: Option<&Path>,
    workspace_scope: Option<&str>,
    emit_runtime_uri: bool,
) -> Result<String, ToolPathContractError> {
    let normalized_relative_path = normalize_runtime_relative_path(relative_path)?;
    build_tool_runtime_artifact_reference(
        &format!("sessions/{}/{}", session_id, normalized_relative_path),
        runtime_root,
        workspace_scope,
        emit_runtime_uri,
    )
}

pub fn posix_style_path_is_absolute(path: &str) -> bool {
    let path = path.trim().replace('\\', "/");
    path.starts_with('/')
}

pub fn normalize_absolute_posix_path(path: &str) -> String {
    let normalized = path.trim().replace('\\', "/");
    let is_absolute = normalized.starts_with('/');
    let mut segments = Vec::new();

    for segment in normalized.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                if !segments.is_empty() {
                    segments.pop();
                }
            }
            value => segments.push(value.to_string()),
        }
    }

    let body = segments.join("/");
    if is_absolute {
        if body.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", body)
        }
    } else {
        body
    }
}

pub fn is_remote_posix_path_within_root(path: &str, root: &str) -> bool {
    let normalized_path = normalize_absolute_posix_path(path);
    let normalized_root = normalize_absolute_posix_path(root);

    if !normalized_path.starts_with('/') || !normalized_root.starts_with('/') {
        return false;
    }

    if normalized_root == "/" {
        return true;
    }

    normalized_path == normalized_root
        || normalized_path
            .strip_prefix(&normalized_root)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

pub fn posix_resolve_path_with_workspace(
    path: &str,
    workspace_root: Option<&str>,
) -> Result<String, ToolPathContractError> {
    let path = path.trim();
    if path.is_empty() {
        return Err(ToolPathContractError::EmptyPath);
    }

    let normalized_input = path.replace('\\', "/");

    let combined = if posix_style_path_is_absolute(&normalized_input) {
        normalized_input
    } else {
        let base = workspace_root
            .ok_or_else(|| ToolPathContractError::MissingWorkspaceRoot {
                path: path.to_string(),
            })?
            .trim()
            .replace('\\', "/");
        let base = base.trim_end_matches('/');
        format!("{}/{}", base, normalized_input)
    };

    Ok(normalize_absolute_posix_path(&combined))
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

pub fn is_tool_path_allowed_by_resolved_roots<E>(
    resolution: &ToolPathResolution,
    resolved_roots: &[ToolPathResolution],
    mut root_contains_path: impl FnMut(&ToolPathResolution, &ToolPathResolution) -> Result<bool, E>,
) -> Result<bool, E> {
    for root in resolved_roots {
        if root.backend != resolution.backend {
            continue;
        }

        if root_contains_path(resolution, root)? {
            return Ok(true);
        }
    }

    Ok(false)
}

pub fn build_tool_path_policy_denial_message(
    logical_path: &str,
    operation: ToolPathOperation,
    allowed_roots: &[String],
) -> String {
    format!(
        "Path '{}' is not allowed for {}. Allowed roots: {}",
        logical_path,
        operation.verb(),
        allowed_roots.join(", ")
    )
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolRuntimeRestrictions {
    #[serde(default)]
    pub allowed_tool_names: BTreeSet<String>,
    #[serde(default)]
    pub denied_tool_names: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub denied_tool_messages: BTreeMap<String, String>,
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
                message: self.denied_tool_messages.get(tool_name).cloned(),
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
    Denied {
        tool_name: String,
        message: Option<String>,
    },
    NotAllowed {
        tool_name: String,
    },
}

impl fmt::Display for ToolRestrictionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Denied { tool_name, message } => {
                if let Some(message) = message.as_deref() {
                    write!(formatter, "{message}")
                } else {
                    write!(
                        formatter,
                        "Tool '{}' is denied by runtime restrictions",
                        tool_name
                    )
                }
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct TestTool {
        name: &'static str,
        available: bool,
    }

    #[async_trait]
    impl ToolRegistryItem for TestTool {
        fn name(&self) -> &str {
            self.name
        }

        async fn description(&self) -> Result<String, String> {
            Ok(format!("{} description", self.name))
        }

        fn input_schema(&self) -> Value {
            json!({
                "type": "object",
                "properties": {},
            })
        }
    }

    #[async_trait]
    impl ContextualToolManifestItem<()> for TestTool {
        async fn is_available_in_context(&self, _context: &()) -> bool {
            self.available
        }
    }

    #[tokio::test]
    async fn contextual_manifest_omits_unavailable_tools_from_model_definitions() {
        let task = Arc::new(TestTool {
            name: "Task",
            available: false,
        });
        let read = Arc::new(TestTool {
            name: "Read",
            available: true,
        });
        let tools: Vec<Arc<TestTool>> = vec![task, read];
        let allowed_tools = vec!["Task".to_string(), "Read".to_string()];

        let manifest = resolve_contextual_tool_manifest(
            &tools,
            &allowed_tools,
            &IndexMap::new(),
            &(),
            GET_TOOL_SPEC_TOOL_NAME,
        )
        .await;

        assert!(!manifest
            .tool_definitions
            .iter()
            .any(|definition| definition.name == "Task"));
        assert!(manifest
            .tool_definitions
            .iter()
            .any(|definition| definition.name == "Read"));
    }
}
