//! Unified configuration system type definitions
//!
//! Defines all configuration-related types shared between backend and frontend.

use crate::util::errors::*;
use async_trait::async_trait;
use bitfun_core_types::WorktreeSettings;
pub use bitfun_core_types::{ReasoningConfig, ReasoningPreset, ReasoningPresetAction};
use bitfun_runtime_ports::{PermissionRule, ToolPermissionConfig};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

fn deserialize_agent_profiles<'de, D>(
    deserializer: D,
) -> Result<HashMap<String, AgentProfileConfig>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<HashMap<String, Option<AgentProfileConfig>>>::deserialize(deserializer)?;
    Ok(raw
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(profile_id, config)| config.map(|config| (profile_id, config)))
        .collect())
}

/// Web UI font preferences (settings → basics). Keys match `FontPreference` in the frontend (camelCase).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FontPreferenceSnapshot {
    pub ui_size: UiFontSizeSnapshot,
    pub flow_chat: FlowChatFontSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiFontSizeSnapshot {
    pub level: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_px: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowChatFontSnapshot {
    pub mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_px: Option<u32>,
}

/// Global configuration structure - matches the frontend `GlobalConfig` exactly.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GlobalConfig {
    pub app: AppConfig,
    pub editor: EditorConfig,
    pub terminal: TerminalConfig,
    pub workspace: WorkspaceConfig,
    pub ai: AIConfig,
    /// User-level static tool permission policy and interaction preferences.
    #[serde(default)]
    pub tool_permissions: ToolPermissionConfig,
    #[serde(default)]
    pub memories: MemoriesConfig,
    /// Project-scoped overlays stored in the shared config document.
    #[serde(default, skip_serializing_if = "ProjectConfig::is_empty")]
    pub project: ProjectConfig,
    /// MCP server configuration (stored uniformly; supports both JSON and structured formats).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<serde_json::Value>,
    /// ACP client configuration (stored as `{ "acpClients": { ... } }`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acp_clients: Option<serde_json::Value>,
    /// Web UI appearance selection. The full package contract is owned by the frontend.
    pub appearance: AppearanceConfig,
    /// Web UI font size preferences (`get_config` / `set_config` path `font`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<FontPreferenceSnapshot>,
    /// Version of the persisted configuration schema. This is intentionally
    /// independent from the BitFun application version stored in `version`.
    #[serde(default = "default_config_schema_version")]
    pub schema_version: u32,
    pub version: String,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub last_modified: chrono::DateTime<chrono::Utc>,
}

/// Project-scoped configuration overlay.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectConfig {
    /// Project-level MCP server configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<serde_json::Value>,
}

impl ProjectConfig {
    fn is_empty(&self) -> bool {
        self.mcp_servers.is_none()
    }
}

/// App configuration.
fn default_close_button_behavior() -> String {
    "minimize_to_tray".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub language: String,
    pub auto_update: bool,
    pub telemetry: bool,
    pub startup_behavior: String,
    pub confirm_on_exit: bool,
    pub restore_windows: bool,
    /// Keep the local computer awake while the desktop application is running.
    pub prevent_sleep: bool,
    pub zoom_level: f64,
    #[serde(default)]
    pub logging: AppLoggingConfig,
    pub sidebar: SidebarConfig,
    pub right_panel: RightPanelConfig,
    pub notifications: NotificationConfig,
    #[serde(default)]
    pub flow_chat: AppFlowChatConfig,
    pub ai_experience: AIExperienceConfig,
    /// User-defined keyboard shortcut overrides.
    /// Stored as opaque JSON so the backend remains schema-agnostic;
    /// the frontend owns the versioned format (StoredKeybindingsV1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keybindings: Option<serde_json::Value>,
    /// Global, user-defined groups used to organize Agent tool pickers.
    #[serde(default, skip_serializing_if = "UserToolGroupsConfig::is_empty")]
    pub user_tool_groups: UserToolGroupsConfig,
    /// Global, user-defined groups used to organize Skill pickers.
    #[serde(default, skip_serializing_if = "UserSkillGroupsConfig::is_empty")]
    pub user_skill_groups: UserSkillGroupsConfig,
    /// What happens when the window close button is clicked on Windows / Linux.
    /// Allowed values: "quit" | "minimize_to_tray" | "ask".
    #[serde(default = "default_close_button_behavior")]
    pub close_button_behavior: String,
    /// Native agent lifecycle hooks (Codex-compatible hooks.json).
    #[serde(default)]
    pub hooks: AgentHooksConfig,
    /// Defaults for opt-in managed Git worktrees.
    #[serde(default)]
    pub worktrees: WorktreeSettings,
}

/// Enablement gates for native agent hooks.
///
/// Hook declarations themselves live in `hooks.json` documents (user scope:
/// `config/hooks.json` next to this file; project scope:
/// `{project}/.bitfun/config/hooks.json`), not in this settings document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AgentHooksConfig {
    /// Master switch for native agent hooks.
    pub enabled: bool,
    /// Whether project-scope hook files are honored. Disabled by default
    /// because project hook files execute commands from the checked-out
    /// repository; enable only for workspaces you trust.
    pub project_hooks_enabled: bool,
}

impl Default for AgentHooksConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            project_hooks_enabled: false,
        }
    }
}

/// Versioned user preference for grouping selectable Agent tools in the UI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UserToolGroupsConfig {
    pub version: u32,
    pub groups: Vec<UserToolGroup>,
}

impl UserToolGroupsConfig {
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }
}

impl Default for UserToolGroupsConfig {
    fn default() -> Self {
        Self {
            version: 1,
            groups: Vec::new(),
        }
    }
}

/// A user-defined group of canonical tool names.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UserToolGroup {
    pub id: String,
    pub name: String,
    pub tool_names: Vec<String>,
}

/// Versioned user preference for grouping selectable Skills in the UI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UserSkillGroupsConfig {
    pub version: u32,
    pub groups: Vec<UserSkillGroup>,
}

impl UserSkillGroupsConfig {
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }
}

impl Default for UserSkillGroupsConfig {
    fn default() -> Self {
        Self {
            version: 1,
            groups: Vec::new(),
        }
    }
}

/// A user-defined group of stable Skill keys.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UserSkillGroup {
    pub id: String,
    pub name: String,
    pub skill_keys: Vec<String>,
}

/// App logging configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppLoggingConfig {
    /// Runtime backend log level.
    /// Allowed values: trace, debug, info, warn, error, off.
    pub level: String,
    /// Whether diagnostic logs may include sensitive troubleshooting payloads.
    #[serde(default = "default_true")]
    pub include_sensitive_diagnostics: bool,
    /// Whether the local UI records detailed Flow Chat viewport diagnostics.
    #[serde(default)]
    pub flow_chat_diagnostics: bool,
    /// Per-request AI model exchange tracing configuration for developer diagnostics.
    #[serde(default)]
    pub model_exchange_tracing: ModelExchangeTracingConfig,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ModelExchangeTracingMode {
    #[default]
    Off,
    Full,
    UsageOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelExchangeTracingConfig {
    pub mode: ModelExchangeTracingMode,
}

/// FlowChat UI preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppFlowChatConfig {
    /// Optional user override for the default ChatInput mode id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_mode_id: Option<String>,
    /// Whether the chat input exposes the global permission-mode shortcut.
    ///
    /// The default is visible, but that value is omitted from persisted config
    /// so existing config files remain unchanged until the user hides it.
    #[serde(
        default = "default_show_permission_mode_control",
        skip_serializing_if = "is_permission_mode_control_visible"
    )]
    pub show_permission_mode_control: bool,
}

fn default_show_permission_mode_control() -> bool {
    true
}

fn is_permission_mode_control_visible(value: &bool) -> bool {
    *value
}

impl Default for AppFlowChatConfig {
    fn default() -> Self {
        Self {
            default_mode_id: None,
            show_permission_mode_control: default_show_permission_mode_control(),
        }
    }
}

/// A user-defined quick action for the FlowChat post-coding actions menu.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AiExperienceQuickAction {
    pub id: String,
    pub label: String,
    pub prompt: String,
    pub enabled: bool,
}

/// Local voice input preferences for the chat composer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceInputConfig {
    pub enabled: bool,
    pub provider: String,
    pub model_id: String,
    pub default_language: String,
    pub max_recording_seconds: u32,
    pub microphone_device_id: String,
}

impl Default for VoiceInputConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            provider: "local".to_string(),
            model_id: "sensevoice-small-int8".to_string(),
            default_language: "auto".to_string(),
            max_recording_seconds: 60,
            microphone_device_id: String::new(),
        }
    }
}

/// Domain request for atomically saving a cloud speech-recognition model and
/// selecting it for voice input. Text-generation fields are intentionally not
/// part of this contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SaveCloudSpeechConfigRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_id: Option<String>,
    pub preset: String,
    pub name: String,
    pub base_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_url: Option<String>,
    pub model_name: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SaveCloudSpeechConfigResult {
    pub model_id: String,
    pub created: bool,
}

/// AI experience configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AIExperienceConfig {
    /// Whether to enable automatic AI-generated summaries for session titles.
    pub enable_session_title_generation: bool,
    /// Whether to enable AI analysis of work status on the FlowChat welcome page.
    pub enable_welcome_panel_ai_analysis: bool,
    /// Whether to enable visual mode.
    pub enable_visual_mode: bool,
    /// Whether to show the pixel Agent companion in the collapsed chat input.
    pub enable_agent_companion: bool,
    /// Where to show the Agent companion: "input" or "desktop".
    pub agent_companion_display_mode: String,
    /// Optional Petdex-compatible companion package selected by the user.
    #[serde(
        default = "default_agent_companion_pet",
        skip_serializing_if = "Option::is_none"
    )]
    pub agent_companion_pet: Option<AgentCompanionPetSelection>,
    /// Whether to enable flashgrep-backed accelerated workspace search.
    pub enable_workspace_search: bool,
    /// Local speech-to-text settings for the chat composer.
    pub voice_input: VoiceInputConfig,
    /// User-defined quick actions (post-coding menu); persisted for the web UI.
    #[serde(default)]
    pub quick_actions: Vec<AiExperienceQuickAction>,
}

/// User-selected Agent companion pet package.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentCompanionPetSelection {
    pub id: String,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub source: String,
    pub package_path: String,
    pub spritesheet_path: String,
    pub spritesheet_mime_type: String,
}

fn default_agent_companion_pet() -> Option<AgentCompanionPetSelection> {
    Some(AgentCompanionPetSelection {
        id: "bitfun".to_string(),
        display_name: "Bitfun".to_string(),
        description: Some(
            "BitFun's mascot — Bifang, a figure from Chinese mythology said to live on Mount Zhang'e. In the Classic of Mountains and Seas (Shan Hai Jing · Western Mountains), Bifang is described as crane-like with one foot, blue feathers marked with red, and a white beak.".to_string(),
        ),
        source: "preset".to_string(),
        package_path: "/agent-companion-pets/bitfun".to_string(),
        spritesheet_path: "/agent-companion-pets/bitfun/spritesheet.webp".to_string(),
        spritesheet_mime_type: "image/webp".to_string(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SidebarConfig {
    pub width: u32,
    pub collapsed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RightPanelConfig {
    pub width: u32,
    pub collapsed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NotificationConfig {
    pub enabled: bool,
    pub position: String,
    pub duration: u32,
    /// Whether to show a toast notification when a dialog turn completes while the window is not focused.
    #[serde(default = "default_true")]
    pub dialog_completion_notify: bool,
    /// Whether to show a toast notification when an approval request arrives while the window is not focused.
    #[serde(default = "default_true")]
    pub permission_request_notify: bool,
    /// Whether to show built-in tip cards on startup (can be disabled by the user).
    #[serde(default = "default_true")]
    pub enable_startup_tips: bool,
}

/// Web UI appearance configuration. Rust stores only the selected package id.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceConfig {
    /// Selected appearance package ID or `system`.
    pub selection: String,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            selection: "system".to_string(),
        }
    }
}

/// Editor configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EditorConfig {
    pub font_size: u32,
    pub font_family: String,
    pub line_height: f64,
    pub tab_size: u32,
    pub insert_spaces: bool,
    pub word_wrap: String,
    pub line_numbers: String,
    pub minimap: MinimapConfig,
    pub auto_save: String,
    pub auto_save_delay: u32,
    pub format_on_save: bool,
    pub format_on_paste: bool,
    pub trim_auto_whitespace: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MinimapConfig {
    pub enabled: bool,
    pub side: String,
    pub size: String,
}

/// Terminal configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TerminalConfig {
    /// Empty string means "auto-detect".
    pub default_shell: String,
    /// Terminal panel placement in the session layout: "right" or "bottom".
    pub terminal_panel_position: String,
    pub font_size: u32,
    pub font_family: String,
    pub cursor_blink: bool,
    pub cursor_style: String,
    pub scrollback: u32,
}

/// Workspace configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceConfig {
    pub exclude_patterns: Vec<String>,
    pub include_patterns: Vec<String>,
    pub watch_ignore: Vec<String>,
    /// Maximum file size in bytes.
    pub max_file_size: u64,
    pub encoding: String,
    pub line_ending: String,
    pub trim_trailing_whitespace: bool,
    pub insert_final_newline: bool,
}

/// Model capability type (a model can have multiple capabilities).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ModelCapability {
    /// Text chat (primary capability).
    TextChat,
    /// Image understanding (vision).
    ImageUnderstanding,
    /// Image generation.
    ImageGeneration,
    /// Embeddings (semantic vectors).
    Embedding,
    /// Search API (e.g. Perplexity).
    Search,
    /// Code specialized.
    CodeSpecialized,
    /// Function calling / tool use.
    FunctionCalling,
    /// Speech-to-text.
    SpeechRecognition,
}

pub const CURRENT_CONFIG_SCHEMA_VERSION: u32 = 1;

fn default_config_schema_version() -> u32 {
    CURRENT_CONFIG_SCHEMA_VERSION
}

/// Model category (for UI display and filtering).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ModelCategory {
    /// General chat model.
    #[default]
    GeneralChat,
    /// Multimodal model (text + image understanding).
    Multimodal,
    /// Image generation model.
    ImageGeneration,
    /// Embedding / vector model.
    Embedding,
    /// Search-enhanced model.
    SearchEnhanced,
    /// Code-specialized model.
    CodeSpecialized,
    /// Speech recognition model.
    SpeechRecognition,
}

/// Default model configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct DefaultModelsConfig {
    /// Primary model ID (for complex tasks).
    pub primary: Option<String>,
    /// Fast model ID (for simple tasks).
    pub fast: Option<String>,
    /// Search model.
    pub search: Option<String>,
    /// Image understanding model.
    pub image_understanding: Option<String>,
    /// Image generation model.
    pub image_generation: Option<String>,
    /// Speech recognition model.
    pub speech_recognition: Option<String>,
}

/// Model choice for a subagent created in the context of a parent session.
///
/// `Inherit` is intentionally distinct from a model ID so a user-configured
/// model named `inherit` can never be interpreted as a control value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[derive(Default)]
pub enum SubagentModelSelection {
    Fixed {
        model_id: String,
    },
    #[default]
    Inherit,
}

impl SubagentModelSelection {
    pub fn fixed(model_id: impl Into<String>) -> Self {
        Self::Fixed {
            model_id: model_id.into(),
        }
    }

    pub fn fixed_model_id(&self) -> Option<&str> {
        match self {
            Self::Fixed { model_id } => Some(model_id.as_str()),
            Self::Inherit => None,
        }
    }
}

/// Model defaults for subagents created through user-visible delegation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SubagentModelDefaultsConfig {
    /// Shared fallback for normal subagents without an explicit override.
    #[serde(rename = "default", default = "default_subagent_model_selection")]
    pub default_selection: SubagentModelSelection,
    /// Per-builtin defaults and user overrides. Missing entries use `default`.
    pub builtin: HashMap<String, SubagentModelSelection>,
    /// Default choice for a child created from the parent's context.
    pub fork: SubagentModelSelection,
}

impl Default for SubagentModelDefaultsConfig {
    fn default() -> Self {
        Self {
            default_selection: default_subagent_model_selection(),
            builtin: HashMap::from([(
                "GeneralPurpose".to_string(),
                SubagentModelSelection::fixed("primary"),
            )]),
            fork: SubagentModelSelection::Inherit,
        }
    }
}

fn default_subagent_model_selection() -> SubagentModelSelection {
    SubagentModelSelection::fixed("fast")
}

/// Defaults used when the product creates an agent session without an explicit
/// per-session model choice.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentModelDefaultsConfig {
    /// Shared model selector for future mode sessions.
    pub mode: String,
    /// User-visible delegated subagent model choices.
    pub subagents: SubagentModelDefaultsConfig,
}

