//! Unified configuration system type definitions
//!
//! Defines all configuration-related types shared between backend and frontend.

use crate::util::errors::*;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Global configuration structure - matches the frontend `GlobalConfig` exactly.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GlobalConfig {
    pub app: AppConfig,
    pub theme: ThemeConfig,
    pub editor: EditorConfig,
    pub terminal: TerminalConfig,
    pub workspace: WorkspaceConfig,
    pub ai: AIConfig,
    pub prompt_templates: Option<serde_json::Value>,
    /// MCP server configuration (stored uniformly; supports both JSON and structured formats).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<serde_json::Value>,
    /// Theme system configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub themes: Option<ThemesConfig>,
    pub version: String,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub last_modified: chrono::DateTime<chrono::Utc>,
}

/// App configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub language: String,
    pub auto_update: bool,
    pub telemetry: bool,
    pub startup_behavior: String,
    pub confirm_on_exit: bool,
    pub restore_windows: bool,
    pub zoom_level: f64,
    pub sidebar: SidebarConfig,
    pub right_panel: RightPanelConfig,
    pub notifications: NotificationConfig,
    pub ai_experience: AIExperienceConfig,
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
}

/// Theme configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    pub id: String,
    pub name: String,
    pub display_name: String,
    #[serde(rename = "type")]
    pub theme_type: String,
    pub colors: ThemeColors,
    pub fonts: ThemeFonts,
    pub spacing: ThemeSpacing,
    pub border_radius: ThemeBorderRadius,
    pub shadows: ThemeShadows,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeColors {
    pub primary: String,
    pub secondary: String,
    pub background: String,
    pub surface: String,
    pub text: String,
    pub text_secondary: String,
    pub border: String,
    pub accent: String,
    pub success: String,
    pub warning: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeFonts {
    pub primary: String,
    pub code: String,
    pub sizes: FontSizes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FontSizes {
    pub xs: String,
    pub sm: String,
    pub base: String,
    pub lg: String,
    pub xl: String,
    #[serde(rename = "2xl")]
    pub xxl: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeSpacing {
    pub xs: String,
    pub sm: String,
    pub md: String,
    pub lg: String,
    pub xl: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeBorderRadius {
    pub sm: String,
    pub md: String,
    pub lg: String,
    pub full: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeShadows {
    pub sm: String,
    pub md: String,
    pub lg: String,
}

/// Theme system configuration (new).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemesConfig {
    /// Currently active theme ID.
    pub current: String,
    /// User-defined themes (stored as JSON).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom: Option<serde_json::Value>,
}

impl Default for ThemesConfig {
    fn default() -> Self {
        Self {
            current: "bitfun-slate".to_string(),
            custom: None,
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
    pub theme: String,
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
    pub font_size: u32,
    pub font_family: String,
    pub cursor_blink: bool,
    pub cursor_style: String,
    pub scrollback: u32,
    pub theme: TerminalThemeConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TerminalThemeConfig {
    pub background: String,
    pub foreground: String,
    pub cursor: String,
    pub selection: String,
    pub black: String,
    pub red: String,
    pub green: String,
    pub yellow: String,
    pub blue: String,
    pub magenta: String,
    pub cyan: String,
    pub white: String,
    pub bright_black: String,
    pub bright_red: String,
    pub bright_green: String,
    pub bright_yellow: String,
    pub bright_blue: String,
    pub bright_magenta: String,
    pub bright_cyan: String,
    pub bright_white: String,
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

/// Model category (for UI display and filtering).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ModelCategory {
    /// General chat model.
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

impl Default for ModelCategory {
    fn default() -> Self {
        Self::GeneralChat
    }
}

/// Default model configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
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

impl Default for DefaultModelsConfig {
    fn default() -> Self {
        Self {
            primary: None,
            fast: None,
            search: None,
            image_understanding: None,
            image_generation: None,
            speech_recognition: None,
        }
    }
}

/// AI configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AIConfig {
    /// All configured models.
    pub models: Vec<AIModelConfig>,

    /// Model mapping for primary agents (e.g. Explore, FileFinder).
    /// agent_type -> model_id
    pub agent_models: HashMap<String, String>,

    /// Model mapping for functional agents (e.g. startchat-func-agent, git-func-agent).
    /// func_agent_name -> model_id
    #[serde(default)]
    pub func_agent_models: HashMap<String, String>,

    /// Default model configuration.
    #[serde(default)]
    pub default_models: DefaultModelsConfig,

    /// Mode configuration.
    /// mode_id -> ModeConfig
    #[serde(default)]
    pub mode_configs: HashMap<String, ModeConfig>,

    /// SubAgent configuration (enable/disable state).
    /// subagent_id -> SubAgentConfig
    #[serde(default)]
    pub subagent_configs: HashMap<String, SubAgentConfig>,

    /// Global proxy configuration.
    pub proxy: ProxyConfig,

    /// Tool execution timeout in seconds; `None` means wait indefinitely.
    #[serde(default = "default_tool_execution_timeout")]
    pub tool_execution_timeout_secs: Option<u64>,

    /// Tool confirmation timeout in seconds; `None` means wait indefinitely.
    #[serde(default = "default_tool_confirmation_timeout")]
    pub tool_confirmation_timeout_secs: Option<u64>,

    /// Skip tool execution confirmation (global, applies to all modes).
    #[serde(default)]
    pub skip_tool_confirmation: bool,

    /// Debug-mode configuration (log path, language templates, etc.).
    #[serde(default)]
    pub debug_mode_config: DebugModeConfig,

    /// Known tools (all non-MCP tools from the registry at last startup).
    /// Used to detect added and removed tools.
    #[serde(default)]
    pub known_tools: Vec<String>,
}

/// Mode configuration (tool configuration per mode).
///
/// Model mapping has moved to `AIConfig.agent_models`, keyed by `mode_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ModeConfig {
    /// Mode ID (e.g. agentic, debug, requirement, ui-design).
    pub mode_id: String,

    /// Available tools.
    pub available_tools: Vec<String>,

    /// Whether this mode is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Default tools for this mode (from the mode registry; not read from config).
    /// Used only for frontend display and reset; persisted but overwritten on load.
    #[serde(skip_deserializing)]
    pub default_tools: Vec<String>,
}

fn default_true() -> bool {
    true
}

/// Default is no timeout (wait forever).
fn default_tool_execution_timeout() -> Option<u64> {
    None
}

/// Default is no timeout (wait forever).
fn default_tool_confirmation_timeout() -> Option<u64> {
    None
}

impl Default for ModeConfig {
    fn default() -> Self {
        Self {
            mode_id: String::new(),
            available_tools: Vec::new(),
            enabled: true,
            default_tools: Vec::new(),
        }
    }
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

/// SubAgent configuration (enabled/disabled per sub-agent).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SubAgentConfig {
    /// Whether this SubAgent is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for SubAgentConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AIModelConfig {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub model_name: String,
    pub base_url: String,
    pub api_key: String,
    /// Context window size (total token limit for input + output).
    pub context_window: Option<u32>,
    /// Max output tokens (request parameter limiting model output length).
    pub max_tokens: Option<u32>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub frequency_penalty: Option<f64>,
    pub presence_penalty: Option<f64>,
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

    /// Whether to display the thinking process (for hybrid/thinking models such as o1).
    #[serde(default)]
    pub enable_thinking_process: bool,

    /// Whether preserved thinking is supported (Preserved Thinking).
    /// If false, `reasoning_content` from previous turns is ignored when sending messages.
    #[serde(default)]
    pub support_preserved_thinking: bool,

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
}

/// Proxy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProxyConfig {
    /// Whether the proxy is enabled.
    pub enabled: bool,

    /// Proxy URL (format: http://host:port or socks5://host:port).
    pub url: String,

    /// Proxy username (optional).
    pub username: Option<String>,

    /// Proxy password (optional).
    pub password: Option<String>,
}


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
            theme: ThemeConfig::default(),
            editor: EditorConfig::default(),
            terminal: TerminalConfig::default(),
            workspace: WorkspaceConfig::default(),
            ai: AIConfig::default(),
            prompt_templates: None,
            mcp_servers: None,
            themes: Some(ThemesConfig::default()),
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
            zoom_level: 1.0,
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
            },
            ai_experience: AIExperienceConfig::default(),
        }
    }
}

impl Default for AIExperienceConfig {
    fn default() -> Self {
        Self {
            enable_session_title_generation: true,
            enable_welcome_panel_ai_analysis: false,
            enable_visual_mode: false,
        }
    }
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            id: "dark".to_string(),
            name: "dark".to_string(),
            display_name: "Dark Theme".to_string(),
            theme_type: "dark".to_string(),
            colors: ThemeColors::default(),
            fonts: ThemeFonts::default(),
            spacing: ThemeSpacing::default(),
            border_radius: ThemeBorderRadius::default(),
            shadows: ThemeShadows::default(),
        }
    }
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            primary: "#007acc".to_string(),
            secondary: "#6c757d".to_string(),
            background: "#1e1e1e".to_string(),
            surface: "#2d2d30".to_string(),
            text: "#cccccc".to_string(),
            text_secondary: "#969696".to_string(),
            border: "#3e3e42".to_string(),
            accent: "#007acc".to_string(),
            success: "#28a745".to_string(),
            warning: "#ffc107".to_string(),
            error: "#dc3545".to_string(),
        }
    }
}

