//! Debug Mode - Evidence-driven debugging mode

use super::prompt_builder::PromptBuilder;
use super::Agent;
use crate::service::config::global::GlobalConfigManager;
use crate::service::config::types::{DebugModeConfig, LanguageDebugTemplate};
use crate::service::lsp::project_detector::{ProjectDetector, ProjectInfo};
use crate::util::errors::BitFunResult;
use async_trait::async_trait;
use log::debug;
use std::path::Path;

pub struct DebugMode;

include!(concat!(env!("OUT_DIR"), "/embedded_agents_prompt.rs"));

impl DebugMode {
    pub fn new() -> Self {
        Self
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
        section.push_str("\n");
        section.push_str(&template.region_start);
        section.push_str("\n");
        section.push_str(
            &template
                .instrumentation_template
                .replace("{PORT}", &config.ingest_port.to_string())
                .replace("{LOG_PATH}", &config.log_path)
                .replace("{SESSION_ID}", "debug-session")
                .replace("{HYPOTHESIS_ID}", "X")
                .replace("{RUN_ID}", "pre-fix"),
        );
        section.push_str("\n");
        section.push_str(&template.region_end);
        section.push_str("\n```\n\n");

        if !template.notes.is_empty() {
            section.push_str(&format!("**{} Notes:**\n", template.display_name));
            for note in &template.notes {
                section.push_str(&format!("- {}\n", note));
            }
            section.push_str("\n");
        }

        section
    }

    /// Builds session-level configuration with dynamic values like server endpoint and log path.
    fn build_session_level_rule(&self, config: &DebugModeConfig, workspace_path: &str) -> String {
        let log_path = if config.log_path.starts_with('/') || config.log_path.starts_with('.') {
            config.log_path.clone()
        } else {
            format!("{}/{}", workspace_path, config.log_path)
        };

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

    /// Builds a system reminder appended after each dialog turn.
    fn build_system_reminder(&self) -> String {
        r#"Debug mode is still active. You must debug with **runtime evidence**.

**Before each run:** Use Delete tool to clear the log file, do not use shell commands like rm, touch, etc.
**During fixes:** Do NOT remove instrumentation until user confirms success with post-fix verification logs.
**If fix failed:** Generate NEW hypotheses from different subsystems and add more instrumentation. You MUST conclude your response with the `<reproduction_steps>` block unless the issue is fixed."#.to_string()
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

    fn prompt_template_name(&self) -> &str {
        "debug_mode"
    }

    async fn build_prompt(&self, workspace_path: &str) -> BitFunResult<String> {
        let prompt_components = PromptBuilder::new(workspace_path);
        let env_info = prompt_components.get_env_info();

        let debug_config = self.get_debug_config().await;
        let project_info = self.detect_project_info(workspace_path).await;

        debug!(
            "Debug mode project detection: languages={:?}, types={:?}",
            project_info.languages, project_info.project_types
        );

        let system_prompt_template = get_embedded_prompt("debug_mode")
            .unwrap_or("Debug mode prompt not found in embedded files");

        let language_templates =
            Self::build_language_templates_prompt(&debug_config, &project_info.languages);

        let main_prompt = system_prompt_template
            .replace("{ENV_INFO}", &env_info)
            .replace("{LOG_PATH}", &debug_config.log_path)
            .replace("{INGEST_PORT}", &debug_config.ingest_port.to_string())
            .replace("{LANGUAGE_TEMPLATES}", &language_templates);

        let mut prompt_list = vec![main_prompt];

        debug!(
            "Debug mode language templates length: {}",
            language_templates.len()
        );

        let project_layout = prompt_components.get_project_layout();
        prompt_list.push(format!(
            r##"# Current Workspace File Structure
<project_layout>
Below is a snapshot of the current workspace's file structure.

{project_layout}
</project_layout>

"##
        ));

        if let Some(project_context) = prompt_components.get_project_context(None).await {
            prompt_list.push(format!(
                "# Current Workspace Context\n{project_context}\n\n"
            ));
        }

        if let Some(rules_prompt) = prompt_components.load_ai_rules().await {
            prompt_list.push(rules_prompt);
        }

        if let Some(memory_prompt) = prompt_components.load_ai_memories().await {
            prompt_list.push(memory_prompt);
        }

        let session_rule = self.build_session_level_rule(&debug_config, workspace_path);
        prompt_list.push(session_rule);

        Ok(prompt_list.join(""))
    }

    async fn get_system_reminder(&self, _index: usize) -> BitFunResult<String> {
        Ok(self.build_system_reminder())
    }

    fn default_tools(&self) -> Vec<String> {
        vec![
            "Read".to_string(),
            "Write".to_string(),
            "Edit".to_string(),
            "Delete".to_string(),
            "Bash".to_string(),
            "Grep".to_string(),
            "Glob".to_string(),
            "WebSearch".to_string(),
            "TodoWrite".to_string(),
            "IdeControl".to_string(),
            "MermaidInteractive".to_string(),
            "Log".to_string(),
            "ReadLints".to_string(),
        ]
    }

    fn is_readonly(&self) -> bool {
        false
    }
}