impl AgentModelDefaultsConfig {
    pub fn builtin_subagent_selection(&self, agent_id: &str) -> SubagentModelSelection {
        self.subagents
            .builtin
            .get(agent_id)
            .cloned()
            .unwrap_or_else(|| self.subagents.default_selection.clone())
    }
}

impl Default for AgentModelDefaultsConfig {
    fn default() -> Self {
        Self {
            mode: "auto".to_string(),
            subagents: SubagentModelDefaultsConfig::default(),
        }
    }
}

/// Default review-team execution policy and membership configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ReviewTeamConfig {
    /// Additional reviewer subagent IDs configured by the user.
    pub extra_subagent_ids: Vec<String>,
    /// Default review depth used by the whole review team.
    pub strategy_level: String,
    /// Per-reviewer review depth overrides keyed by subagent ID.
    pub member_strategy_overrides: HashMap<String, String>,
    /// Optional timeout applied to reviewer Task calls. 0 disables the cap.
    pub reviewer_timeout_seconds: u64,
    /// Optional timeout applied to ReviewJudge Task calls. 0 disables the cap.
    pub judge_timeout_seconds: u64,
    /// Whether ReviewFixer may be launched by DeepReview.
    pub auto_fix_enabled: bool,
    /// Minimum number of target files that triggers same-role reviewer splitting.
    /// 0 disables file splitting.
    pub reviewer_file_split_threshold: usize,
    /// Maximum number of same-role reviewer instances per role when file splitting is active.
    pub max_same_role_instances: usize,
}

impl Default for ReviewTeamConfig {
    fn default() -> Self {
        Self {
            extra_subagent_ids: Vec::new(),
            strategy_level: "normal".to_string(),
            member_strategy_overrides: HashMap::new(),
            reviewer_timeout_seconds: 3600,
            judge_timeout_seconds: 2400,
            auto_fix_enabled: false,
            reviewer_file_split_threshold: 20,
            max_same_role_instances: 3,
        }
    }
}

fn default_review_team_configs() -> HashMap<String, ReviewTeamConfig> {
    HashMap::from([("default".to_string(), ReviewTeamConfig::default())])
}

fn default_review_team_rate_limit_status() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

/// AI configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AIConfig {
    /// All configured models.
    pub models: Vec<AIModelConfig>,

    /// Model mapping for functional agents (e.g. startchat-func-agent, session-title-func-agent).
    /// func_agent_name -> model_id
    #[serde(default)]
    pub func_agent_models: HashMap<String, String>,

    /// Default model configuration.
    #[serde(default)]
    pub default_models: DefaultModelsConfig,

    /// Default selectors for future mode and delegated-subagent sessions.
    #[serde(default)]
    pub agent_model_defaults: AgentModelDefaultsConfig,

    /// Shared agent-profile configuration.
    /// profile_id -> AgentProfileConfig
    #[serde(default, deserialize_with = "deserialize_agent_profiles")]
    pub agent_profiles: HashMap<String, AgentProfileConfig>,

    /// User-level Skill availability shared by every agent profile.
    #[serde(default)]
    pub skill_settings: SkillSettingsConfig,

    /// Review team configuration.
    /// team_id -> ReviewTeamConfig
    #[serde(default = "default_review_team_configs")]
    pub review_teams: HashMap<String, ReviewTeamConfig>,

    /// Runtime rate-limit snapshot for Review Team launches.
    #[serde(default = "default_review_team_rate_limit_status")]
    pub review_team_rate_limit_status: serde_json::Value,

    /// Maximum number of subagents that may execute concurrently.
    #[serde(default = "default_subagent_max_concurrency")]
    pub subagent_max_concurrency: usize,

    /// Scheduling policy for multiple subagent launch calls in the same model batch.
    #[serde(default = "default_subagent_batch_execution_policy")]
    pub subagent_batch_execution_policy: SubagentBatchExecutionPolicy,

    /// Global proxy configuration.
    pub proxy: ProxyConfig,

    /// Streaming idle timeout in seconds; `None` means wait indefinitely.
    #[serde(default = "default_stream_idle_timeout")]
    pub stream_idle_timeout_secs: Option<u64>,

    /// Time-to-first-token timeout in seconds while opening a streaming request;
    /// `None` means wait indefinitely.
    #[serde(default = "default_stream_ttft_timeout")]
    pub stream_ttft_timeout_secs: Option<u64>,

    /// Tool execution timeout in seconds; `None` means wait indefinitely.
    #[serde(default = "default_tool_execution_timeout")]
    pub tool_execution_timeout_secs: Option<u64>,

    /// Whether tools with deferred exposure load their schemas on demand.
    #[serde(default = "default_enable_deferred_tool_loading")]
    pub enable_deferred_tool_loading: bool,

    /// Allows broad JSON repair for non-Write tool arguments only after a
    /// provider confirms a normal tool-use completion.
    #[serde(default = "default_true")]
    pub allow_tool_json_repair: bool,

    /// Debug-mode configuration (log path, language templates, etc.).
    #[serde(default)]
    pub debug_mode_config: DebugModeConfig,

    /// Allow Computer use (desktop automation) when the desktop host is available (all session modes).
    #[serde(default)]
    pub computer_use_enabled: bool,

    /// Preferred browser for CDP browser control. Empty/default uses the system default browser.
    #[serde(default)]
    pub browser_control_preferred_browser: String,

    /// Reattach to an already-running browser when BitFun starts. Off by
    /// default: the browser forgets its approval when it restarts, so this can
    /// put an approval dialog in front of the user before they asked for the
    /// browser at all.
    #[serde(default)]
    pub browser_control_auto_connect_on_startup: bool,

    /// Maximum number of rounds per dialog turn before soft-pausing.
    #[serde(default = "default_max_rounds")]
    pub max_rounds: usize,

    /// User-controllable master switch for the RBAC/Warden mechanism (R-26).
    ///
    /// When `false`, the RBAC tool-restriction checks and the Warden runtime
    /// (turn/tool failure tracking, violation records, reminders) are fully
    /// bypassed. Defaults to `true` (mechanism on). Users can turn it off in
    /// the settings document under `ai.rbac_enabled`.
    #[serde(default = "default_true")]
    pub rbac_enabled: bool,

    /// Master switch for loading external user instruction sources
    /// (`~/.claude/CLAUDE.md` + `rules/`, OpenCode `AGENTS.md`, Codex
    /// `AGENTS.md`) into the User Context.
    ///
    /// When `false`, the runtime does not read any external instruction file:
    /// workspace instruction files (`AGENTS.md` inside the project, project
    /// `.claude/rules`) are unaffected. Defaults to `true` (load), preserving
    /// the historical behavior. Users can turn it off in the settings document
    /// under `ai.external_instruction_sources`.
    #[serde(default = "default_true")]
    pub external_instruction_sources: bool,

    /// Root directory of the knowledge base used by the KnowledgeBaseSearch
    /// tool. When set, the desktop host injects it into the
    /// `BITFUN_KNOWLEDGE_BASE_ROOT` environment variable at startup so the
    /// tool can resolve it at call time (L6-P0-1). Empty/absent keeps the
    /// tool disabled with its fail-closed configuration error.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub knowledge_base_root: String,

    /// Maximum number of legion nodes in a single LegionControl topology
    /// (legion 阈值参数配置化：前端可配置，默认 20 保持现语义）。
    ///
    /// Replaces the hard-coded `MAX_LEGION_NODES = 20` in legion_control_tool.rs.
    /// `0` is not a meaningful value for a per-topology cap: the tool clamps it
    /// to `DEFAULT_LEGION_MAX_NODES` when unset (see legion_control_tool.rs).
    #[serde(default = "default_legion_max_nodes")]
    pub legion_max_nodes: usize,

    /// Maximum total number of legion node sessions a single creator may own
    /// across deployments (legion 阈值参数配置化：前端可配置，默认 60 保持现语义）。
    ///
    /// Replaces the hard-coded `MAX_LEGION_TOTAL_NODES = 3 * MAX_LEGION_NODES`
    /// in legion_control_tool.rs. A value below 1 is meaningless (it would
    /// reject every deployment) and falls back to the default.
    #[serde(default = "default_legion_max_total_nodes")]
    pub legion_max_total_nodes: usize,

    /// Maximum number of LegionControl `load` deployments allowed per creator
    /// session within a one-hour sliding window (legion 阈值参数配置化：前端
    /// 可配置，默认 10 次/小时）。
    ///
    /// The tool records a `legionDeployTime` timestamp on the creator session
    /// metadata after each successful load and rejects a new load when the
    /// window is exceeded. `0` (or unset) disables the frequency limit.
    #[serde(default = "default_legion_deploy_frequency_per_hour")]
    pub legion_deploy_frequency_per_hour: usize,

    /// Tunable AI behavior thresholds (阈值参数配置化统一入口).
    ///
    /// Every hard-coded user-visible threshold (compression budgets, retry
    /// backoffs, tool output caps, timeouts, ACP windows, Warden poke pacing,
    /// deep-review budgets, memory token limits, output-token tiers and goal
    /// continuations) is surfaced here under `ai.thresholds.<domain>.*`.
    /// Defaults reproduce the legacy hard-coded values exactly, so an
    /// unconfigured document behaves identically to before.
    #[serde(default)]
    pub thresholds: AiThresholdsConfig,
}

/// Tunable AI behavior thresholds, grouped by functional domain
/// (阈值参数配置化统一入口：`ai.thresholds.*`).
///
/// Every field carries a `#[serde(default = "...")]` mirror of the legacy
/// hard-coded constant so unconfigured documents preserve prior behavior.
/// Runtime consumers apply their own `clamp` on top (defense in depth), so a
/// maliciously extreme configured value still cannot exhaust resources.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AiThresholdsConfig {
    /// Subagent scheduling thresholds.
    #[serde(default)]
    pub subagent: SubagentThresholds,
    /// Context-compression token budgets and recovery counts.
    #[serde(default)]
    pub compression: CompressionThresholds,
    /// Model-stream retry attempts and exponential-backoff windows.
    #[serde(default)]
    pub model_retry: ModelRetryThresholds,
    /// Per-tool / per-round character caps for oversized tool results.
    #[serde(default)]
    pub tool_output_cap: ToolOutputCapThresholds,
    /// Default timeouts applied by tools that own their execution timeout.
    #[serde(default)]
    pub tool_timeout: ToolTimeoutThresholds,
    /// Knowledge-base search scan and result caps.
    #[serde(default)]
    pub knowledge_search: KnowledgeSearchThresholds,
    /// External ACP client timeouts.
    #[serde(default)]
    pub acp_timeout: AcpTimeoutThresholds,
    /// Warden challenge-poke pacing and judgement timeouts.
    #[serde(default)]
    pub warden: WardenThresholds,
    /// Deep-review execution budgets.
    #[serde(default)]
    pub deep_review: DeepReviewThresholds,
    /// Memory roll-out/transcript token limits not covered by `memories.*`.
    #[serde(default)]
    pub memories: MemoryThresholds,
    /// Automatic output-token tiering for model context windows.
    #[serde(default)]
    pub output_tokens: OutputTokensThresholds,
    /// Goal idle-wakeup and auto-continuation budgets.
    #[serde(default)]
    pub goal: GoalThresholds,
}

impl Default for AiThresholdsConfig {
    fn default() -> Self {
        Self {
            subagent: SubagentThresholds::default(),
            compression: CompressionThresholds::default(),
            model_retry: ModelRetryThresholds::default(),
            tool_output_cap: ToolOutputCapThresholds::default(),
            tool_timeout: ToolTimeoutThresholds::default(),
            knowledge_search: KnowledgeSearchThresholds::default(),
            acp_timeout: AcpTimeoutThresholds::default(),
            warden: WardenThresholds::default(),
            deep_review: DeepReviewThresholds::default(),
            memories: MemoryThresholds::default(),
            output_tokens: OutputTokensThresholds::default(),
            goal: GoalThresholds::default(),
        }
    }
}

/// Subagent scheduling thresholds (`ai.thresholds.subagent.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SubagentThresholds {
    /// Hard cap on subagent concurrency. Mirrors legacy `MAX_SUBAGENT_MAX_CONCURRENCY = 64`.
    #[serde(default = "default_subagent_max_hard_cap")]
    pub max_hard_cap: usize,
    /// Grace period (seconds) granted while awaiting subagent cancellation.
    #[serde(default = "default_subagent_timeout_grace_secs")]
    pub timeout_grace_secs: u64,
    /// Maximum session references a single message may carry.
    #[serde(default = "default_session_references_per_turn")]
    pub session_references_per_turn: usize,
    /// Sliding-window cap on the cumulative number of subagent deployments
    /// per parent session per window (`ai.thresholds.subagent.max_dispatch_per_parent_window`).
    ///
    /// The concurrency limiter only bounds *simultaneously running* subagents;
    /// a runaway dispatch loop can still create an unbounded cumulative fleet
    /// (observed: 865 executor subagents in 49 minutes, each burning a full
    /// first-round model request). This cumulative gate rejects new dispatches
    /// once the window cap is reached. `0` disables the limit.
    #[serde(default = "default_subagent_max_dispatch_per_parent_window")]
    pub max_dispatch_per_parent_window: usize,
    /// Sliding window length (seconds) for the cumulative dispatch cap.
    #[serde(default = "default_subagent_dispatch_window_secs")]
    pub dispatch_window_secs: u64,
    /// Cooldown (seconds) applied when the dispatch cap is hit: further
    /// dispatches from the same parent are rejected until the window rolls
    /// over. `0` disables the cooldown (rejection is instantaneous).
    #[serde(default = "default_subagent_dispatch_cooldown_secs")]
    pub dispatch_cooldown_secs: u64,
}

impl Default for SubagentThresholds {
    fn default() -> Self {
        Self {
            max_hard_cap: default_subagent_max_hard_cap(),
            timeout_grace_secs: default_subagent_timeout_grace_secs(),
            session_references_per_turn: default_session_references_per_turn(),
            max_dispatch_per_parent_window: default_subagent_max_dispatch_per_parent_window(),
            dispatch_window_secs: default_subagent_dispatch_window_secs(),
            dispatch_cooldown_secs: default_subagent_dispatch_cooldown_secs(),
        }
    }
}

fn default_subagent_max_hard_cap() -> usize {
    64
}

fn default_subagent_timeout_grace_secs() -> u64 {
    10
}

fn default_session_references_per_turn() -> usize {
    5
}

fn default_subagent_max_dispatch_per_parent_window() -> usize {
    20
}

fn default_subagent_dispatch_window_secs() -> u64 {
    3600
}

fn default_subagent_dispatch_cooldown_secs() -> u64 {
    300
}

/// Context-compression budgets and recovery counts (`ai.thresholds.compression.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CompressionThresholds {
    /// Automatic-compression safety reserve (tokens). Legacy `AUTO_COMPRESSION_SAFETY_RESERVE_TOKENS = 10_000`.
    #[serde(default = "default_compression_safety_reserve_tokens")]
    pub safety_reserve_tokens: usize,
    /// Max compression overflow retries. Legacy `MAX_COMPRESSION_OVERFLOW_ATTEMPTS = 4`.
    #[serde(default = "default_compression_overflow_attempts")]
    pub overflow_attempts: usize,
    /// Max main-context overflow recoveries. Legacy `MAX_MAIN_CONTEXT_OVERFLOW_RECOVERIES = 2`.
    #[serde(default = "default_compression_overflow_recoveries")]
    pub main_context_overflow_recoveries: usize,
    /// Max consecutive compression failures before giving up. Legacy `MAX_CONSECUTIVE_COMPRESSION_FAILURES = 3`.
    #[serde(default = "default_compression_consecutive_failures")]
    pub consecutive_failures: usize,
    /// Max failed-tool recovery attempts. Legacy `MAX_FAILED_TOOL_RECOVERY_ATTEMPTS = 3`.
    #[serde(default = "default_compression_failed_tool_recovery_attempts")]
    pub failed_tool_recovery_attempts: usize,
    /// Max stop-hook continuations per turn. Legacy `MAX_STOP_HOOK_CONTINUATIONS = 3`.
    #[serde(default = "default_compression_stop_hook_continuations")]
    pub stop_hook_continuations: usize,
    /// Max same-round compression passes. Legacy `MAX_SAME_ROUND_COMPRESSION_PASSES = 2`.
    #[serde(default = "default_compression_same_round_passes")]
    pub same_round_passes: usize,
    /// Max image-bearing messages whose images are kept for the API.
    /// Legacy `MAX_IMAGE_BEARING_MESSAGE_ROUNDS = 2`.
    #[serde(default = "default_compression_image_bearing_messages")]
    pub image_bearing_messages: usize,
    /// Recent-context tokens preserved by the compressor. Legacy `DEFAULT_RECENT_CONTEXT_TOKENS = 10_000`.
    #[serde(default = "default_compression_recent_context_tokens")]
    pub recent_context_tokens: usize,
    /// Retry step when a compression pass overflows. Legacy `RECENT_CONTEXT_RETRY_STEP_TOKENS = 10_000`.
    #[serde(default = "default_compression_retry_step_tokens")]
    pub retry_step_tokens: usize,
    /// Maximum retained user tokens. Legacy `MAX_RETAINED_USER_TOKENS = 20_000`.
    #[serde(default = "default_compression_max_retained_user_tokens")]
    pub max_retained_user_tokens: usize,
}

