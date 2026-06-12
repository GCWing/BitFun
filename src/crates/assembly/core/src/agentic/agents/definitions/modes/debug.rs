//! Debug Mode
//!
//! Shared coding modes inherit the agentic baseline for
//! `SHARED_CODING_MODE_PROMPT_TEMPLATE`, `shared_coding_mode_tools()`,
//! `shared_coding_mode_tool_exposure_overrides()`, and
//! `shared_coding_mode_user_context_policy()`. Across the four coding modes,
//! only the reminder content differs.

use crate::agentic::agents::{
    get_embedded_prompt, shared_coding_mode_tool_exposure_overrides, shared_coding_mode_tools,
    shared_coding_mode_user_context_policy, Agent, AgentToolPolicyOverrides, UserContextPolicy,
    SHARED_CODING_MODE_PROMPT_TEMPLATE,
};
use crate::service::config::global::GlobalConfigManager;
use crate::service::config::types::{DebugModeConfig, LanguageDebugTemplate};
use crate::service::lsp::project_detector::{ProjectDetector, ProjectInfo};
use crate::util::errors::BitFunResult;
use async_trait::async_trait;
use log::debug;
use std::path::Path;

pub struct DebugMode {
    default_tools: Vec<String>,
    tool_exposure_overrides: AgentToolPolicyOverrides,
}

const DEBUG_MODE_FIRST_ENTRY_REMINDER_TEMPLATE: &str = "debug_mode_first_entry_reminder";
const DEBUG_MODE_ONGOING_REMINDER_TEMPLATE: &str = "debug_mode_ongoing_reminder";

impl Default for DebugMode {
    fn default() -> Self {
        Self::new()
    }
}

impl DebugMode {
    pub fn new() -> Self {
        Self {
            default_tools: shared_coding_mode_tools(),
            tool_exposure_overrides: shared_coding_mode_tool_exposure_overrides(),
        }
    }

    async fn get_debug_config(&self) -> DebugModeConfig {
        if let Ok(config_service) = GlobalConfigManager::get_service().await {
            config_service
                .get_config::<DebugModeConfig>(Some("ai.debug_mode_config"))
                .await
                .unwrap_or_default()
        } else {
            DebugModeConfig::default()
        }
    }

    async fn detect_project_info(&self, workspace_path: &str) -> ProjectInfo {
        let path = Path::new(workspace_path);
        ProjectDetector::detect(path).await.unwrap_or_default()
    }

    fn load_reminder_template(&self, template_name: &str) -> BitFunResult<String> {
        get_embedded_prompt(template_name)
            .map(str::to_string)
            .ok_or_else(|| {
                crate::util::errors::BitFunError::Agent(format!(
                    "{} not found in embedded files",
                    template_name
                ))
            })
    }

    const BUILTIN_JS_TEMPLATE: &'static str = r#"fetch('http://127.0.0.1:{PORT}/ingest/{SESSION_ID}',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({location:'{LOCATION}',message:'{MESSAGE}',data:{DATA},timestamp:Date.now(),sessionId:'{SESSION_ID}',hypothesisId:'{HYPOTHESIS_ID}',runId:'{RUN_ID}'})}).catch(()=>{});"#;

    /// Generates language-specific instrumentation templates based on detected languages.
    fn build_language_templates_prompt(
        config: &DebugModeConfig,
        detected_languages: &[String],
    ) -> String {
        let mut output = String::new();

        let is_web_project = detected_languages.iter().any(|lang| {
            let l = lang.to_lowercase();
            l == "javascript" || l == "typescript"
        });

        let has_other_languages = detected_languages.iter().any(|lang| {
            let l = lang.to_lowercase();
            l != "javascript" && l != "typescript"
        });

        let user_other_templates: Vec<_> = config
            .language_templates
            .iter()
            .filter(|(lang, template)| {
                *lang != "javascript"
                    && template.enabled
                    && !template.instrumentation_template.trim().is_empty()
            })
            .collect();

        if is_web_project {
            let use_custom = config
                .language_templates
                .get("javascript")
                .map(|t| t.enabled && !t.instrumentation_template.trim().is_empty())
                .unwrap_or(false);

            if use_custom {
                if let Some(template) = config.language_templates.get("javascript") {
                    output.push_str(&Self::render_template(template, config));
                }
            } else {
                output.push_str(&Self::render_builtin_js_template(config));
            }
        }

        if has_other_languages {
            let matched_user_templates: Vec<_> = user_other_templates
                .iter()
                .filter(|(lang, _)| {
                    detected_languages
                        .iter()
                        .any(|detected| detected.to_lowercase() == lang.to_lowercase())
                })
                .collect();

            if !matched_user_templates.is_empty() {
                for (_, template) in matched_user_templates {
                    output.push_str(&Self::render_template(template, config));
                }
            } else {
                output.push_str(&Self::render_general_guidelines(config));
            }
        } else if !is_web_project {
            if !user_other_templates.is_empty() {
                for (_language, template) in &user_other_templates {
                    output.push_str(&Self::render_template(template, config));
                }
            } else {
                output.push_str(&Self::render_general_guidelines(config));
            }
        }

        output
    }

