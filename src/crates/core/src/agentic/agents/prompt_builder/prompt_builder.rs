//! System prompts module providing main dialogue and agent dialogue prompts
use crate::agentic::util::get_formatted_files_list;
use crate::infrastructure::try_get_path_manager_arc;
use crate::service::ai_memory::AIMemoryManager;
use crate::service::ai_rules::get_global_ai_rules_service;
use crate::service::config::global::GlobalConfigManager;
use crate::service::project_context::ProjectContextService;
use crate::util::errors::{BitFunError, BitFunResult};
use log::{debug, warn};
use std::path::Path;

/// Placeholder constants
const PLACEHOLDER_ENV_INFO: &str = "{ENV_INFO}";
const PLACEHOLDER_PROJECT_LAYOUT: &str = "{PROJECT_LAYOUT}";
// PROJECT_CONTEXT_FILES needs configuration parsing
// const PLACEHOLDER_PROJECT_CONTEXT_FILES: &str = "{PROJECT_CONTEXT_FILES}";
const PLACEHOLDER_RULES: &str = "{RULES}";
const PLACEHOLDER_MEMORIES: &str = "{MEMORIES}";
const PLACEHOLDER_LANGUAGE_PREFERENCE: &str = "{LANGUAGE_PREFERENCE}";
const PLACEHOLDER_VISUAL_MODE: &str = "{VISUAL_MODE}";

pub struct PromptBuilder {
    pub workspace_path: String,
    pub file_tree_max_entries: usize,
}

impl PromptBuilder {
    pub fn new(workspace_path: &str) -> Self {
        Self {
            workspace_path: workspace_path.replace("\\", "/"),
            file_tree_max_entries: 200,
        }
    }

    /// Provide complete environment information
    pub fn get_env_info(&self) -> String {
        let os_name = std::env::consts::OS;
        let os_family = std::env::consts::FAMILY;
        let arch = std::env::consts::ARCH;

        let now = chrono::Local::now();
        let current_date = now.format("%A, %B %d, %Y").to_string();

        format!(
            r#"# Environment Information
<environment_details>
- Current Working Directory: {}
- Operating System: {} ({})
- Architecture: {}
- Current Date: {}
</environment_details>

"#,
            self.workspace_path, os_name, os_family, arch, current_date
        )
    }

    /// Get workspace file list
    pub fn get_project_layout(&self) -> String {
        let (hit_limit, formatted_files_list) =
            get_formatted_files_list(&self.workspace_path, self.file_tree_max_entries, None)
                .unwrap_or_else(|e| (false, format!("Error listing directory: {}", e)));
        let mut project_layout = "# Workspace Layout\n<project_layout>\n".to_string();
        if hit_limit {
            project_layout.push_str(&format!("Below is a snapshot of the current workspace's file structure (showing up to {} entries).\n\n", self.file_tree_max_entries));
        } else {
            project_layout
                .push_str("Below is a snapshot of the current workspace's file structure.\n\n");
        }
        project_layout.push_str(&formatted_files_list);
        project_layout.push_str("\n</project_layout>\n\n");
        project_layout
    }

    /// Get user-provided project information files
    /// These files (e.g., AGENTS.md, CLAUDE.md) are provided by users to describe project architecture, conventions, and guidelines
    ///
    /// Parameters:
    /// - filter: Optional filter, supports `include=category1,category2` or `exclude=category1`
    pub async fn get_project_context(&self, filter: Option<&str>) -> Option<String> {
        let service = ProjectContextService::new();
        let workspace = Path::new(&self.workspace_path);

        match service.build_context_prompt(workspace, filter).await {
            Ok(prompt) if !prompt.is_empty() => {
                let result = format!(
                    r#"# Project Context
The following are project documentation that describe the project's architecture, conventions, and guidelines, etc.
These files are maintained by the user and should NOT be modified unless explicitly requested.

{}

"#,
                    prompt
                );
                Some(result)
            }
            _ => None,
        }
    }

    /// Load AI memories from disk and format as prompt
    pub async fn load_ai_memories(&self) -> Option<String> {
        let path_manager = match try_get_path_manager_arc() {
            Ok(pm) => pm,
            Err(e) => {
                warn!("Failed to create PathManager: {}", e);
                return None;
            }
        };

        let memory_manager = match AIMemoryManager::new(path_manager).await {
            Ok(mm) => mm,
            Err(e) => {
                warn!("Failed to create AIMemoryManager: {}", e);
                return None;
            }
        };

        match memory_manager.get_memories_for_prompt().await {
            Ok(Some(prompt)) => Some(prompt),
            Ok(None) => None,
            Err(e) => {
                warn!("Failed to load memories: {}", e);
                None
            }
        }
    }

    /// Load AI rules from disk and format as prompt
    pub async fn load_ai_rules(&self) -> Option<String> {
        let rules_service = match get_global_ai_rules_service().await {
            Ok(service) => service,
            Err(e) => {
                warn!("Failed to get AIRulesService: {}", e);
                return None;
            }
        };

        let workspace_pathbuf = std::path::PathBuf::from(&self.workspace_path);
        if let Err(e) = rules_service.set_workspace(workspace_pathbuf).await {
            debug!("Failed to set workspace: {}", e);
        }

        match rules_service.build_system_prompt().await {
            Ok(prompt) => {
                if prompt.is_empty() {
                    None
                } else {
                    Some(prompt)
                }
            }
            Err(e) => {
                warn!("Failed to build AI rules system prompt: {}", e);
                None
            }
        }
    }