impl Default for CompressionThresholds {
    fn default() -> Self {
        Self {
            safety_reserve_tokens: default_compression_safety_reserve_tokens(),
            overflow_attempts: default_compression_overflow_attempts(),
            main_context_overflow_recoveries: default_compression_overflow_recoveries(),
            consecutive_failures: default_compression_consecutive_failures(),
            failed_tool_recovery_attempts: default_compression_failed_tool_recovery_attempts(),
            stop_hook_continuations: default_compression_stop_hook_continuations(),
            same_round_passes: default_compression_same_round_passes(),
            image_bearing_messages: default_compression_image_bearing_messages(),
            recent_context_tokens: default_compression_recent_context_tokens(),
            retry_step_tokens: default_compression_retry_step_tokens(),
            max_retained_user_tokens: default_compression_max_retained_user_tokens(),
        }
    }
}

fn default_compression_safety_reserve_tokens() -> usize {
    10_000
}

fn default_compression_overflow_attempts() -> usize {
    4
}

fn default_compression_overflow_recoveries() -> usize {
    2
}

fn default_compression_consecutive_failures() -> usize {
    3
}

fn default_compression_failed_tool_recovery_attempts() -> usize {
    3
}

fn default_compression_stop_hook_continuations() -> usize {
    3
}

fn default_compression_same_round_passes() -> usize {
    2
}

fn default_compression_image_bearing_messages() -> usize {
    2
}

fn default_compression_recent_context_tokens() -> usize {
    10_000
}

fn default_compression_retry_step_tokens() -> usize {
    10_000
}

fn default_compression_max_retained_user_tokens() -> usize {
    20_000
}

/// Model-stream retry backoff parameters (`ai.thresholds.model_retry.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelRetryThresholds {
    /// Max stream attempts. Legacy `MAX_STREAM_ATTEMPTS = 10`.
    #[serde(default = "default_model_retry_max_attempts")]
    pub max_attempts: usize,
    /// Base retry delay (ms). Legacy `RETRY_BASE_DELAY_MS = 500`.
    #[serde(default = "default_model_retry_base_delay_ms")]
    pub base_delay_ms: u64,
    /// Rate-limit retry base delay (ms). Legacy `RATE_LIMIT_RETRY_BASE_DELAY_MS = 2000`.
    #[serde(default = "default_model_retry_rate_limit_base_delay_ms")]
    pub rate_limit_base_delay_ms: u64,
    /// Exponential-delay cap (ms). Legacy `MAX_EXPONENTIAL_DELAY_MS = 30_000`.
    #[serde(default = "default_model_retry_max_exponential_delay_ms")]
    pub max_exponential_delay_ms: u64,
    /// Rate-limit delay cap (ms). Legacy `MAX_RATE_LIMIT_DELAY_MS = 60_000`.
    #[serde(default = "default_model_retry_max_rate_limit_delay_ms")]
    pub max_rate_limit_delay_ms: u64,
    /// Max retry exponent shift. Legacy `MAX_RETRY_EXPONENT_SHIFT = 6`.
    #[serde(default = "default_model_retry_max_exponent_shift")]
    pub max_exponent_shift: u32,
}

impl Default for ModelRetryThresholds {
    fn default() -> Self {
        Self {
            max_attempts: default_model_retry_max_attempts(),
            base_delay_ms: default_model_retry_base_delay_ms(),
            rate_limit_base_delay_ms: default_model_retry_rate_limit_base_delay_ms(),
            max_exponential_delay_ms: default_model_retry_max_exponential_delay_ms(),
            max_rate_limit_delay_ms: default_model_retry_max_rate_limit_delay_ms(),
            max_exponent_shift: default_model_retry_max_exponent_shift(),
        }
    }
}

fn default_model_retry_max_attempts() -> usize {
    10
}

fn default_model_retry_base_delay_ms() -> u64 {
    500
}

fn default_model_retry_rate_limit_base_delay_ms() -> u64 {
    2_000
}

fn default_model_retry_max_exponential_delay_ms() -> u64 {
    30_000
}

fn default_model_retry_max_rate_limit_delay_ms() -> u64 {
    60_000
}

fn default_model_retry_max_exponent_shift() -> u32 {
    6
}

/// Per-tool / per-round character caps for oversized tool results
/// (`ai.thresholds.tool_output_cap.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolOutputCapThresholds {
    /// Default per-tool result cap (chars). Legacy `DEFAULT_MAX_TOOL_RESULT_CHARS = 50_000`.
    #[serde(default = "default_tool_output_default_chars")]
    pub default_chars: usize,
    /// Per-round aggregate cap (chars). Legacy `MAX_TOOL_RESULTS_PER_ROUND_CHARS = 200_000`.
    #[serde(default = "default_tool_output_per_round_chars")]
    pub per_round_chars: usize,
    /// Persisted-output preview (chars). Legacy `TOOL_RESULT_PREVIEW_CHARS = 2_000`.
    #[serde(default = "default_tool_output_preview_chars")]
    pub preview_chars: usize,
    /// Read tool result cap (chars). Legacy `READ_MAX_TOOL_RESULT_CHARS = 72_000`.
    #[serde(default = "default_tool_output_read_chars")]
    pub read_chars: usize,
    /// Bash/shell result cap (chars). Legacy `SHELL_MAX_TOOL_RESULT_CHARS = 30_000`.
    #[serde(default = "default_tool_output_shell_chars")]
    pub shell_chars: usize,
}

impl Default for ToolOutputCapThresholds {
    fn default() -> Self {
        Self {
            default_chars: default_tool_output_default_chars(),
            per_round_chars: default_tool_output_per_round_chars(),
            preview_chars: default_tool_output_preview_chars(),
            read_chars: default_tool_output_read_chars(),
            shell_chars: default_tool_output_shell_chars(),
        }
    }
}

fn default_tool_output_default_chars() -> usize {
    50_000
}

fn default_tool_output_per_round_chars() -> usize {
    200_000
}

fn default_tool_output_preview_chars() -> usize {
    2_000
}

fn default_tool_output_read_chars() -> usize {
    72_000
}

fn default_tool_output_shell_chars() -> usize {
    30_000
}

/// Default timeouts (ms) for tools that own their execution timeout
/// (`ai.thresholds.tool_timeout.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolTimeoutThresholds {
    /// Bash tool default timeout (ms). Legacy `DEFAULT_TIMEOUT_MS = 120_000`.
    #[serde(default = "default_tool_timeout_bash_default_ms")]
    pub bash_default_ms: u64,
    /// Bash tool max timeout (ms). Legacy `MAX_TIMEOUT_MS = 600_000`.
    #[serde(default = "default_tool_timeout_bash_max_ms")]
    pub bash_max_ms: u64,
    /// ExecCommand default yield (ms). Legacy `EXEC_COMMAND_DEFAULT_YIELD_TIME_MS = 30_000`.
    #[serde(default = "default_tool_timeout_exec_command_yield_ms")]
    pub exec_command_yield_ms: u64,
    /// Remote shell probe timeout (ms). Legacy `REMOTE_EXEC_SHELL_PROBE_TIMEOUT_MS = 3_000`.
    #[serde(default = "default_tool_timeout_remote_shell_probe_ms")]
    pub remote_shell_probe_ms: u64,
    /// Document-conversion timeout (secs). Legacy `DOCUMENT_CONVERSION_TIMEOUT = 30`.
    #[serde(default = "default_tool_timeout_document_conversion_secs")]
    pub document_conversion_secs: u64,
    /// Web fetch timeout (secs). Legacy `WEB_FETCH_TIMEOUT_SECS = 30`.
    #[serde(default = "default_tool_timeout_web_fetch_secs")]
    pub web_fetch_secs: u64,
    /// Exa web-search timeout (secs). Legacy `EXA_TIMEOUT_SECS = 25`.
    #[serde(default = "default_tool_timeout_exa_secs")]
    pub exa_secs: u64,
    /// AgentWait default timeout (ms). Legacy `DEFAULT_TIMEOUT_MS = 600_000`.
    #[serde(default = "default_tool_timeout_agent_wait_default_ms")]
    pub agent_wait_default_ms: u64,
    /// AgentWait max timeout (ms). Legacy `MAX_TIMEOUT_MS = 3_600_000`.
    #[serde(default = "default_tool_timeout_agent_wait_max_ms")]
    pub agent_wait_max_ms: u64,
    /// MCP tool default render cap (chars). Legacy `DEFAULT_RENDER_CHAR_LIMIT = 32_000`.
    #[serde(default = "default_tool_timeout_mcp_render_chars")]
    pub mcp_render_chars: usize,
    /// GetFileDiff prepared diff page budget (chars). Legacy `PREPARED_REVIEW_DIFF_PAGE_CHARS = 40_000`.
    #[serde(default = "default_tool_timeout_diff_page_chars")]
    pub diff_page_chars: usize,
    /// GetFileDiff prepared diff total budget (chars). Legacy `PREPARED_REVIEW_DIFF_TOTAL_CHARS = 80_000`.
    #[serde(default = "default_tool_timeout_diff_total_chars")]
    pub diff_total_chars: usize,
    /// GetFileDiff new-file content limit (bytes). Legacy `REVIEW_NEW_FILE_CONTENT_LIMIT = 16 KiB`.
    #[serde(default = "default_tool_timeout_diff_new_file_bytes")]
    pub diff_new_file_bytes: u64,
}

impl Default for ToolTimeoutThresholds {
    fn default() -> Self {
        Self {
            bash_default_ms: default_tool_timeout_bash_default_ms(),
            bash_max_ms: default_tool_timeout_bash_max_ms(),
            exec_command_yield_ms: default_tool_timeout_exec_command_yield_ms(),
            remote_shell_probe_ms: default_tool_timeout_remote_shell_probe_ms(),
            document_conversion_secs: default_tool_timeout_document_conversion_secs(),
            web_fetch_secs: default_tool_timeout_web_fetch_secs(),
            exa_secs: default_tool_timeout_exa_secs(),
            agent_wait_default_ms: default_tool_timeout_agent_wait_default_ms(),
            agent_wait_max_ms: default_tool_timeout_agent_wait_max_ms(),
            mcp_render_chars: default_tool_timeout_mcp_render_chars(),
            diff_page_chars: default_tool_timeout_diff_page_chars(),
            diff_total_chars: default_tool_timeout_diff_total_chars(),
            diff_new_file_bytes: default_tool_timeout_diff_new_file_bytes(),
        }
    }
}

fn default_tool_timeout_bash_default_ms() -> u64 {
    120_000
}

fn default_tool_timeout_bash_max_ms() -> u64 {
    600_000
}

fn default_tool_timeout_exec_command_yield_ms() -> u64 {
    30_000
}

fn default_tool_timeout_remote_shell_probe_ms() -> u64 {
    3_000
}

fn default_tool_timeout_document_conversion_secs() -> u64 {
    30
}

fn default_tool_timeout_web_fetch_secs() -> u64 {
    30
}

fn default_tool_timeout_exa_secs() -> u64 {
    25
}

fn default_tool_timeout_agent_wait_default_ms() -> u64 {
    600_000
}

fn default_tool_timeout_agent_wait_max_ms() -> u64 {
    60 * 60 * 1_000
}

fn default_tool_timeout_mcp_render_chars() -> usize {
    32_000
}

fn default_tool_timeout_diff_page_chars() -> usize {
    40_000
}

fn default_tool_timeout_diff_total_chars() -> usize {
    80_000
}

fn default_tool_timeout_diff_new_file_bytes() -> u64 {
    16 * 1024
}

/// Knowledge-base search scan and result caps (`ai.thresholds.knowledge_search.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KnowledgeSearchThresholds {
    /// Max scanned file size (bytes). Legacy `MAX_SCAN_FILE_SIZE = 2 MiB`.
    #[serde(default = "default_knowledge_search_max_file_bytes")]
    pub max_scan_file_bytes: u64,
    /// Max directory scan depth. Legacy `MAX_SCAN_DEPTH = 16`.
    #[serde(default = "default_knowledge_search_max_depth")]
    pub max_scan_depth: usize,
    /// Default result cap. Legacy `DEFAULT_MAX_RESULTS = 50`.
    #[serde(default = "default_knowledge_search_default_max_results")]
    pub default_max_results: usize,
    /// Hard cap for `max_results`. Legacy `MAX_RESULTS_CAP = 200`.
    #[serde(default = "default_knowledge_search_max_results_cap")]
    pub max_results_cap: usize,
}

impl Default for KnowledgeSearchThresholds {
    fn default() -> Self {
        Self {
            max_scan_file_bytes: default_knowledge_search_max_file_bytes(),
            max_scan_depth: default_knowledge_search_max_depth(),
            default_max_results: default_knowledge_search_default_max_results(),
            max_results_cap: default_knowledge_search_max_results_cap(),
        }
    }
}

fn default_knowledge_search_max_file_bytes() -> u64 {
    2 * 1024 * 1024
}

fn default_knowledge_search_max_depth() -> usize {
    16
}

fn default_knowledge_search_default_max_results() -> usize {
    50
}

fn default_knowledge_search_max_results_cap() -> usize {
    200
}

/// External ACP client timeouts (`ai.thresholds.acp_timeout.*`, seconds).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AcpTimeoutThresholds {
    /// Client startup timeout (secs). Legacy `CLIENT_STARTUP_TIMEOUT_SECS = 60`.
    #[serde(default = "default_acp_client_startup_secs")]
    pub client_startup_secs: u64,
    /// Permission request timeout (secs). Legacy `PERMISSION_TIMEOUT = 600`.
    #[serde(default = "default_acp_permission_secs")]
    pub permission_secs: u64,
    /// Session-close timeout (secs). Legacy `SESSION_CLOSE_TIMEOUT = 5`.
    #[serde(default = "default_acp_session_close_secs")]
    pub session_close_secs: u64,
    /// CLI detect probe timeout (secs). Legacy `CLI_DETECT_TIMEOUT_SECS = 5`.
    #[serde(default = "default_acp_cli_detect_secs")]
    pub cli_detect_secs: u64,
    /// ACP handshake timeout (secs). Legacy `ACP_HANDSHAKE_TIMEOUT_SECS = 30`.
    #[serde(default = "default_acp_handshake_secs")]
    pub handshake_secs: u64,
    /// Total try-connect probe timeout (secs). Legacy `TRY_CONNECT_TOTAL_TIMEOUT_SECS = 35`.
    #[serde(default = "default_acp_try_connect_total_secs")]
    pub try_connect_total_secs: u64,
    /// Requirement probe timeout (secs). Legacy `REQUIREMENT_PROBE_TIMEOUT = 3`.
    #[serde(default = "default_acp_requirement_probe_secs")]
    pub requirement_probe_secs: u64,
    /// Adapter download timeout (secs). Legacy `ADAPTER_DOWNLOAD_TIMEOUT = 120`.
    #[serde(default = "default_acp_adapter_download_secs")]
    pub adapter_download_secs: u64,
    /// CLI install timeout (secs). Legacy `CLI_INSTALL_TIMEOUT = 600`.
    #[serde(default = "default_acp_cli_install_secs")]
    pub cli_install_secs: u64,
    /// Background ACP direct delivery window (secs). Legacy `ACP_DIRECT_TIMEOUT_SECONDS = 1800`.
    #[serde(default = "default_acp_direct_secs")]
    pub direct_secs: u64,
    /// ACP Task-tool bounded window (secs). Legacy `ACP_TASK_TIMEOUT_SECONDS = 600`.
    #[serde(default = "default_acp_task_secs")]
    pub task_secs: u64,
}

impl Default for AcpTimeoutThresholds {
    fn default() -> Self {
        Self {
            client_startup_secs: default_acp_client_startup_secs(),
            permission_secs: default_acp_permission_secs(),
            session_close_secs: default_acp_session_close_secs(),
            cli_detect_secs: default_acp_cli_detect_secs(),
            handshake_secs: default_acp_handshake_secs(),
            try_connect_total_secs: default_acp_try_connect_total_secs(),
            requirement_probe_secs: default_acp_requirement_probe_secs(),
            adapter_download_secs: default_acp_adapter_download_secs(),
            cli_install_secs: default_acp_cli_install_secs(),
            direct_secs: default_acp_direct_secs(),
            task_secs: default_acp_task_secs(),
        }
    }
}

fn default_acp_client_startup_secs() -> u64 {
    60
}

fn default_acp_permission_secs() -> u64 {
    600
}

fn default_acp_session_close_secs() -> u64 {
    5
}