impl Default for ThemeFonts {
    fn default() -> Self {
        Self {
            primary: "-apple-system, BlinkMacSystemFont, \"Segoe UI\", Roboto, sans-serif"
                .to_string(),
            code: "Consolas, \"Courier New\", monospace".to_string(),
            sizes: FontSizes::default(),
        }
    }
}

impl Default for FontSizes {
    fn default() -> Self {
        Self {
            xs: "0.75rem".to_string(),
            sm: "0.875rem".to_string(),
            base: "1rem".to_string(),
            lg: "1.125rem".to_string(),
            xl: "1.25rem".to_string(),
            xxl: "1.5rem".to_string(),
        }
    }
}

impl Default for ThemeSpacing {
    fn default() -> Self {
        Self {
            xs: "0.25rem".to_string(),
            sm: "0.5rem".to_string(),
            md: "1rem".to_string(),
            lg: "1.5rem".to_string(),
            xl: "2rem".to_string(),
        }
    }
}

impl Default for ThemeBorderRadius {
    fn default() -> Self {
        Self {
            sm: "0.125rem".to_string(),
            md: "0.25rem".to_string(),
            lg: "0.5rem".to_string(),
            full: "9999px".to_string(),
        }
    }
}

impl Default for ThemeShadows {
    fn default() -> Self {
        Self {
            sm: "0 1px 2px 0 rgba(0, 0, 0, 0.05)".to_string(),
            md: "0 4px 6px -1px rgba(0, 0, 0, 0.1)".to_string(),
            lg: "0 10px 15px -3px rgba(0, 0, 0, 0.1)".to_string(),
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
            theme: "vs-dark".to_string(),
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
            font_size: 14,
            font_family: "Consolas, \"Courier New\", monospace".to_string(),
            cursor_blink: true,
            cursor_style: "block".to_string(),
            scrollback: 1000,
            theme: TerminalThemeConfig::default(),
        }
    }
}