    fn render_builtin_js_template(config: &DebugModeConfig) -> String {
        let mut section = "## JavaScript / TypeScript Instrumentation\n\n".to_string();
        section.push_str("```javascript\n");
        section.push_str("// #region agent log\n");
        section.push_str(
            &Self::BUILTIN_JS_TEMPLATE
                .replace("{PORT}", &config.ingest_port.to_string())
                .replace("{SESSION_ID}", "debug-session")
                .replace("{HYPOTHESIS_ID}", "X")
                .replace("{RUN_ID}", "pre-fix"),
        );
        section.push_str("\n// #endregion\n```\n\n");
        section.push_str("**JavaScript / TypeScript Notes:**\n");
        section.push_str("- Sends logs via HTTP POST to ingest server\n");
        section.push_str("- Replace {DATA} with a JavaScript object expression\n\n");
        section
    }

    fn render_template(template: &LanguageDebugTemplate, config: &DebugModeConfig) -> String {
        if template.instrumentation_template.trim().is_empty() {
            return String::new();
        }

        let lang_hint = match template.language.as_str() {
            "javascript" => "javascript",
            "typescript" => "typescript",
            "python" => "python",
            "rust" => "rust",
            "go" => "go",
            "java" => "java",
            "cpp" => "cpp",
            _ => "text",
        };

        let mut section = format!("## {} Instrumentation\n\n", template.display_name);
        section.push_str("```");
        section.push_str(lang_hint);
        section.push('\n');
        section.push_str(&template.region_start);
        section.push('\n');
        section.push_str(
            &template
                .instrumentation_template
                .replace("{PORT}", &config.ingest_port.to_string())
                .replace("{LOG_PATH}", &config.log_path)
                .replace("{SESSION_ID}", "debug-session")
                .replace("{HYPOTHESIS_ID}", "X")
                .replace("{RUN_ID}", "pre-fix"),
        );
        section.push('\n');
        section.push_str(&template.region_end);
        section.push_str("\n```\n\n");

        if !template.notes.is_empty() {
            section.push_str(&format!("**{} Notes:**\n", template.display_name));
            for note in &template.notes {
                section.push_str(&format!("- {}\n", note));
            }
            section.push('\n');
        }

        section
    }

    fn resolve_log_path(config: &DebugModeConfig, workspace_path: &str) -> String {
        if config.log_path.starts_with('/') || config.log_path.starts_with('.') {
            config.log_path.clone()
        } else {
            format!("{}/{}", workspace_path, config.log_path)
        }
    }

    /// Builds session-level reminder details with dynamic values like server endpoint and log path.
    fn build_session_level_rule(&self, config: &DebugModeConfig, workspace_path: &str) -> String {
        let log_path = Self::resolve_log_path(config, workspace_path);

        format!(
            r#"
# Mode-Specific Configuration (Session Level)

The NDJSON ingest server is running and ready to receive debug logs.

**Server endpoint**: `http://127.0.0.1:{port}/ingest/debug-session`
**Log path**: `{log_path}`

Use these exact values when inserting instrumentation code. The server automatically writes received logs to the log path in NDJSON format.

"#,
            port = config.ingest_port,
            log_path = log_path
        )
    }