fn default_acp_cli_detect_secs() -> u64 {
    5
}

fn default_acp_handshake_secs() -> u64 {
    30
}

fn default_acp_try_connect_total_secs() -> u64 {
    35
}

fn default_acp_requirement_probe_secs() -> u64 {
    3
}

fn default_acp_adapter_download_secs() -> u64 {
    120
}

fn default_acp_cli_install_secs() -> u64 {
    600
}

fn default_acp_direct_secs() -> u64 {
    1800
}

fn default_acp_task_secs() -> u64 {
    600
}

/// Warden challenge-poke pacing and judgement timeouts
/// (`ai.thresholds.warden.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WardenThresholds {
    /// Max consecutive deferrals before a forced work turn. Legacy `MAX_DEFER_COUNT = 3`.
    #[serde(default = "default_warden_max_defer_count")]
    pub max_defer_count: u32,
    /// Max accepted average inter-poke interval rate. Legacy `MAX_RATE = 1000.0`.
    #[serde(default = "default_warden_max_rate")]
    pub max_rate: f64,
    /// Warden model-judgement timeout (secs). Legacy `WARDEN_JUDGEMENT_TIMEOUT = 8`.
    #[serde(default = "default_warden_judgement_timeout_secs")]
    pub judgement_timeout_secs: u64,
}

impl Default for WardenThresholds {
    fn default() -> Self {
        Self {
            max_defer_count: default_warden_max_defer_count(),
            max_rate: default_warden_max_rate(),
            judgement_timeout_secs: default_warden_judgement_timeout_secs(),
        }
    }
}

fn default_warden_max_defer_count() -> u32 {
    3
}

fn default_warden_max_rate() -> f64 {
    1000.0
}

fn default_warden_judgement_timeout_secs() -> u64 {
    8
}

/// Deep-review execution budgets (`ai.thresholds.deep_review.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DeepReviewThresholds {
    /// Per-review-turn diff budget (chars). Legacy `REVIEW_DIFF_MAX_CHARS_PER_TURN = 240_000`.
    #[serde(default = "default_deep_review_diff_max_chars_per_turn")]
    pub diff_max_chars_per_turn: usize,
    /// Max provider-diff acquisitions per turn. Legacy `REVIEW_PROVIDER_DIFF_MAX_ACQUISITIONS_PER_TURN = 128`.
    #[serde(default = "default_deep_review_diff_max_acquisitions_per_turn")]
    pub diff_max_acquisitions_per_turn: usize,
    /// Default max parallel reviewer instances. Legacy `DEFAULT_MAX_PARALLEL_INSTANCES = 4`.
    #[serde(default = "default_deep_review_max_parallel_instances")]
    pub max_parallel_instances: usize,
    /// Max queue wait before a reviewer launch is skipped (secs). Legacy `DEFAULT_MAX_QUEUE_WAIT_SECONDS = 1200`.
    #[serde(default = "default_deep_review_max_queue_wait_secs")]
    pub max_queue_wait_secs: u64,
    /// Auto-retry elapsed guard (secs). Legacy `DEFAULT_AUTO_RETRY_ELAPSED_GUARD_SECONDS = 180`.
    #[serde(default = "default_deep_review_auto_retry_elapsed_guard_secs")]
    pub auto_retry_elapsed_guard_secs: u64,
}

impl Default for DeepReviewThresholds {
    fn default() -> Self {
        Self {
            diff_max_chars_per_turn: default_deep_review_diff_max_chars_per_turn(),
            diff_max_acquisitions_per_turn: default_deep_review_diff_max_acquisitions_per_turn(),
            max_parallel_instances: default_deep_review_max_parallel_instances(),
            max_queue_wait_secs: default_deep_review_max_queue_wait_secs(),
            auto_retry_elapsed_guard_secs: default_deep_review_auto_retry_elapsed_guard_secs(),
        }
    }
}

fn default_deep_review_diff_max_chars_per_turn() -> usize {
    240_000
}

fn default_deep_review_diff_max_acquisitions_per_turn() -> usize {
    128
}

fn default_deep_review_max_parallel_instances() -> usize {
    4
}

fn default_deep_review_max_queue_wait_secs() -> u64 {
    1200
}

fn default_deep_review_auto_retry_elapsed_guard_secs() -> u64 {
    180
}

/// Memory token limits not covered by `memories.*` (`ai.thresholds.memories.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryThresholds {
    /// Memory summary token limit. Legacy `MEMORY_SUMMARY_TOKEN_LIMIT = 2_500`.
    #[serde(default = "default_memory_summary_token_limit")]
    pub summary_token_limit: usize,
    /// Transcript user-message token limit. Legacy `MESSAGE_CONTENT_TOKEN_LIMIT = 8_000`.
    #[serde(default = "default_memory_message_content_token_limit")]
    pub message_content_token_limit: usize,
    /// Transcript tool-input token limit. Legacy `TOOL_INPUT_TOKEN_LIMIT = 6_000`.
    #[serde(default = "default_memory_tool_input_token_limit")]
    pub tool_input_token_limit: usize,
    /// Transcript tool-result token limit. Legacy `TOOL_RESULT_TOKEN_LIMIT = 12_000`.
    #[serde(default = "default_memory_tool_result_token_limit")]
    pub tool_result_token_limit: usize,
    /// Transcript tool-error token limit. Legacy `TOOL_ERROR_TOKEN_LIMIT = 1_000`.
    #[serde(default = "default_memory_tool_error_token_limit")]
    pub tool_error_token_limit: usize,
    /// Phase-1 rollout token limit. Legacy `DEFAULT_ROLLOUT_TOKEN_LIMIT = 120_000`.
    #[serde(default = "default_memory_rollout_token_limit")]
    pub rollout_token_limit: usize,
}

impl Default for MemoryThresholds {
    fn default() -> Self {
        Self {
            summary_token_limit: default_memory_summary_token_limit(),
            message_content_token_limit: default_memory_message_content_token_limit(),
            tool_input_token_limit: default_memory_tool_input_token_limit(),
            tool_result_token_limit: default_memory_tool_result_token_limit(),
            tool_error_token_limit: default_memory_tool_error_token_limit(),
            rollout_token_limit: default_memory_rollout_token_limit(),
        }
    }
}

fn default_memory_summary_token_limit() -> usize {
    2_500
}

fn default_memory_message_content_token_limit() -> usize {
    8_000
}

fn default_memory_tool_input_token_limit() -> usize {
    6_000
}

fn default_memory_tool_result_token_limit() -> usize {
    12_000
}

fn default_memory_tool_error_token_limit() -> usize {
    1_000
}

fn default_memory_rollout_token_limit() -> usize {
    120_000
}

/// Automatic output-token tiering (`ai.thresholds.output_tokens.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputTokensThresholds {
    /// Automatic output-token tiers (largest tier first). Legacy `AUTOMATIC_MAX_OUTPUT_TOKEN_TIERS = [8k,16k,24k,32k,64k]`.
    #[serde(default = "default_output_token_tiers")]
    pub automatic_tiers: Vec<u32>,
    /// Max configured output-token ratio (percent of context window). Legacy `MAX_CONFIGURED_OUTPUT_TOKENS_RATIO_PERCENT = 40`.
    #[serde(default = "default_output_tokens_ratio_percent")]
    pub ratio_percent: u32,
}

impl Default for OutputTokensThresholds {
    fn default() -> Self {
        Self {
            automatic_tiers: default_output_token_tiers(),
            ratio_percent: default_output_tokens_ratio_percent(),
        }
    }
}

fn default_output_token_tiers() -> Vec<u32> {
    vec![8_000, 16_000, 24_000, 32_000, 64_000]
}

fn default_output_tokens_ratio_percent() -> u32 {
    40
}

/// Goal idle-wakeup and auto-continuation budgets (`ai.thresholds.goal.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GoalThresholds {
    /// Goal idle-wakeup delay (ms). Legacy `GOAL_IDLE_WAKEUP_DELAY_MS = 600_000`.
    #[serde(default = "default_goal_idle_wakeup_delay_ms")]
    pub idle_wakeup_delay_ms: u64,
    /// Max automatic goal continuations. Legacy `MAX_THREAD_GOAL_AUTO_CONTINUATIONS = 10`.
    #[serde(default = "default_goal_max_auto_continuations")]
    pub max_auto_continuations: u32,
}

impl Default for GoalThresholds {
    fn default() -> Self {
        Self {
            idle_wakeup_delay_ms: default_goal_idle_wakeup_delay_ms(),
            max_auto_continuations: default_goal_max_auto_continuations(),
        }
    }
}

fn default_goal_idle_wakeup_delay_ms() -> u64 {
    600_000
}

fn default_goal_max_auto_continuations() -> u32 {
    10
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SubagentBatchExecutionPolicy {
    /// Preserve the tool-owned concurrency-safety decision.
    SafeOnly,
    /// Force multiple Task calls from the same model batch into parallel scheduling.
    #[default]
    ForceParallel,
    /// Treat all Task calls as serial even when a subagent is read-only.
    Serial,
}

/// Automatic memory subsystem configuration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum MemoryExternalContextPolicy {
    /// Keep sessions that used external context tools, but clear those tool results in Phase 1.
    #[default]
    ClearToolResults,
    /// Keep sessions and tool results as-is.
    Allow,
    /// Mark sessions that used external context tools as polluted and skip extraction.
    SkipSession,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct MemoriesConfig {
    /// Enables automatic Phase 1 extraction and Phase 2 consolidation.
    pub generate_memories: bool,
    /// Allows persistent BTW sessions to become memory-generation sources.
    pub generate_for_btw_sessions: bool,
    /// Enables prompt injection of the consolidated memory summary.
    pub use_memories: bool,
    /// Controls how sessions that used external context tools are handled.
    pub external_context_policy: MemoryExternalContextPolicy,
    /// Maximum number of stage-1 outputs selected for phase-2 consolidation.
    pub max_raw_memories_for_consolidation: usize,
    /// Maximum age in days for a stage-1 output to stay eligible for phase-2 reuse.
    pub max_unused_days: i64,
    /// Maximum age in days for a source session to be considered by Phase 1.
    pub max_rollout_age_days: i64,
    /// Maximum source sessions claimed for extraction per memory startup pass.
    pub max_rollouts_per_startup: usize,
    /// Maximum source sessions scanned while looking for extraction candidates per memory startup pass.
    pub max_rollouts_scan_limit: usize,
    /// Minimum idle time in hours before a source session can be extracted.
    pub min_rollout_idle_hours: i64,
    /// Maximum number of concurrent Phase 1 extraction jobs.
    pub phase1_max_concurrency: usize,
    /// Retry backoff after a failed Phase 1 extraction.
    pub phase1_retry_backoff_minutes: i64,
    /// Lease duration for claimed Phase 1 jobs.
    pub phase1_lease_seconds: i64,
    /// Lease duration for the global Phase 2 consolidation job.
    pub phase2_lease_seconds: i64,
    /// Phase-2 consolidation cooldown in seconds after a successful run.
    pub phase2_success_cooldown_seconds: i64,
    /// Phase-2 retry delay in seconds after a failed run.
    pub phase2_retry_delay_seconds: i64,
    /// Optional model selector for Phase 1 extraction.
    pub extract_model: Option<String>,
    /// Optional model selector for Phase 2 consolidation.
    pub consolidation_model: Option<String>,
}

impl AIConfig {
    /// Resolves a canonical configured model ID.
    ///
    /// Returns the model id only when the matched model is `enabled`. This is the
    /// single source of truth for "is this model usable right now?" and is the
    /// variant every runtime path (client factory, execution engine, etc.) should
    /// use. UI / migration code that needs to look up disabled entries should call
    /// [`Self::resolve_model_reference_any`] instead.
    pub fn resolve_model_reference(&self, model_id: &str) -> Option<String> {
        let mut matches = self.models.iter().filter(|m| m.enabled && m.id == model_id);
        let model = matches.next()?;
        (matches.next().is_none()).then(|| model.id.clone())
    }

    /// Resolves a canonical configured model ID regardless of `enabled` state.
    /// UI / migration only — never use this on the runtime model-selection path.
    pub fn resolve_model_reference_any(&self, model_id: &str) -> Option<String> {
        let mut matches = self.models.iter().filter(|m| m.id == model_id);
        let model = matches.next()?;
        (matches.next().is_none()).then(|| model.id.clone())
    }

    /// Returns true if the given reference points to a model that exists and is
    /// currently enabled.
    pub fn is_model_reference_active(&self, model_ref: &str) -> bool {
        self.resolve_model_reference(model_ref).is_some()
    }

    /// Returns the id of the first enabled model, if any. Used as a final
    /// fallback when a configured default points to a disabled / missing model.
    pub fn first_enabled_model_id(&self) -> Option<String> {
        self.models.iter().find(|m| m.enabled).map(|m| m.id.clone())
    }

    /// Resolves a model selector value.
    ///
    /// Special values:
    /// - `primary`: must resolve to a valid (enabled) primary model
    /// - `fast`: first tries the configured fast model, then falls back to primary
    ///
    /// Regular values must be canonical configured model IDs. All lookups require
    /// the target model to be enabled — disabled models are treated as if they did
    /// not exist.
    pub fn resolve_model_selection(&self, model_ref: &str) -> Option<String> {
        match model_ref {
            "primary" => self
                .default_models
                .primary
                .as_deref()
                .and_then(|value| self.resolve_model_reference(value)),
            "fast" => self
                .default_models
                .fast
                .as_deref()
                .and_then(|value| self.resolve_model_reference(value))
                .or_else(|| {
                    self.default_models
                        .primary
                        .as_deref()
                        .and_then(|value| self.resolve_model_reference(value))
                }),
            _ => self.resolve_model_reference(model_ref),
        }
    }
}

/// Shared agent-profile configuration.
///
/// Tool and skill configuration shared by compatible mode profiles.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AgentProfileConfig {
    /// Shared profile ID (e.g. agentic, coding_shared, requirement, ui-design).
    pub profile_id: String,

    /// Tools explicitly enabled by the user that are not part of the mode defaults.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added_tools: Vec<String>,

    /// Default tools explicitly disabled by the user.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_tools: Vec<String>,

    /// User-level skills disabled for this mode.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub disabled_user_skills: Vec<String>,

    /// User-level built-in skills explicitly enabled even though the mode default disables them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enabled_user_skills: Vec<String>,

    /// User-level subagent availability overrides for this shared profile.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub subagent_overrides: ParentSubagentOverrideConfig,

    /// Agent-level permission rules applied after project rules.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_permission_rules: Vec<PermissionRule>,
}

/// User-level Skill configuration shared by every agent profile.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SkillSettingsConfig {
    /// User-level Skill keys disabled for every agent profile.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub globally_disabled_user_skills: Vec<String>,
}

/// API view of a mode configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(default)]
pub struct AgentProfileView {
    pub profile_id: String,
    pub enabled_tools: Vec<String>,
    pub default_tools: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub disabled_user_skills: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enabled_user_skills: Vec<String>,
}

fn default_true() -> bool {
    true
}

/// Default streaming idle timeout between chunks.
fn default_stream_idle_timeout() -> Option<u64> {
    Some(600)
}

/// Default timeout while waiting for the first effective streamed output.
fn default_stream_ttft_timeout() -> Option<u64> {
    Some(600)
}

/// Default is no timeout (wait forever).
fn default_tool_execution_timeout() -> Option<u64> {
    None
}

fn default_enable_deferred_tool_loading() -> bool {
    true
}

fn default_subagent_max_concurrency() -> usize {
    5
}

fn default_memory_max_raw_memories_for_consolidation() -> usize {
    64
}

fn default_memory_max_unused_days() -> i64 {
    30
}

fn default_memory_max_rollout_age_days() -> i64 {
    10
}

fn default_memory_max_rollouts_per_startup() -> usize {
    2
}

fn default_memory_max_rollouts_scan_limit() -> usize {
    2_000
}

fn default_memory_min_rollout_idle_hours() -> i64 {
    6
}

fn default_memory_phase1_max_concurrency() -> usize {
    1
}

fn default_memory_phase1_retry_backoff_minutes() -> i64 {
    60
}

fn default_memory_phase1_lease_seconds() -> i64 {
    60 * 60
}

fn default_memory_phase2_lease_seconds() -> i64 {
    60 * 60
}

fn default_memory_phase2_success_cooldown_seconds() -> i64 {
    6 * 60 * 60
}

fn default_memory_phase2_retry_delay_seconds() -> i64 {
    60 * 60
}

fn default_subagent_batch_execution_policy() -> SubagentBatchExecutionPolicy {
    SubagentBatchExecutionPolicy::ForceParallel
}

/// Default single-topology legion node cap (legion 阈值参数配置化）。
///
/// Keeps the legacy hard-coded `MAX_LEGION_NODES = 20` semantics when the user
/// does not configure `ai.legion_max_nodes`.
pub fn default_legion_max_nodes() -> usize {
    20
}