impl Default for TerminalThemeConfig {
    fn default() -> Self {
        Self {
            background: "#1e1e1e".to_string(),
            foreground: "#d4d4d4".to_string(),
            cursor: "#d4d4d4".to_string(),
            selection: "#264f78".to_string(),
            black: "#000000".to_string(),
            red: "#cd3131".to_string(),
            green: "#0dbc79".to_string(),
            yellow: "#e5e510".to_string(),
            blue: "#2472c8".to_string(),
            magenta: "#bc3fbc".to_string(),
            cyan: "#11a8cd".to_string(),
            white: "#e5e5e5".to_string(),
            bright_black: "#666666".to_string(),
            bright_red: "#f14c4c".to_string(),
            bright_green: "#23d18b".to_string(),
            bright_yellow: "#f5f543".to_string(),
            bright_blue: "#3b8eea".to_string(),
            bright_magenta: "#d670d6".to_string(),
            bright_cyan: "#29b8db".to_string(),
            bright_white: "#e5e5e5".to_string(),
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
            agent_models: std::collections::HashMap::new(),
            func_agent_models: std::collections::HashMap::new(),
            default_models: DefaultModelsConfig::default(),
            mode_configs: std::collections::HashMap::new(),
            subagent_configs: std::collections::HashMap::new(),
            proxy: ProxyConfig::default(),
            tool_execution_timeout_secs: default_tool_execution_timeout(),
            tool_confirmation_timeout_secs: default_tool_confirmation_timeout(),
            skip_tool_confirmation: false,
            debug_mode_config: DebugModeConfig::default(),
            known_tools: Vec::new(),
        }
    }
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            url: String::new(),
            username: None,
            password: None,
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
            api_key: String::new(),
            context_window: None,
            max_tokens: None,
            temperature: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            enabled: false,
            category: ModelCategory::GeneralChat,
            capabilities: vec![ModelCapability::TextChat],
            recommended_for: vec![],
            metadata: None,
            enable_thinking_process: false,
            support_preserved_thinking: false,
            custom_headers: None,
            custom_headers_mode: None,
            skip_ssl_verify: false,
            custom_request_body: None,
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
    /// Infers the model category from the model name and provider (for backward compatibility
    /// with older configs).
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

    /// Infers capability tags from the model category and name.
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

    /// Auto-completes missing category and capability information (used for configuration
    /// migration).
    pub fn ensure_category_and_capabilities(&mut self) {
        if self.category == ModelCategory::GeneralChat && self.capabilities.is_empty() {
            self.category = self.infer_category_from_model_name();
            self.capabilities = self.infer_capabilities_from_model();
        }

        if self.capabilities.is_empty() {
            self.capabilities = self.infer_capabilities_from_model();
        }
    }
}