    /// Builds the ongoing reminder appended after each dialog turn while staying in Debug mode.
    fn build_ongoing_reminder(&self) -> BitFunResult<String> {
        self.load_reminder_template(DEBUG_MODE_ONGOING_REMINDER_TEMPLATE)
    }

    fn build_first_entry_reminder(
        &self,
        debug_config: &DebugModeConfig,
        project_info: &ProjectInfo,
        workspace_path: &str,
    ) -> BitFunResult<String> {
        let reminder_template =
            self.load_reminder_template(DEBUG_MODE_FIRST_ENTRY_REMINDER_TEMPLATE)?;

        let language_templates =
            Self::build_language_templates_prompt(debug_config, &project_info.languages);

        debug!(
            "Debug mode language templates length: {}",
            language_templates.len()
        );

        let log_path = Self::resolve_log_path(debug_config, workspace_path);
        let session_rule = self.build_session_level_rule(debug_config, workspace_path);

        Ok(reminder_template
            .replace("{LOG_PATH}", &log_path)
            .replace("{INGEST_PORT}", &debug_config.ingest_port.to_string())
            .replace("{LANGUAGE_TEMPLATES}", &language_templates)
            + &session_rule)
    }

    /// Renders general instrumentation guidelines for non-web projects.
    fn render_general_guidelines(config: &DebugModeConfig) -> String {
        format!(
            r#"## General Instrumentation Guidelines

In **non-JavaScript languages** (Python, Go, Rust, Java, C, C++, Ruby, etc.), instrument by opening the **log path** in append mode using standard library file I/O, writing a single NDJSON line with your payload, and then closing the file. Keep these snippets as tiny and compact as possible (ideally one line, or just a few).

**Log path:** `{log_path}`

**Log Format (NDJSON - one JSON object per line):**
- `location`: file path and line number (e.g., "src/main.rs:42")
- `message`: brief description of what is being logged
- `data`: runtime values you want to inspect
- `timestamp`: current time in milliseconds since epoch
- `sessionId`: use "debug-session"
- `hypothesisId`: the hypothesis ID (A, B, C, etc.)
- `runId`: "pre-fix" or "post-fix"

**Region Markers:**
Wrap all instrumentation code so it can be easily removed later:
```
// #region agent log
<your compact logging code here>
// #endregion
```

**Example log entry:**
```json
{{"location":"src/handler.rs:128","message":"checking user status","data":{{"userId":"abc","status":"active"}},"timestamp":1704000000000,"sessionId":"debug-session","hypothesisId":"A","runId":"pre-fix"}}
```

**What to log:**
- Function entry/exit with parameters and return values
- Branch decisions (which if/match arm was taken)
- State mutations (before and after values)
- Error conditions and exception details

**Safety:**
- Do NOT log secrets (passwords, tokens, API keys, PII)
- Safe to log: types, lengths, prefixes, flags, IDs, counts

"#,
            log_path = config.log_path
        )
    }
}

#[async_trait]
impl Agent for DebugMode {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "debug"
    }

    fn name(&self) -> &str {
        "Debug"
    }

    fn description(&self) -> &str {
        "Evidence-driven debugging: form hypotheses, gather runtime evidence with logs, and fix with 100% confidence"
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        SHARED_CODING_MODE_PROMPT_TEMPLATE
    }

    fn user_context_policy(&self) -> UserContextPolicy {
        shared_coding_mode_user_context_policy()
    }

    fn tool_exposure_overrides(&self) -> &AgentToolPolicyOverrides {
        &self.tool_exposure_overrides
    }

    async fn get_system_reminder(
        &self,
        previous_agent_type: Option<&str>,
        workspace: Option<&crate::agentic::WorkspaceBinding>,
    ) -> BitFunResult<String> {
        if previous_agent_type == Some(self.id()) {
            return self.build_ongoing_reminder();
        }

        let workspace_path = workspace
            .map(|binding| binding.root_path_string())
            .unwrap_or_else(|| ".".to_string());
        let debug_config = self.get_debug_config().await;
        let project_info = self.detect_project_info(&workspace_path).await;

        debug!(
            "Debug mode project detection: languages={:?}, types={:?}",
            project_info.languages, project_info.project_types
        );

        self.build_first_entry_reminder(&debug_config, &project_info, &workspace_path)
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn is_readonly(&self) -> bool {
        false
    }
}