/// Default cross-deployment legion node cap (legion 阈值参数配置化）。
///
/// Keeps the legacy hard-coded `MAX_LEGION_TOTAL_NODES = 3 * 20 = 60` semantics
/// when the user does not configure `ai.legion_max_total_nodes`.
pub fn default_legion_max_total_nodes() -> usize {
    3 * default_legion_max_nodes()
}

/// Default legion deployment frequency cap: 10 loads per hour per creator
/// (legion 阈值参数配置化）。`0` disables the limit.
pub fn default_legion_deploy_frequency_per_hour() -> usize {
    10
}

pub const DEFAULT_MAX_ROUNDS: usize = 200;

fn default_max_rounds() -> usize {
    DEFAULT_MAX_ROUNDS
}

/// Debug-mode configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DebugModeConfig {
    /// Custom log path (relative to the workspace; default: `.bitfun/debug.log`).
    pub log_path: String,

    /// Ingest server port.
    pub ingest_port: u16,

    /// Enabled languages (auto-detected based on project type when empty).
    pub enabled_languages: Vec<String>,

    /// Debug template configuration per language.
    pub language_templates: HashMap<String, LanguageDebugTemplate>,
}

impl Default for DebugModeConfig {
    fn default() -> Self {
        Self {
            log_path: ".bitfun/debug.log".to_string(),
            ingest_port: 7242,
            enabled_languages: Vec::new(),
            language_templates: Self::default_language_templates(),
        }
    }
}

impl DebugModeConfig {
    /// Returns the default language templates.
    ///
    /// Core languages (JavaScript) are enabled by default and cannot be disabled;
    /// they are included in the static prompt.
    /// Other languages (Python/Rust/Go/Java) are disabled by default and can be enabled as needed.
    pub fn default_language_templates() -> HashMap<String, LanguageDebugTemplate> {
        let mut templates = HashMap::new();

        templates.insert("javascript".to_string(), LanguageDebugTemplate {
            language: "javascript".to_string(),
            display_name: "JavaScript / TypeScript".to_string(),
            enabled: false,
            instrumentation_template: r#"fetch('http://127.0.0.1:{PORT}/ingest/{SESSION_ID}',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({location:'{LOCATION}',message:'{MESSAGE}',data:{DATA},timestamp:Date.now(),sessionId:'{SESSION_ID}',hypothesisId:'{HYPOTHESIS_ID}',runId:'{RUN_ID}'})}).catch(()=>{});"#.to_string(),
            region_start: "// #region agent log".to_string(),
            region_end: "// #endregion".to_string(),
            notes: vec![
                "Send logs to the ingest server via HTTP POST.".to_string(),
                "{DATA} must be replaced with a JavaScript object expression.".to_string(),
            ],
        });

        templates.insert("python".to_string(), LanguageDebugTemplate {
            language: "python".to_string(),
            display_name: "Python".to_string(),
            enabled: false,
            instrumentation_template: r#"import json, time, os
with open(os.path.join(os.getcwd(), '{LOG_PATH}'), 'a', encoding='utf-8') as _f:
    _f.write(json.dumps({"location": "{LOCATION}", "message": "{MESSAGE}", "data": {DATA}, "timestamp": int(time.time()*1000), "sessionId": "{SESSION_ID}", "hypothesisId": "{HYPOTHESIS_ID}", "runId": "{RUN_ID}"}, ensure_ascii=False) + '\n')"#.to_string(),
            region_start: "# region agent log".to_string(),
            region_end: "# endregion".to_string(),
            notes: vec![
                "Append NDJSON logs directly to workspace LOG_PATH.".to_string(),
                "Use ensure_ascii=False to preserve non-ASCII characters.".to_string(),
                "{DATA} must be a Python expression (e.g., {\"var\": var} or locals()).".to_string(),
                "Imports only need to be declared once at the top.".to_string(),
            ],
        });

        templates.insert("rust".to_string(), LanguageDebugTemplate {
            language: "rust".to_string(),
            display_name: "Rust".to_string(),
            enabled: false,
            instrumentation_template: r##"{
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};
    if let Ok(mut _f) = OpenOptions::new().create(true).append(true).open("{LOG_PATH}") {
        let _ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        let _ = writeln!(_f, r#"{{"location":"{LOCATION}","message":"{MESSAGE}","data":{},"timestamp":{},"sessionId":"{SESSION_ID}","hypothesisId":"{HYPOTHESIS_ID}","runId":"{RUN_ID}"}}"#, serde_json::json!({DATA}), _ts);
    }
}"##.to_string(),
            region_start: "// #region agent log".to_string(),
            region_end: "// #endregion".to_string(),
            notes: vec![
                "Append NDJSON logs directly to LOG_PATH.".to_string(),
                "Requires serde_json: cargo add serde_json.".to_string(),
                "{DATA} must be a Rust expression (e.g., {\"var\": var}).".to_string(),
                "Use in sync code; for async code use tokio::fs.".to_string(),
            ],
        });

        templates.insert("go".to_string(), LanguageDebugTemplate {
            language: "go".to_string(),
            display_name: "Go".to_string(),
            enabled: false,
            instrumentation_template: r#"func() {
	f, err := os.OpenFile("{LOG_PATH}", os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0644)
	if err == nil {
		defer f.Close()
		data, _ := json.Marshal(map[string]interface{}{"location": "{LOCATION}", "message": "{MESSAGE}", "data": {DATA}, "timestamp": time.Now().UnixMilli(), "sessionId": "{SESSION_ID}", "hypothesisId": "{HYPOTHESIS_ID}", "runId": "{RUN_ID}"})
		f.Write(append(data, '\n'))
	}
}()"#.to_string(),
            region_start: "// #region agent log".to_string(),
            region_end: "// #endregion".to_string(),
            notes: vec![
                "Use an immediately-invoked anonymous function; can be inserted anywhere.".to_string(),
                "Append NDJSON logs directly to LOG_PATH.".to_string(),
                "Import \"os\", \"encoding/json\", and \"time\".".to_string(),
                "{DATA} must be a Go expression (e.g., map[string]interface{}{\"var\": var}).".to_string(),
            ],
        });

        templates.insert("java".to_string(), LanguageDebugTemplate {
            language: "java".to_string(),
            display_name: "Java".to_string(),
            enabled: false,
            instrumentation_template: r#"try {
    java.nio.file.Files.writeString(
        java.nio.file.Path.of("{LOG_PATH}"),
        String.format("{\"location\":\"{LOCATION}\",\"message\":\"{MESSAGE}\",\"data\":%s,\"timestamp\":%d,\"sessionId\":\"{SESSION_ID}\",\"hypothesisId\":\"{HYPOTHESIS_ID}\",\"runId\":\"{RUN_ID}\"}%n",
            new com.google.gson.Gson().toJson({DATA}), System.currentTimeMillis()),
        java.nio.file.StandardOpenOption.CREATE, java.nio.file.StandardOpenOption.APPEND);
} catch (Exception _e) { /* debug log */ }"#.to_string(),
            region_start: "// #region agent log".to_string(),
            region_end: "// #endregion".to_string(),
            notes: vec![
                "Append NDJSON logs directly to LOG_PATH.".to_string(),
                "Requires Gson (or use Jackson).".to_string(),
                "{DATA} must be a Java object (e.g., Map.of(\"var\", var)).".to_string(),
                "Java 11+ can use Files.writeString; older versions use Files.write + getBytes().".to_string(),
            ],
        });

        templates
    }

    /// Returns relevant templates based on detected project languages.
    pub fn get_templates_for_languages(
        &self,
        detected_languages: &[String],
    ) -> Vec<&LanguageDebugTemplate> {
        let target_languages: Vec<&str> = if !self.enabled_languages.is_empty() {
            self.enabled_languages.iter().map(|s| s.as_str()).collect()
        } else {
            detected_languages.iter().map(|s| s.as_str()).collect()
        };

        let language_mapping: HashMap<&str, &str> = [
            ("typescript", "javascript"),
            ("javascript", "javascript"),
            ("python", "python"),
            ("rust", "rust"),
            ("go", "go"),
            ("java", "java"),
            ("kotlin", "java"),
        ]
        .into_iter()
        .collect();

        let mut result = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for lang in &target_languages {
            let template_lang = language_mapping.get(lang).unwrap_or(lang);
            if !seen.contains(template_lang) {
                if let Some(template) = self.language_templates.get(*template_lang) {
                    if template.enabled {
                        result.push(template);
                        seen.insert(template_lang);
                    }
                }
            }
        }

        result
    }
}

/// Language debug template.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LanguageDebugTemplate {
    /// Language identifier (javascript, python, rust, go, java).
    pub language: String,

    /// Display name.
    pub display_name: String,

    /// Whether this language template is enabled (when enabled, user-defined templates override
    /// built-in logic).
    pub enabled: bool,

    /// Instrumentation code template.
    /// Placeholders: {LOCATION}, {MESSAGE}, {DATA}, {PORT}, {SESSION_ID}, {HYPOTHESIS_ID},
    /// {RUN_ID}, {LOG_PATH}
    pub instrumentation_template: String,

    /// Region marker start.
    pub region_start: String,

    /// Region marker end.
    pub region_end: String,

    /// Special notes.
    pub notes: Vec<String>,
}

impl Default for LanguageDebugTemplate {
    fn default() -> Self {
        Self {
            language: String::new(),
            display_name: String::new(),
            enabled: false,
            instrumentation_template: String::new(),
            region_start: "// #region agent log".to_string(),
            region_end: "// #endregion".to_string(),
            notes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentSubagentOverrideState {
    Enabled,
    Disabled,
}

pub type ParentSubagentOverrideConfig = HashMap<String, AgentSubagentOverrideState>;
pub type AgentSubagentOverrideConfig = HashMap<String, ParentSubagentOverrideConfig>;

pub const DEFAULT_MODEL_CONTEXT_WINDOW_TOKENS: u32 = 1_048_576;
pub const MIN_MODEL_CONTEXT_WINDOW_TOKENS: u32 = 32_000;
pub const MAX_CONFIGURED_OUTPUT_TOKENS_RATIO_PERCENT: u32 = 40;
const AUTOMATIC_MAX_OUTPUT_TOKEN_TIERS: [u32; 5] = [8_000, 16_000, 24_000, 32_000, 64_000];

/// Chooses the largest supported output tier that does not exceed one quarter
/// of the model context window.
pub fn automatic_max_output_tokens(context_window: u32) -> u32 {
    let quarter_context = context_window / 4;
    AUTOMATIC_MAX_OUTPUT_TOKEN_TIERS
        .iter()
        .rev()
        .copied()
        .find(|tier| *tier <= quarter_context)
        .unwrap_or(quarter_context)
}

/// Same as [`automatic_max_output_tokens`] but honoring the configured tiers
/// (阈值参数配置化：`ai.thresholds.output_tokens.automatic_tiers`).
pub async fn automatic_max_output_tokens_configured(context_window: u32) -> u32 {
    let tiers = configured_output_token_tiers().await;
    let quarter_context = context_window / 4;
    tiers
        .iter()
        .rev()
        .copied()
        .find(|tier| *tier <= quarter_context)
        .unwrap_or(quarter_context)
}

/// Resolve the configured output-token tiers
/// (`ai.thresholds.output_tokens.automatic_tiers`), falling back to the legacy
/// `AUTOMATIC_MAX_OUTPUT_TOKEN_TIERS` when unset or empty.
async fn configured_output_token_tiers() -> Vec<u32> {
    let Ok(config_service) = crate::service::config::get_global_config_service().await else {
        return AUTOMATIC_MAX_OUTPUT_TOKEN_TIERS.to_vec();
    };
    let Ok(thresholds) = config_service
        .get_config::<AiThresholdsConfig>(Some("ai.thresholds"))
        .await
    else {
        return AUTOMATIC_MAX_OUTPUT_TOKEN_TIERS.to_vec();
    };
    let tiers = &thresholds.output_tokens.automatic_tiers;
    if tiers.is_empty() {
        return AUTOMATIC_MAX_OUTPUT_TOKEN_TIERS.to_vec();
    }
    tiers.clone()
}

/// A configured output cap may use up to 40% of the model context window.
pub fn is_valid_configured_max_output_tokens(context_window: u32, max_tokens: u32) -> bool {
    max_tokens > 0
        && u64::from(max_tokens) * 100
            <= u64::from(context_window) * u64::from(MAX_CONFIGURED_OUTPUT_TOKENS_RATIO_PERCENT)
}

/// Same as [`is_valid_configured_max_output_tokens`] but honoring the
/// configured ratio (阈值参数配置化：`ai.thresholds.output_tokens.ratio_percent`).
pub async fn is_valid_configured_max_output_tokens_configured(
    context_window: u32,
    max_tokens: u32,
) -> bool {
    let ratio_percent = configured_output_tokens_ratio_percent().await;
    max_tokens > 0
        && u64::from(max_tokens) * 100 <= u64::from(context_window) * u64::from(ratio_percent)
}

/// Resolve the configured output-token ratio percent
/// (`ai.thresholds.output_tokens.ratio_percent`), falling back to the legacy
/// `MAX_CONFIGURED_OUTPUT_TOKENS_RATIO_PERCENT = 40` when unset or zero.
pub(crate) async fn configured_output_tokens_ratio_percent() -> u32 {
    let Ok(config_service) = crate::service::config::get_global_config_service().await else {
        return MAX_CONFIGURED_OUTPUT_TOKENS_RATIO_PERCENT;
    };
    let Ok(thresholds) = config_service
        .get_config::<AiThresholdsConfig>(Some("ai.thresholds"))
        .await
    else {
        return MAX_CONFIGURED_OUTPUT_TOKENS_RATIO_PERCENT;
    };
    let ratio = thresholds.output_tokens.ratio_percent;
    if ratio == 0 {
        return MAX_CONFIGURED_OUTPUT_TOKENS_RATIO_PERCENT;
    }
    ratio
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, from = "AIModelConfigCompat")]
pub struct AIModelConfig {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub model_name: String,
    pub base_url: String,

    /// Computed actual request URL (auto-derived from base_url + provider format).
    /// Stored by the frontend when config is saved; falls back to base_url if absent.
    #[serde(default)]
    pub request_url: Option<String>,

    pub api_key: String,
    /// Context window size (total token limit for input + output).
    pub context_window: Option<u32>,
    /// Optional advanced override for the request output limit. When absent,
    /// BitFun derives a tiered limit from the context window at runtime.
    pub max_tokens: Option<u32>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub enabled: bool,
    /// Model category (primary category used for UI filtering).
    pub category: ModelCategory,
    /// Capability tags (multi-select).
    pub capabilities: Vec<ModelCapability>,
    /// Recommended use cases.
    #[serde(default)]
    pub recommended_for: Vec<String>,
    /// Additional metadata (JSON, for extensibility).
    pub metadata: Option<serde_json::Value>,

    /// Canonical model reasoning presets and default selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ReasoningConfig>,

    /// Whether to parse OpenAI-compatible text chunks containing `<think>...</think>` into
    /// streaming reasoning content.
    #[serde(default = "default_true")]
    pub inline_think_in_text: bool,

    /// Custom HTTP request headers.
    #[serde(default)]
    pub custom_headers: Option<std::collections::HashMap<String, String>>,

    /// Custom header mode: "replace" (default, full replacement) or "merge" (merge; apply
    /// defaults first, then custom).
    #[serde(default)]
    pub custom_headers_mode: Option<String>,

    /// Whether to skip SSL certificate verification (advanced; use only when necessary).
    #[serde(default)]
    pub skip_ssl_verify: bool,

    /// Custom request body (JSON string, used to override default request body fields).
    #[serde(default)]
    pub custom_request_body: Option<String>,

    /// Custom request body mode: "merge" (default) or "trim" (keep only essential runtime
    /// fields, then apply custom JSON).
    #[serde(default)]
    pub custom_request_body_mode: Option<String>,

    /// Authentication source for this model. Defaults to a static API key for
    /// backward compatibility; selecting a CLI source causes the AI client
    /// factory to look up `~/.codex/auth.json` or `~/.gemini/...` at request
    /// time and inject the resolved Bearer token / extra headers.
    #[serde(default)]
    pub auth: AuthConfig,
}

/// Stable identity of the runtime-affecting parts of a concrete model config.
///
/// Credentials are deliberately excluded: rotating a secret must not require
/// the user to approve the same provider/model again. Endpoint, provider,
/// model, request options, and authentication source remain part of the
/// identity so an approved binding cannot silently drift to different runtime
/// behavior while retaining the same config id.
pub fn model_runtime_binding_fingerprint(model: &AIModelConfig) -> String {
    let mut value = serde_json::to_value(model).unwrap_or(serde_json::Value::Null);
    if let serde_json::Value::Object(fields) = &mut value {
        fields.remove("api_key");
    }

    fn canonicalize(value: serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(fields) => {
                let mut entries = fields.into_iter().collect::<Vec<_>>();
                entries.sort_by(|left, right| left.0.cmp(&right.0));
                serde_json::Value::Object(
                    entries
                        .into_iter()
                        .map(|(key, value)| (key, canonicalize(value)))
                        .collect(),
                )
            }
            serde_json::Value::Array(values) => {
                serde_json::Value::Array(values.into_iter().map(canonicalize).collect())
            }
            value => value,
        }
    }

    let canonical = serde_json::to_vec(&canonicalize(value)).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(canonical);
    hex::encode(hasher.finalize())
}

/// Subscription provider whose in-app OAuth tokens authenticate a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionProvider {
    Codex,
    Antigravity,
    Opencode,
}