    /// Get visual mode instruction from user config
    ///
    /// Reads `app.ai_experience.enable_visual_mode` from global config.
    /// Returns a prompt snippet when enabled, or empty string when disabled.
    async fn get_visual_mode_instruction(&self) -> String {
        let enabled = match GlobalConfigManager::get_service().await {
            Ok(service) => service
                .get_config::<bool>(Some("app.ai_experience.enable_visual_mode"))
                .await
                .unwrap_or(false),
            Err(e) => {
                debug!("Failed to read visual mode config: {}", e);
                false
            }
        };

        if enabled {
            r"# Visualizing complex logic as you explain
Use Mermaid diagrams to visualize complex logic, workflows, architectures, and data flows whenever it helps clarify the explanation.
Prefer MermaidInteractive tool when available, otherwise output Mermaid code blocks directly.
".to_string()
        } else {
            String::new()
        }
    }

    /// Get user language preference instruction
    ///
    /// Read app.language from global config, generate simple language instruction
    /// Returns empty string if config cannot be read
    /// Returns error if language code is unsupported
    async fn get_language_preference(&self) -> BitFunResult<String> {
        let language_code = GlobalConfigManager::get_service()
            .await?
            .get_config::<String>(Some("app.language"))
            .await?;

        Self::format_language_instruction(&language_code)
    }

    /// Format language instruction based on language code
    fn format_language_instruction(lang_code: &str) -> BitFunResult<String> {
        let language = match lang_code {
            "zh-CN" => "**Simplified Chinese**",
            "en-US" => "**English**",
            _ => {
                return Err(BitFunError::config(format!(
                    "Unknown language code: {}",
                    lang_code
                )));
            }
        };
        Ok(format!("# Language Preference\nYou MUST respond in {} regardless of the user's input language. This is the system language setting and should be followed unless the user explicitly specifies a different language. This is crucial for smooth communication and user experience\n", language))
    }

    /// Build prompt from template, automatically fill content based on placeholders
    ///
    /// Supported placeholders:
    /// - `{LANGUAGE_PREFERENCE}` - User language preference (read from global config)
    /// - `{ENV_INFO}` - Environment information
    /// - `{PROJECT_LAYOUT}` - Project file layout
    /// - `{PROJECT_CONTEXT_FILES}` - Project context files (AGENTS.md, CLAUDE.md, etc.)
    /// - `{RULES}` - AI rules
    /// - `{MEMORIES}` - AI memories
    /// - `{VISUAL_MODE}` - Visual mode instruction (Mermaid diagrams, read from global config)
    ///
    /// If a placeholder is not in the template, corresponding content will not be added
    pub async fn build_prompt_from_template(&self, template: &str) -> BitFunResult<String> {
        let mut result = template.to_string();

        // Replace {LANGUAGE_PREFERENCE}
        if result.contains(PLACEHOLDER_LANGUAGE_PREFERENCE) {
            let language_preference = self.get_language_preference().await?;
            result = result.replace(PLACEHOLDER_LANGUAGE_PREFERENCE, &language_preference);
        }

        // Replace {ENV_INFO}
        if result.contains(PLACEHOLDER_ENV_INFO) {
            let env_info = self.get_env_info();
            result = result.replace(PLACEHOLDER_ENV_INFO, &env_info);
        }

        // Replace {PROJECT_LAYOUT}
        if result.contains(PLACEHOLDER_PROJECT_LAYOUT) {
            let project_layout = self.get_project_layout();
            result = result.replace(PLACEHOLDER_PROJECT_LAYOUT, &project_layout);
        }

        // Replace {PROJECT_CONTEXT_FILES}
        // Supported syntax:
        // - {PROJECT_CONTEXT_FILES} - Include all enabled documents
        // - {PROJECT_CONTEXT_FILES:include=general,design} - Only include specified categories
        // - {PROJECT_CONTEXT_FILES:exclude=review} - Exclude specified categories
        while let Some(start) = result.find("{PROJECT_CONTEXT_FILES") {
            let start_pos = start;
            // Find placeholder end position
            let end_pos = result[start_pos..]
                .find('}')
                .map(|p| start_pos + p + 1)
                .unwrap_or(result.len());

            // Extract complete placeholder
            let placeholder = &result[start_pos..end_pos];

            // Parse filter
            let filter = if let Some(colon_pos) = placeholder.find(':') {
                // Has filter: {PROJECT_CONTEXT_FILES:include=xxx} or {PROJECT_CONTEXT_FILES:exclude=xxx}
                let filter_str = &placeholder[colon_pos + 1..placeholder.len() - 1];
                Some(filter_str.trim().to_string())
            } else {
                // No filter
                None
            };

            let filter_ref = filter.as_deref();
            let project_context = self
                .get_project_context(filter_ref)
                .await
                .unwrap_or_default();

            result = result.replace(placeholder, &project_context);
        }

        // Replace {RULES}
        if result.contains(PLACEHOLDER_RULES) {
            let rules = self.load_ai_rules().await.unwrap_or_default();
            result = result.replace(PLACEHOLDER_RULES, &rules);
        }

        // Replace {MEMORIES}
        if result.contains(PLACEHOLDER_MEMORIES) {
            let memories = self.load_ai_memories().await.unwrap_or_default();
            result = result.replace(PLACEHOLDER_MEMORIES, &memories);
        }

        // Replace {VISUAL_MODE}
        if result.contains(PLACEHOLDER_VISUAL_MODE) {
            let visual_mode = self.get_visual_mode_instruction().await;
            result = result.replace(PLACEHOLDER_VISUAL_MODE, &visual_mode);
        }

        Ok(result.trim().to_string())
    }
}