/// OpenCode API product selected for a subscription-authenticated model.
/// Zen and Go share one account credential but use different API namespaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenCodePlan {
    Zen,
    Go,
}

/// Where to obtain the runtime auth material for an `AIModelConfig`.
///
/// Stored on disk as `{"type":"api_key"}` or
/// `{"type":"subscription","provider":"codex"|"antigravity"|"opencode"}`.
/// OpenCode models may additionally persist `"plan":"zen"|"go"`; an absent
/// plan preserves the legacy Zen Chat Completions behavior.
/// Tokens live in the subscription auth store and are resolved at request time.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthConfig {
    /// Use the inline `api_key` string (default).
    #[default]
    ApiKey,
    /// Use BitFun in-app subscription OAuth for the named provider.
    Subscription {
        provider: SubscriptionProvider,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        plan: Option<OpenCodePlan>,
    },
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct AIModelConfigCompat {
    id: String,
    name: String,
    provider: String,
    model_name: String,
    base_url: String,
    request_url: Option<String>,
    api_key: String,
    context_window: Option<u32>,
    max_tokens: Option<u32>,
    temperature: Option<f64>,
    top_p: Option<f64>,
    enabled: bool,
    category: ModelCategory,
    capabilities: Vec<ModelCapability>,
    recommended_for: Vec<String>,
    metadata: Option<serde_json::Value>,
    reasoning: Option<ReasoningConfig>,
    #[serde(default = "default_true")]
    inline_think_in_text: bool,
    custom_headers: Option<std::collections::HashMap<String, String>>,
    custom_headers_mode: Option<String>,
    skip_ssl_verify: bool,
    custom_request_body: Option<String>,
    custom_request_body_mode: Option<String>,
    /// Parsed flexibly so unknown legacy auth tags fall back to ApiKey.
    #[serde(default)]
    auth: Option<serde_json::Value>,
}

fn parse_auth_config(value: Option<serde_json::Value>) -> AuthConfig {
    match value {
        None => AuthConfig::ApiKey,
        Some(raw) => serde_json::from_value(raw).unwrap_or(AuthConfig::ApiKey),
    }
}

impl From<AIModelConfigCompat> for AIModelConfig {
    fn from(value: AIModelConfigCompat) -> Self {
        Self {
            id: value.id,
            name: value.name,
            provider: value.provider,
            model_name: value.model_name,
            base_url: value.base_url,
            request_url: value.request_url,
            api_key: value.api_key,
            context_window: value.context_window,
            max_tokens: value.max_tokens,
            temperature: value.temperature,
            top_p: value.top_p,
            enabled: value.enabled,
            category: value.category,
            capabilities: value.capabilities,
            recommended_for: value.recommended_for,
            metadata: value.metadata,
            reasoning: value.reasoning,
            inline_think_in_text: value.inline_think_in_text,
            custom_headers: value.custom_headers,
            custom_headers_mode: value.custom_headers_mode,
            skip_ssl_verify: value.skip_ssl_verify,
            custom_request_body: value.custom_request_body,
            custom_request_body_mode: value.custom_request_body_mode,
            auth: parse_auth_config(value.auth),
        }
    }
}

pub use bitfun_core_types::ProxyConfig;

/// Configuration provider interface.
#[async_trait]
pub trait ConfigProvider: Send + Sync {
    /// Provider name.
    fn name(&self) -> &str;

    /// Returns the default configuration.
    fn get_default_config(&self) -> serde_json::Value;

    /// Validates configuration.
    async fn validate_config(&self, config: &serde_json::Value) -> BitFunResult<Vec<String>>;

    /// Called when configuration changes.
    async fn on_config_changed(
        &self,
        old_config: &serde_json::Value,
        new_config: &serde_json::Value,
    ) -> BitFunResult<()>;

    /// Migrates configuration (used for version upgrades).
    async fn migrate_config(
        &self,
        version: &str,
        config: serde_json::Value,
    ) -> BitFunResult<serde_json::Value>;
}

/// Configuration change event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigChangeEvent {
    pub path: String,
    pub old_value: serde_json::Value,
    pub new_value: serde_json::Value,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Event source: "user" | "system" | "migration".
    pub source: String,
}

/// Configuration validation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigValidationResult {
    pub valid: bool,
    pub errors: Vec<ConfigValidationError>,
    pub warnings: Vec<ConfigValidationWarning>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ConfigDiagnostic>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConfigDiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConfigDiagnosticRecoverability {
    None,
    AutoFix,
    ModelDisabled,
    DefaultsUsed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigDiagnostic {
    pub path: String,
    pub message: String,
    pub code: String,
    pub severity: ConfigDiagnosticSeverity,
    pub recoverability: ConfigDiagnosticRecoverability,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigValidationError {
    pub path: String,
    pub message: String,
    pub code: String,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigValidationWarning {
    pub path: String,
    pub message: String,
    pub code: String,
    pub severity: String,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            app: AppConfig::default(),
            editor: EditorConfig::default(),
            terminal: TerminalConfig::default(),
            workspace: WorkspaceConfig::default(),
            ai: AIConfig::default(),
            memories: MemoriesConfig::default(),
            project: ProjectConfig::default(),
            tool_permissions: ToolPermissionConfig::default(),
            mcp_servers: None,
            acp_clients: None,
            appearance: AppearanceConfig::default(),
            font: None,
            schema_version: CURRENT_CONFIG_SCHEMA_VERSION,
            version: "1.0.0".to_string(),
            last_modified: chrono::Utc::now(),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            language: "zh-CN".to_string(),
            auto_update: true,
            telemetry: false,
            startup_behavior: "lastWorkspace".to_string(),
            confirm_on_exit: true,
            restore_windows: true,
            prevent_sleep: false,
            zoom_level: 1.0,
            logging: AppLoggingConfig::default(),
            sidebar: SidebarConfig {
                width: 300,
                collapsed: false,
            },
            right_panel: RightPanelConfig {
                width: 400,
                collapsed: true,
            },
            notifications: NotificationConfig {
                enabled: true,
                position: "topRight".to_string(),
                duration: 5000,
                dialog_completion_notify: true,
                permission_request_notify: true,
                enable_startup_tips: true,
            },
            flow_chat: AppFlowChatConfig::default(),
            ai_experience: AIExperienceConfig::default(),
            keybindings: None,
            user_tool_groups: UserToolGroupsConfig::default(),
            user_skill_groups: UserSkillGroupsConfig::default(),
            close_button_behavior: default_close_button_behavior(),
            hooks: AgentHooksConfig::default(),
            worktrees: WorktreeSettings::default(),
        }
    }
}

impl Default for AppLoggingConfig {
    fn default() -> Self {
        Self {
            // Set to Debug in early development for easier diagnostics
            level: "debug".to_string(),
            include_sensitive_diagnostics: true,
            flow_chat_diagnostics: false,
            model_exchange_tracing: ModelExchangeTracingConfig::default(),
        }
    }
}

impl Default for ModelExchangeTracingConfig {
    fn default() -> Self {
        Self {
            mode: ModelExchangeTracingMode::Off,
        }
    }
}

impl Default for AIExperienceConfig {
    fn default() -> Self {
        Self {
            enable_session_title_generation: true,
            enable_welcome_panel_ai_analysis: false,
            enable_visual_mode: false,
            enable_agent_companion: true,
            agent_companion_display_mode: "desktop".to_string(),
            agent_companion_pet: default_agent_companion_pet(),
            enable_workspace_search: false,
            voice_input: VoiceInputConfig::default(),
            quick_actions: Vec::new(),
        }
    }
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            font_size: 14,
            font_family: "Consolas, \"Courier New\", monospace".to_string(),
            line_height: 1.5,
            tab_size: 2,
            insert_spaces: true,
            word_wrap: "off".to_string(),
            line_numbers: "on".to_string(),
            minimap: MinimapConfig {
                enabled: true,
                side: "right".to_string(),
                size: "proportional".to_string(),
            },
            auto_save: "afterDelay".to_string(),
            auto_save_delay: 1000,
            format_on_save: true,
            format_on_paste: true,
            trim_auto_whitespace: true,
        }
    }
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            default_shell: String::new(),
            terminal_panel_position: "right".to_string(),
            font_size: 14,
            font_family: "Consolas, \"Courier New\", monospace".to_string(),
            cursor_blink: true,
            cursor_style: "block".to_string(),
            scrollback: 1000,
        }
    }
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            exclude_patterns: vec![
                "**/node_modules/**".to_string(),
                "**/target/**".to_string(),
                "**/.git/**".to_string(),
                "**/dist/**".to_string(),
                "**/build/**".to_string(),
            ],
            include_patterns: vec!["**/*".to_string()],
            watch_ignore: vec![
                "**/node_modules/**".to_string(),
                "**/target/**".to_string(),
                "**/.git/**".to_string(),
            ],
            max_file_size: 50 * 1024 * 1024,
            encoding: "utf8".to_string(),
            line_ending: "auto".to_string(),
            trim_trailing_whitespace: true,
            insert_final_newline: true,
        }
    }
}

impl Default for AIConfig {
    fn default() -> Self {
        Self {
            models: vec![],
            func_agent_models: std::collections::HashMap::new(),
            default_models: DefaultModelsConfig::default(),
            agent_model_defaults: AgentModelDefaultsConfig::default(),
            agent_profiles: std::collections::HashMap::new(),
            skill_settings: SkillSettingsConfig::default(),
            review_teams: default_review_team_configs(),
            review_team_rate_limit_status: default_review_team_rate_limit_status(),
            subagent_max_concurrency: default_subagent_max_concurrency(),
            subagent_batch_execution_policy: default_subagent_batch_execution_policy(),
            proxy: ProxyConfig::default(),
            stream_idle_timeout_secs: default_stream_idle_timeout(),
            stream_ttft_timeout_secs: default_stream_ttft_timeout(),
            tool_execution_timeout_secs: default_tool_execution_timeout(),
            enable_deferred_tool_loading: default_enable_deferred_tool_loading(),
            allow_tool_json_repair: true,
            debug_mode_config: DebugModeConfig::default(),
            computer_use_enabled: false,
            browser_control_preferred_browser: String::new(),
            browser_control_auto_connect_on_startup: false,
            max_rounds: default_max_rounds(),
            rbac_enabled: true,
            external_instruction_sources: true,
            knowledge_base_root: String::new(),
            legion_max_nodes: default_legion_max_nodes(),
            legion_max_total_nodes: default_legion_max_total_nodes(),
            legion_deploy_frequency_per_hour: default_legion_deploy_frequency_per_hour(),
            thresholds: AiThresholdsConfig::default(),
        }
    }
}

impl Default for MemoriesConfig {
    fn default() -> Self {
        Self {
            generate_memories: false,
            generate_for_btw_sessions: false,
            use_memories: false,
            external_context_policy: MemoryExternalContextPolicy::ClearToolResults,
            max_raw_memories_for_consolidation: default_memory_max_raw_memories_for_consolidation(),
            max_unused_days: default_memory_max_unused_days(),
            max_rollout_age_days: default_memory_max_rollout_age_days(),
            max_rollouts_per_startup: default_memory_max_rollouts_per_startup(),
            max_rollouts_scan_limit: default_memory_max_rollouts_scan_limit(),
            min_rollout_idle_hours: default_memory_min_rollout_idle_hours(),
            phase1_max_concurrency: default_memory_phase1_max_concurrency(),
            phase1_retry_backoff_minutes: default_memory_phase1_retry_backoff_minutes(),
            phase1_lease_seconds: default_memory_phase1_lease_seconds(),
            phase2_lease_seconds: default_memory_phase2_lease_seconds(),
            phase2_success_cooldown_seconds: default_memory_phase2_success_cooldown_seconds(),
            phase2_retry_delay_seconds: default_memory_phase2_retry_delay_seconds(),
            extract_model: None,
            consolidation_model: None,
        }
    }
}

impl Default for AIModelConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            provider: String::new(),
            model_name: String::new(),
            base_url: String::new(),
            request_url: None,
            api_key: String::new(),
            context_window: None,
            max_tokens: None,
            temperature: None,
            top_p: None,
            enabled: false,
            category: ModelCategory::GeneralChat,
            capabilities: vec![],
            recommended_for: vec![],
            metadata: None,
            reasoning: None,
            inline_think_in_text: true,
            custom_headers: None,
            custom_headers_mode: None,
            skip_ssl_verify: false,
            custom_request_body: None,
            custom_request_body_mode: None,
            auth: AuthConfig::ApiKey,
        }
    }
}

impl Default for SidebarConfig {
    fn default() -> Self {
        Self {
            width: 300,
            collapsed: false,
        }
    }
}

impl Default for RightPanelConfig {
    fn default() -> Self {
        Self {
            width: 400,
            collapsed: true,
        }
    }
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            position: "topRight".to_string(),
            duration: 5000,
            dialog_completion_notify: true,
            permission_request_notify: true,
            enable_startup_tips: true,
        }
    }
}

impl Default for MinimapConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            side: "right".to_string(),
            size: "proportional".to_string(),
        }
    }
}

impl AIModelConfig {
    pub fn supports_capability(&self, capability: ModelCapability) -> bool {
        if self.capabilities.is_empty() {
            self.default_capabilities_for_category()
                .contains(&capability)
        } else {
            self.capabilities.contains(&capability)
        }
    }

    pub fn supports_text_generation(&self) -> bool {
        self.supports_capability(ModelCapability::TextChat)
    }

    /// Canonicalizes fields that only have meaning for text-generation
    /// requests. Returns the names of fields that were cleared.
    pub fn normalize_inapplicable_generation_fields(&mut self) -> Vec<&'static str> {
        if self.supports_text_generation() {
            return Vec::new();
        }

        let mut cleared = Vec::new();
        if self.context_window.take().is_some() {
            cleared.push("context_window");
        }
        if self.max_tokens.take().is_some() {
            cleared.push("max_tokens");
        }
        if self.temperature.take().is_some() {
            cleared.push("temperature");
        }
        if self.top_p.take().is_some() {
            cleared.push("top_p");
        }
        cleared
    }

    pub fn supports_image_understanding(&self) -> bool {
        self.supports_capability(ModelCapability::ImageUnderstanding)
    }

    /// Legacy helper that infers the model category from the model name and provider.
    ///
    /// This is kept for one-off migrations/debugging, but runtime behavior should prefer
    /// explicitly configured `category`/`capabilities`.
    pub fn infer_category_from_model_name(&self) -> ModelCategory {
        let model_name_lower = self.model_name.to_lowercase();
        let provider_lower = self.provider.to_lowercase();

        if model_name_lower.contains("dall-e")
            || model_name_lower.contains("dalle")
            || model_name_lower.contains("stable-diffusion")
            || model_name_lower.contains("midjourney")
        {
            return ModelCategory::ImageGeneration;
        }

        if model_name_lower.contains("embedding") || model_name_lower.contains("text-embedding") {
            return ModelCategory::Embedding;
        }

        if provider_lower.contains("perplexity") || model_name_lower.contains("perplexity") {
            return ModelCategory::SearchEnhanced;
        }

        if model_name_lower.contains("vision")
            || model_name_lower.contains("gpt-4o")
            || model_name_lower.contains("gpt-4-turbo")
            || model_name_lower.contains("claude-3")
            || model_name_lower.contains("gemini-pro-vision")
            || model_name_lower.contains("gemini-1.5")
            || model_name_lower.starts_with("kimi")
        {
            return ModelCategory::Multimodal;
        }

        if model_name_lower.contains("deepseek")
            || model_name_lower.contains("codellama")
            || model_name_lower.contains("code-")
        {
            return ModelCategory::CodeSpecialized;
        }

        ModelCategory::GeneralChat
    }

    /// Legacy helper that infers capability tags from the model category and name.
    ///
    /// This is kept for one-off migrations/debugging, but runtime behavior should prefer
    /// explicitly configured `category`/`capabilities`.
    pub fn infer_capabilities_from_model(&self) -> Vec<ModelCapability> {
        let mut capabilities = vec![];
        let model_name_lower = self.model_name.to_lowercase();

        match self.category {
            ModelCategory::GeneralChat => {
                capabilities.push(ModelCapability::TextChat);
                if model_name_lower.contains("gpt-4")
                    || model_name_lower.contains("claude-3")
                    || model_name_lower.contains("gemini")
                {
                    capabilities.push(ModelCapability::FunctionCalling);
                }
            }
            ModelCategory::Multimodal => {
                capabilities.push(ModelCapability::TextChat);
                capabilities.push(ModelCapability::ImageUnderstanding);
                capabilities.push(ModelCapability::FunctionCalling);
            }
            ModelCategory::ImageGeneration => {
                capabilities.push(ModelCapability::ImageGeneration);
            }
            ModelCategory::Embedding => {
                capabilities.push(ModelCapability::Embedding);
            }
            ModelCategory::SearchEnhanced => {
                capabilities.push(ModelCapability::TextChat);
                capabilities.push(ModelCapability::Search);
            }
            ModelCategory::CodeSpecialized => {
                capabilities.push(ModelCapability::TextChat);
                capabilities.push(ModelCapability::CodeSpecialized);
                capabilities.push(ModelCapability::FunctionCalling);
            }
            ModelCategory::SpeechRecognition => {
                capabilities.push(ModelCapability::SpeechRecognition);
            }
        }

        capabilities
    }

    fn default_capabilities_for_category(&self) -> Vec<ModelCapability> {
        match self.category {
            ModelCategory::GeneralChat => vec![ModelCapability::TextChat],
            ModelCategory::Multimodal => {
                vec![
                    ModelCapability::TextChat,
                    ModelCapability::ImageUnderstanding,
                ]
            }
            ModelCategory::ImageGeneration => vec![ModelCapability::ImageGeneration],
            ModelCategory::Embedding => vec![ModelCapability::Embedding],
            ModelCategory::SearchEnhanced => {
                vec![ModelCapability::TextChat, ModelCapability::Search]
            }
            ModelCategory::CodeSpecialized => {
                vec![ModelCapability::TextChat, ModelCapability::CodeSpecialized]
            }
            ModelCategory::SpeechRecognition => vec![ModelCapability::SpeechRecognition],
        }
    }

    /// Auto-completes missing capability information without rewriting explicit configuration.
    ///
    /// Important: we intentionally do not upgrade `category` or append inferred capabilities
    /// based on the model name here. Runtime behavior should follow explicit configuration.
    pub fn ensure_category_and_capabilities(&mut self) {
        if self.capabilities.is_empty() {
            self.capabilities = self.default_capabilities_for_category();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AIConfig, AIExperienceConfig, AIModelConfig, AgentModelDefaultsConfig, AgentProfileConfig,
        AgentProfileView, AppConfig, AppLoggingConfig, AuthConfig, GlobalConfig,
        MemoryExternalContextPolicy, ModelExchangeTracingMode, NotificationConfig, OpenCodePlan,
        SubagentBatchExecutionPolicy, SubagentModelSelection, SubscriptionProvider,
        UserSkillGroupsConfig, UserToolGroupsConfig,
    };
    use bitfun_runtime_ports::ToolPermissionConfig;

    #[test]
    fn prevent_sleep_defaults_to_disabled() {
        assert!(!AppConfig::default().prevent_sleep);

        let config: AppConfig =
            serde_json::from_value(serde_json::json!({})).expect("empty app config should default");
        assert!(!config.prevent_sleep);
    }

    #[test]
    fn subscription_auth_preserves_legacy_opencode_and_roundtrips_go_plan() {
        let legacy: AuthConfig = serde_json::from_value(serde_json::json!({
            "type": "subscription",
            "provider": "opencode"
        }))
        .expect("legacy OpenCode auth should deserialize");
        assert_eq!(
            legacy,
            AuthConfig::Subscription {
                provider: SubscriptionProvider::Opencode,
                plan: None,
            }
        );
        assert_eq!(
            serde_json::to_value(&legacy).expect("legacy auth should serialize"),
            serde_json::json!({
                "type": "subscription",
                "provider": "opencode"
            })
        );

        let go = AuthConfig::Subscription {
            provider: SubscriptionProvider::Opencode,
            plan: Some(OpenCodePlan::Go),
        };
        let serialized = serde_json::to_value(&go).expect("Go auth should serialize");
        assert_eq!(serialized["plan"], "go");
        assert_eq!(
            serde_json::from_value::<AuthConfig>(serialized).expect("Go auth should roundtrip"),
            go
        );
    }

    #[test]
    fn permission_request_notifications_default_to_enabled() {
        assert!(NotificationConfig::default().permission_request_notify);

        let config: NotificationConfig = serde_json::from_value(serde_json::json!({}))
            .expect("empty notification config should default");
        assert!(config.permission_request_notify);
    }

    #[test]
    fn agent_profile_defaults_keep_all_collections_empty() {
        let config = AgentProfileConfig::default();
        assert!(config.profile_id.is_empty());
        assert!(config.added_tools.is_empty());
        assert!(config.removed_tools.is_empty());
        assert!(config.disabled_user_skills.is_empty());
        assert!(config.enabled_user_skills.is_empty());
        assert!(config.subagent_overrides.is_empty());
        assert!(config.tool_permission_rules.is_empty());

        let view = AgentProfileView::default();
        assert!(view.profile_id.is_empty());
        assert!(view.enabled_tools.is_empty());
        assert!(view.default_tools.is_empty());
        assert!(view.disabled_user_skills.is_empty());
        assert!(view.enabled_user_skills.is_empty());
    }

    #[test]
    fn legacy_agent_profile_defaults_permission_rules_and_omits_empty_field() {
        let config: AgentProfileConfig = serde_json::from_value(serde_json::json!({
            "profile_id": "coding_shared",
            "added_tools": ["read"]
        }))
        .expect("legacy agent profile should deserialize");

        assert!(config.tool_permission_rules.is_empty());
        let serialized = serde_json::to_value(config).expect("agent profile should serialize");
        assert!(serialized.get("tool_permission_rules").is_none());
    }

    #[test]
    fn legacy_global_config_defaults_permission_settings() {
        let config: GlobalConfig = serde_json::from_value(serde_json::json!({}))
            .expect("legacy config should deserialize with permission defaults");

        assert_eq!(config.tool_permissions, ToolPermissionConfig::default());
    }

    #[test]
    fn user_tool_groups_default_to_version_one_without_persisted_groups() {
        let config: GlobalConfig = serde_json::from_value(serde_json::json!({}))
            .expect("legacy global config should deserialize");
        assert_eq!(config.app.user_tool_groups, UserToolGroupsConfig::default());

        let serialized = serde_json::to_value(&config).expect("config should serialize");
        assert!(serialized["app"].get("user_tool_groups").is_none());
    }

    #[test]
    fn user_tool_groups_preserve_the_versioned_ui_shape() {
        let config: GlobalConfig = serde_json::from_value(serde_json::json!({
            "app": {
                "user_tool_groups": {
                    "version": 1,
                    "groups": [{
                        "id": "daily-code",
                        "name": "Daily code changes",
                        "toolNames": ["Read", "Edit"]
                    }]
                }
            }
        }))
        .expect("user tool groups should deserialize");

        assert_eq!(
            config.app.user_tool_groups.groups[0].tool_names,
            vec!["Read".to_string(), "Edit".to_string()]
        );

        let serialized = serde_json::to_value(&config).expect("config should serialize");
        assert_eq!(
            serialized["app"]["user_tool_groups"]["groups"][0]["toolNames"],
            serde_json::json!(["Read", "Edit"])
        );
    }

    #[test]
    fn user_skill_groups_default_to_version_one_without_persisted_groups() {
        let config: GlobalConfig = serde_json::from_value(serde_json::json!({}))
            .expect("legacy global config should deserialize");
        assert_eq!(
            config.app.user_skill_groups,
            UserSkillGroupsConfig::default()
        );

        let serialized = serde_json::to_value(&config).expect("config should serialize");
        assert!(serialized["app"].get("user_skill_groups").is_none());
    }

    #[test]
    fn user_skill_groups_preserve_the_versioned_ui_shape() {
        let config: GlobalConfig = serde_json::from_value(serde_json::json!({
            "app": {
                "user_skill_groups": {
                    "version": 1,
                    "groups": [{
                        "id": "daily-coding",
                        "name": "Daily coding",
                        "skillKeys": ["builtin::find-skills", "user::review"]
                    }]
                }
            }
        }))
        .expect("user skill groups should deserialize");

        assert_eq!(
            config.app.user_skill_groups.groups[0].skill_keys,
            vec![
                "builtin::find-skills".to_string(),
                "user::review".to_string()
            ]
        );

        let serialized = serde_json::to_value(&config).expect("config should serialize");
        assert_eq!(
            serialized["app"]["user_skill_groups"]["groups"][0]["skillKeys"],
            serde_json::json!(["builtin::find-skills", "user::review"])
        );
    }

    #[test]
    fn removed_reasoning_fields_are_ignored_without_creating_reasoning_config() {
        let config: AIModelConfig = serde_json::from_value(serde_json::json!({
            "id": "model_1",
            "name": "Provider",
            "provider": "openai",
            "model_name": "test-model",
            "base_url": "https://example.com/v1",
            "api_key": "key",
            "enabled": true,
            "enable_thinking_process": true,
            "reasoning_mode": "adaptive",
            "reasoning_effort": "high",
            "thinking_budget_tokens": 12000
        }))
        .expect("config with removed fields should deserialize");

        assert!(config.reasoning.is_none());
    }

    #[test]
    fn global_config_preserves_project_mcp_servers() {
        let config: GlobalConfig = serde_json::from_value(serde_json::json!({
            "project": {
                "mcp_servers": [
                    {
                        "id": "project-docs",
                        "name": "Project Docs",
                        "server_type": "local",
                        "command": "docs-mcp",
                        "args": []
                    }
                ]
            }
        }))
        .expect("project scoped MCP config should deserialize");

        assert_eq!(
            config
                .project
                .mcp_servers
                .as_ref()
                .and_then(|value| value.as_array())
                .map(Vec::len),
            Some(1)
        );

        let serialized = serde_json::to_value(&config).expect("config should serialize");
        assert_eq!(
            serialized["project"]["mcp_servers"][0]["id"],
            "project-docs"
        );
    }

    #[test]
    fn global_config_preserves_terminal_panel_position() {
        let config: GlobalConfig = serde_json::from_value(serde_json::json!({
            "terminal": {
                "terminal_panel_position": "bottom"
            }
        }))
        .expect("terminal panel position config should deserialize");

        assert_eq!(config.terminal.terminal_panel_position, "bottom");

        let serialized = serde_json::to_value(&config).expect("config should serialize");
        assert_eq!(serialized["terminal"]["terminal_panel_position"], "bottom");
    }

    #[test]
    fn global_config_serialization_uses_appearance_selection() {
        let serialized =
            serde_json::to_value(GlobalConfig::default()).expect("config should serialize");

        assert!(
            serialized.get("theme").is_none(),
            "Rust config must not export the removed GUI theme schema"
        );
        assert!(serialized.get("themes").is_none());
        assert_eq!(serialized["appearance"]["selection"], "system");
    }

    #[test]
    fn defaults_agent_companion_pet_to_bitfun() {
        let config: AIExperienceConfig =
            serde_json::from_value(serde_json::json!({})).expect("empty config should default");

        let pet = config
            .agent_companion_pet
            .as_ref()
            .expect("default companion pet should be present");
        assert_eq!(pet.id, "bitfun");
        assert_eq!(pet.display_name, "Bitfun");
        assert_eq!(pet.package_path, "/agent-companion-pets/bitfun");
        assert_eq!(
            pet.spritesheet_path,
            "/agent-companion-pets/bitfun/spritesheet.webp"
        );
    }

    #[test]
    fn preserves_selected_agent_companion_pet() {
        let config: AIExperienceConfig = serde_json::from_value(serde_json::json!({
            "enable_session_title_generation": true,
            "enable_welcome_panel_ai_analysis": false,
            "enable_visual_mode": false,
            "enable_agent_companion": true,
            "agent_companion_display_mode": "desktop",
            "agent_companion_pet": {
                "id": "boxcat",
                "displayName": "Boxcat",
                "description": "A tiny cat tucked inside a cardboard box for cozy coding sessions.",
                "source": "preset",
                "packagePath": "/agent-companion-pets/boxcat",
                "spritesheetPath": "/agent-companion-pets/boxcat/spritesheet.webp",
                "spritesheetMimeType": "image/webp"
            }
        }))
        .expect("AI experience config with selected companion pet should deserialize");

        let pet = config
            .agent_companion_pet
            .as_ref()
            .expect("selected companion pet should be retained");
        assert_eq!(pet.id, "boxcat");
        assert_eq!(pet.display_name, "Boxcat");
        assert_eq!(pet.package_path, "/agent-companion-pets/boxcat");
        assert_eq!(config.agent_companion_display_mode, "desktop");

        let serialized = serde_json::to_value(&config).expect("config should serialize");
        assert_eq!(serialized["agent_companion_pet"]["displayName"], "Boxcat");
        assert_eq!(
            serialized["agent_companion_pet"]["spritesheetPath"],
            "/agent-companion-pets/boxcat/spritesheet.webp"
        );
    }

    #[test]
    fn ai_experience_quick_actions_round_trip_through_global_config() {
        let config: GlobalConfig = serde_json::from_value(serde_json::json!({
            "app": {
                "language": "en-US",
                "auto_update": true,
                "telemetry": true,
                "startup_behavior": "default",
                "confirm_on_exit": true,
                "restore_windows": false,
                "zoom_level": 100,
                "sidebar": { "width": 260, "collapsed": false },
                "right_panel": { "width": 400, "collapsed": true },
                "notifications": {
                    "enabled": true,
                    "position": "top-right",
                    "duration": 4000,
                    "dialog_completion_notify": true,
                    "permission_request_notify": false,
                    "enable_startup_tips": true
                },
                "ai_experience": {
                    "enable_session_title_generation": true,
                    "enable_welcome_panel_ai_analysis": false,
                    "enable_visual_mode": false,
                    "enable_agent_companion": true,
                    "agent_companion_display_mode": "desktop",
                    "enable_workspace_search": false,
                    "quick_actions": [
                        {
                            "id": "custom_1",
                            "label": "Run tests",
                            "prompt": "Run the test suite",
                            "enabled": true
                        }
                    ]
                }
            }
        }))
        .expect("minimal app config with quick_actions should deserialize");

        let actions = &config.app.ai_experience.quick_actions;
        assert!(!config.app.notifications.permission_request_notify);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].id, "custom_1");
        assert_eq!(actions[0].label, "Run tests");

        let serialized = serde_json::to_value(&config).expect("config should serialize");
        assert_eq!(
            serialized["app"]["ai_experience"]["quick_actions"][0]["id"],
            "custom_1"
        );
        assert_eq!(
            serialized["app"]["notifications"]["permission_request_notify"],
            false
        );
    }

    #[test]
    fn legacy_app_session_config_is_ignored() {
        let config: GlobalConfig = serde_json::from_value(serde_json::json!({
            "app": {
                "session_config": {
                    "default_mode": "cowork"
                }
            }
        }))
        .expect("legacy app session config should be ignored");

        let serialized = serde_json::to_value(&config).expect("config should serialize");
        assert!(serialized["app"].get("session_config").is_none());
    }

    #[test]
    fn app_flow_chat_default_mode_id_round_trips() {
        let config: GlobalConfig = serde_json::from_value(serde_json::json!({
            "app": {
                "flow_chat": {
                    "default_mode_id": "PlannerPlus"
                }
            }
        }))
        .expect("flow chat config should deserialize");

        assert_eq!(
            config.app.flow_chat.default_mode_id.as_deref(),
            Some("PlannerPlus")
        );

        let serialized = serde_json::to_value(&config).expect("config should serialize");
        assert_eq!(
            serialized["app"]["flow_chat"]["default_mode_id"],
            "PlannerPlus"
        );
    }

    #[test]
    fn app_flow_chat_permission_mode_control_defaults_to_visible_without_persisting_default() {
        let default_config: GlobalConfig = serde_json::from_value(serde_json::json!({
            "app": {
                "flow_chat": {}
            }
        }))
        .expect("flow chat config without visibility preference should deserialize");

        assert!(default_config.app.flow_chat.show_permission_mode_control);
        let default_serialized =
            serde_json::to_value(&default_config).expect("config should serialize");
        assert!(default_serialized["app"]["flow_chat"]
            .get("show_permission_mode_control")
            .is_none());

        let hidden_config: GlobalConfig = serde_json::from_value(serde_json::json!({
            "app": {
                "flow_chat": {
                    "show_permission_mode_control": false
                }
            }
        }))
        .expect("flow chat config with hidden permission control should deserialize");

        assert!(!hidden_config.app.flow_chat.show_permission_mode_control);
        let hidden_serialized =
            serde_json::to_value(&hidden_config).expect("config should serialize");
        assert_eq!(
            hidden_serialized["app"]["flow_chat"]["show_permission_mode_control"],
            false
        );
    }

    #[test]
    fn serialization_omits_removed_reasoning_fields() {
        let config: AIModelConfig = serde_json::from_value(serde_json::json!({
            "id": "model_1",
            "name": "Provider",
            "provider": "openai",
            "model_name": "test-model",
            "base_url": "https://example.com/v1",
            "api_key": "key",
            "enabled": true,
            "enable_thinking_process": true
        }))
        .expect("config with removed fields should deserialize");

        let value = serde_json::to_value(&config).expect("config should serialize");

        assert!(value.get("enable_thinking_process").is_none());
        assert!(value.get("reasoning_mode").is_none());
        assert!(value.get("reasoning_effort").is_none());
        assert!(value.get("thinking_budget_tokens").is_none());
        assert!(value.get("reasoning").is_none());
    }

    #[test]
    fn default_model_config_enables_inline_think_in_text() {
        let config = AIModelConfig::default();
        assert!(config.inline_think_in_text);
    }

    #[test]
    fn deserializes_missing_inline_think_in_text_as_enabled() {
        let config: AIModelConfig = serde_json::from_value(serde_json::json!({
            "id": "model_1",
            "name": "Provider",
            "provider": "openai",
            "model_name": "test-model",
            "base_url": "https://example.com/v1",
            "api_key": "key",
            "enabled": true
        }))
        .expect("config without inline_think_in_text should deserialize");

        assert!(config.inline_think_in_text);
    }

    #[test]
    fn default_ai_config_uses_generous_stream_timeouts() {
        let config = AIConfig::default();

        assert_eq!(config.stream_idle_timeout_secs, Some(600));
        assert_eq!(config.stream_ttft_timeout_secs, Some(600));
        assert!(config.enable_deferred_tool_loading);
        assert!(config.allow_tool_json_repair);
        assert_eq!(config.subagent_max_concurrency, 5);
        assert_eq!(
            config.subagent_batch_execution_policy,
            SubagentBatchExecutionPolicy::ForceParallel
        );
        let review_team = config
            .review_teams
            .get("default")
            .expect("default review team config should exist");
        assert_eq!(review_team.reviewer_timeout_seconds, 3600);
        assert_eq!(review_team.judge_timeout_seconds, 2400);
        assert!(!review_team.auto_fix_enabled);
        assert_eq!(review_team.strategy_level, "normal");
        assert!(review_team.member_strategy_overrides.is_empty());
        assert_eq!(config.review_team_rate_limit_status, serde_json::json!({}));
        assert_eq!(config.agent_model_defaults.mode, "auto");
        assert_eq!(
            config.agent_model_defaults.subagents.default_selection,
            SubagentModelSelection::fixed("fast")
        );
        assert_eq!(
            config
                .agent_model_defaults
                .subagents
                .builtin
                .get("GeneralPurpose"),
            Some(&SubagentModelSelection::fixed("primary"))
        );
        assert_eq!(
            config.agent_model_defaults.subagents.fork,
            SubagentModelSelection::Inherit
        );
    }

    #[test]
    fn subagent_model_selection_uses_a_tagged_persistent_shape() {
        let selection = SubagentModelSelection::fixed("fast");
        assert_eq!(
            serde_json::to_value(selection).expect("selection should serialize"),
            serde_json::json!({ "kind": "fixed", "model_id": "fast" })
        );

        let inherited: SubagentModelSelection = serde_json::from_value(serde_json::json!({
            "kind": "inherit"
        }))
        .expect("inherit selection should deserialize");
        assert_eq!(inherited, SubagentModelSelection::Inherit);
    }

    #[test]
    fn builtin_subagent_without_override_uses_the_shared_default() {
        let mut defaults = AgentModelDefaultsConfig::default();
        defaults.subagents.default_selection = SubagentModelSelection::fixed("primary");

        assert_eq!(
            defaults.builtin_subagent_selection("Explore"),
            SubagentModelSelection::fixed("primary")
        );
    }

    #[test]
    fn general_purpose_uses_primary_unless_explicitly_overridden() {
        let mut defaults = AgentModelDefaultsConfig::default();

        assert_eq!(
            defaults.builtin_subagent_selection("GeneralPurpose"),
            SubagentModelSelection::fixed("primary")
        );

        defaults.subagents.builtin.insert(
            "GeneralPurpose".to_string(),
            SubagentModelSelection::fixed("fast"),
        );
        assert_eq!(
            defaults.builtin_subagent_selection("GeneralPurpose"),
            SubagentModelSelection::fixed("fast")
        );
    }

    #[test]
    fn default_global_config_includes_enabled_memories_config() {
        let config = GlobalConfig::default();

        assert!(!config.memories.generate_memories);
        assert!(!config.memories.generate_for_btw_sessions);
        assert!(!config.memories.use_memories);
        assert_eq!(
            config.memories.external_context_policy,
            MemoryExternalContextPolicy::ClearToolResults
        );
        assert_eq!(config.memories.max_raw_memories_for_consolidation, 64);
        assert_eq!(config.memories.max_unused_days, 30);
        assert_eq!(config.memories.max_rollout_age_days, 10);
        assert_eq!(config.memories.max_rollouts_per_startup, 2);
        assert_eq!(config.memories.max_rollouts_scan_limit, 2_000);
        assert_eq!(config.memories.min_rollout_idle_hours, 6);
        assert_eq!(config.memories.phase1_max_concurrency, 1);
        assert_eq!(config.memories.phase1_retry_backoff_minutes, 60);
        assert_eq!(config.memories.phase1_lease_seconds, 60 * 60);
        assert_eq!(config.memories.phase2_lease_seconds, 60 * 60);
        assert_eq!(config.memories.phase2_success_cooldown_seconds, 6 * 60 * 60);
        assert_eq!(config.memories.phase2_retry_delay_seconds, 60 * 60);
        assert_eq!(config.memories.extract_model, None);
        assert_eq!(config.memories.consolidation_model, None);
    }

    #[test]
    fn deserializes_explicit_memories_config() {
        let config: GlobalConfig = serde_json::from_value(serde_json::json!({
            "memories": {
                "generate_memories": false,
                "generate_for_btw_sessions": true,
                "use_memories": false,
                "external_context_policy": "skip_session",
                "max_raw_memories_for_consolidation": 12,
                "max_unused_days": 7,
                "max_rollout_age_days": 14,
                "max_rollouts_per_startup": 8,
                "max_rollouts_scan_limit": 200,
                "min_rollout_idle_hours": 12,
                "phase1_max_concurrency": 3,
                "phase1_retry_backoff_minutes": 45,
                "phase1_lease_seconds": 600,
                "phase2_lease_seconds": 1200,
                "phase2_success_cooldown_seconds": 7200,
                "phase2_retry_delay_seconds": 300,
                "extract_model": "extractor",
                "consolidation_model": "consolidator"
            }
        }))
        .expect("global config with memories section should deserialize");

        assert!(!config.memories.generate_memories);
        assert!(config.memories.generate_for_btw_sessions);
        assert!(!config.memories.use_memories);
        assert_eq!(
            config.memories.external_context_policy,
            MemoryExternalContextPolicy::SkipSession
        );
        assert_eq!(config.memories.max_raw_memories_for_consolidation, 12);
        assert_eq!(config.memories.max_unused_days, 7);
        assert_eq!(config.memories.max_rollout_age_days, 14);
        assert_eq!(config.memories.max_rollouts_per_startup, 8);
        assert_eq!(config.memories.max_rollouts_scan_limit, 200);
        assert_eq!(config.memories.min_rollout_idle_hours, 12);
        assert_eq!(config.memories.phase1_max_concurrency, 3);
        assert_eq!(config.memories.phase1_retry_backoff_minutes, 45);
        assert_eq!(config.memories.phase1_lease_seconds, 600);
        assert_eq!(config.memories.phase2_lease_seconds, 1200);
        assert_eq!(config.memories.phase2_success_cooldown_seconds, 7200);
        assert_eq!(config.memories.phase2_retry_delay_seconds, 300);
        assert_eq!(config.memories.extract_model.as_deref(), Some("extractor"));
        assert_eq!(
            config.memories.consolidation_model.as_deref(),
            Some("consolidator")
        );
    }

    #[test]
    fn deserializes_missing_stream_timeouts_as_generous_defaults() {
        let config: AIConfig = serde_json::from_value(serde_json::json!({
            "models": [],
            "func_agent_models": {},
            "default_models": {},
            "agent_profiles": {},
            "proxy": {
                "enabled": false,
                "url": ""
            }
        }))
        .expect("config without stream_idle_timeout_secs should deserialize");

        assert_eq!(config.stream_idle_timeout_secs, Some(600));
        assert_eq!(config.stream_ttft_timeout_secs, Some(600));
        assert!(config.allow_tool_json_repair);
        assert_eq!(config.subagent_max_concurrency, 5);
        assert_eq!(
            config.subagent_batch_execution_policy,
            SubagentBatchExecutionPolicy::ForceParallel
        );
        assert!(config.review_teams.contains_key("default"));
    }

    #[test]
    fn preserves_explicit_disabled_tool_json_repair() {
        let config: AIConfig = serde_json::from_value(serde_json::json!({
            "models": [],
            "func_agent_models": {},
            "default_models": {},
            "agent_profiles": {},
            "allow_tool_json_repair": false,
            "proxy": {
                "enabled": false,
                "url": ""
            }
        }))
        .expect("config with an explicit JSON repair setting should deserialize");

        assert!(!config.allow_tool_json_repair);
    }

    #[test]
    fn deserializes_explicit_null_stream_ttft_timeout_as_none() {
        let config: AIConfig = serde_json::from_value(serde_json::json!({
            "models": [],
            "func_agent_models": {},
            "default_models": {},
            "agent_profiles": {},
            "proxy": {
                "enabled": false,
                "url": ""
            },
            "stream_ttft_timeout_secs": null
        }))
        .expect("config with explicit null stream_ttft_timeout_secs should deserialize");

        assert_eq!(config.stream_ttft_timeout_secs, None);
        assert_eq!(config.stream_idle_timeout_secs, Some(600));
    }

    #[test]
    fn app_logging_defaults_to_sensitive_diagnostics_enabled() {
        let config: AppLoggingConfig = serde_json::from_value(serde_json::json!({
            "level": "trace"
        }))
        .expect("logging config without sensitive preference should deserialize");

        assert!(config.include_sensitive_diagnostics);
        assert!(!config.flow_chat_diagnostics);
        assert_eq!(
            config.model_exchange_tracing.mode,
            ModelExchangeTracingMode::Off
        );
    }

    #[test]
    fn deserializes_explicit_subagent_max_concurrency() {
        let config: AIConfig = serde_json::from_value(serde_json::json!({
            "models": [],
            "func_agent_models": {},
            "default_models": {},
            "agent_profiles": {},
            "subagent_max_concurrency": 9,
            "proxy": {
                "enabled": false,
                "url": ""
            }
        }))
        .expect("config with subagent_max_concurrency should deserialize");

        assert_eq!(config.subagent_max_concurrency, 9);
    }

    #[test]
    fn deserializes_explicit_subagent_batch_execution_policy() {
        let config: AIConfig = serde_json::from_value(serde_json::json!({
            "models": [],
            "func_agent_models": {},
            "default_models": {},
            "agent_profiles": {},
            "subagent_batch_execution_policy": "force_parallel",
            "proxy": {
                "enabled": false,
                "url": ""
            }
        }))
        .expect("config with subagent_batch_execution_policy should deserialize");

        assert_eq!(
            config.subagent_batch_execution_policy,
            SubagentBatchExecutionPolicy::ForceParallel
        );
    }

    #[test]
    fn deserializes_mode_profiles_with_null_entries() {
        let config: AIConfig = serde_json::from_value(serde_json::json!({
            "models": [],
            "func_agent_models": {},
            "default_models": {},
            "agent_profiles": {
                "Claw": null,
                "Cowork": {
                    "profile_id": "Cowork",
                    "removed_tools": ["shell"]
                }
            },
            "proxy": {
                "enabled": false,
                "url": ""
            }
        }))
        .expect("config with null mode config entries should deserialize");

        assert!(!config.agent_profiles.contains_key("Claw"));
        assert_eq!(
            config
                .agent_profiles
                .get("Cowork")
                .expect("non-null mode config should be retained")
                .removed_tools,
            vec!["shell".to_string()]
        );
    }

    #[test]
    fn deserializes_explicit_default_review_team_config() {
        let config: AIConfig = serde_json::from_value(serde_json::json!({
            "models": [],
            "func_agent_models": {},
            "default_models": {},
            "agent_profiles": {},
            "review_teams": {
                "default": {
                    "extra_subagent_ids": ["ExtraReviewer"],
                    "reviewer_timeout_seconds": 120,
                    "judge_timeout_seconds": 90,
                    "strategy_level": "deep",
                    "member_strategy_overrides": {
                        "ReviewSecurity": "quick",
                        "ExtraReviewer": "normal"
                    },
                    "auto_fix_enabled": false
                }
            },
            "proxy": {
                "enabled": false,
                "url": ""
            }
        }))
        .expect("config with review_teams should deserialize");

        let review_team = config
            .review_teams
            .get("default")
            .expect("default review team config should be retained");
        assert_eq!(review_team.extra_subagent_ids, vec!["ExtraReviewer"]);
        assert_eq!(review_team.reviewer_timeout_seconds, 120);
        assert_eq!(review_team.judge_timeout_seconds, 90);
        assert_eq!(review_team.strategy_level, "deep");
        assert_eq!(
            review_team.member_strategy_overrides.get("ReviewSecurity"),
            Some(&"quick".to_string())
        );
        assert_eq!(
            review_team.member_strategy_overrides.get("ExtraReviewer"),
            Some(&"normal".to_string())
        );
        assert!(!review_team.auto_fix_enabled);

        let serialized = serde_json::to_value(&config).expect("config should serialize");
        assert_eq!(
            serialized["review_teams"]["default"]["strategy_level"],
            "deep"
        );
        assert_eq!(
            serialized["review_teams"]["default"]["member_strategy_overrides"]["ReviewSecurity"],
            "quick"
        );
    }

    #[test]
    fn review_team_auxiliary_config_is_not_stored_inside_review_team_map() {
        let config: AIConfig = serde_json::from_value(serde_json::json!({
            "models": [],
            "review_teams": {
                "default": {
                    "strategy_level": "normal"
                }
            },
            "review_team_rate_limit_status": {
                "remaining": 2
            },
        }))
        .expect("review team auxiliary config should deserialize");

        assert!(config.review_teams.contains_key("default"));
        assert!(!config.review_teams.contains_key("rate_limit_status"));
        assert_eq!(
            config.review_team_rate_limit_status["remaining"],
            serde_json::json!(2)
        );
        let serialized =
            serde_json::to_value(&config).expect("review team auxiliary config should serialize");
        assert!(serialized["review_teams"]["rate_limit_status"].is_null());
    }

    #[test]
    fn legion_thresholds_default_to_legacy_hardcoded_values() {
        let config = AIConfig::default();
        assert_eq!(config.legion_max_nodes, 20);
        assert_eq!(config.legion_max_total_nodes, 60);
        assert_eq!(config.legion_deploy_frequency_per_hour, 10);

        // Unset config (empty AIConfig) must deserialize to the same defaults —
        // this is what keeps the default path behavior identical to the old
        // hard-coded constants (legion 阈值参数配置化零回归).
        let empty: AIConfig =
            serde_json::from_value(serde_json::json!({})).expect("empty ai config should default");
        assert_eq!(empty.legion_max_nodes, 20);
        assert_eq!(empty.legion_max_total_nodes, 60);
        assert_eq!(empty.legion_deploy_frequency_per_hour, 10);
    }

    #[test]
    fn legion_thresholds_round_trip_explicit_values() {
        let config: AIConfig = serde_json::from_value(serde_json::json!({
            "models": [],
            "func_agent_models": {},
            "default_models": {},
            "agent_profiles": {},
            "legion_max_nodes": 5,
            "legion_max_total_nodes": 30,
            "legion_deploy_frequency_per_hour": 0,
            "proxy": {
                "enabled": false,
                "url": ""
            }
        }))
        .expect("legion thresholds config should deserialize");

        assert_eq!(config.legion_max_nodes, 5);
        assert_eq!(config.legion_max_total_nodes, 30);
        // 0 = frequency limit disabled.
        assert_eq!(config.legion_deploy_frequency_per_hour, 0);

        let serialized = serde_json::to_value(&config).expect("config should serialize");
        assert_eq!(serialized["legion_max_nodes"], 5);
        assert_eq!(serialized["legion_max_total_nodes"], 30);
        assert_eq!(serialized["legion_deploy_frequency_per_hour"], 0);
    }
}
