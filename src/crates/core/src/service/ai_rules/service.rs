//! AI rules management service implementation
//!
//! Rule management based on the `.mdc` file format.

use super::types::*;
use crate::infrastructure::{try_get_path_manager_arc, PathManager};
use crate::util::errors::*;
use globset::{Glob, GlobSetBuilder};
use log::{debug, info, warn};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::RwLock;

/// Global `AIRulesService` singleton container.
static GLOBAL_AI_RULES_SERVICE: OnceLock<Arc<RwLock<Option<Arc<AIRulesService>>>>> =
    OnceLock::new();

/// Initializes the global `AIRulesService` singleton.
/// Must be called before use, typically during application startup.
pub async fn initialize_global_ai_rules_service() -> BitFunResult<()> {
    if is_global_ai_rules_service_initialized() {
        debug!("Global AIRulesService already initialized, skipping");
        return Ok(());
    }

    info!("Initializing global AIRulesService");

    let path_manager = try_get_path_manager_arc()
        .map_err(|e| BitFunError::service(format!("Failed to create PathManager: {}", e)))?;

    let service = AIRulesService::new(path_manager).await?;
    let wrapper = Arc::new(RwLock::new(Some(Arc::new(service))));

    GLOBAL_AI_RULES_SERVICE.set(wrapper).map_err(|_| {
        BitFunError::service("Failed to initialize global AIRulesService".to_string())
    })?;

    info!("Global AIRulesService initialized successfully");
    Ok(())
}

/// Gets the global `AIRulesService` singleton.
pub async fn get_global_ai_rules_service() -> BitFunResult<Arc<AIRulesService>> {
    let wrapper = GLOBAL_AI_RULES_SERVICE.get()
        .ok_or_else(|| BitFunError::service(
            "Global AIRulesService not initialized. Call initialize_global_ai_rules_service() first.".to_string()
        ))?;

    let guard = wrapper.read().await;
    guard
        .as_ref()
        .ok_or_else(|| BitFunError::service("Global AIRulesService is None".to_string()))
        .map(Arc::clone)
}

/// Returns whether the global singleton has been initialized.
pub fn is_global_ai_rules_service_initialized() -> bool {
    GLOBAL_AI_RULES_SERVICE
        .get()
        .map(|w| {
            if let Ok(guard) = w.try_read() {
                guard.is_some()
            } else {
                false
            }
        })
        .unwrap_or(false)
}

/// File rule match result
#[derive(Debug, Clone)]
pub struct FileRulesResult {
    /// Number of matched rules
    pub matched_count: usize,
    /// Formatted rule content (for appending to file read results)
    pub formatted_content: Option<String>,
}

/// AI rules management service
pub struct AIRulesService {
    /// Path manager
    path_manager: Arc<PathManager>,

    /// User-level rule cache
    user_rules: Arc<RwLock<Vec<AIRule>>>,

    /// Project-level rule cache
    project_rules: Arc<RwLock<Vec<AIRule>>>,

    /// Current workspace path
    workspace_path: Arc<RwLock<Option<PathBuf>>>,
}

impl AIRulesService {
    /// Creates a new rules service.
    pub async fn new(path_manager: Arc<PathManager>) -> BitFunResult<Self> {
        let service = Self {
            path_manager,
            user_rules: Arc::new(RwLock::new(Vec::new())),
            project_rules: Arc::new(RwLock::new(Vec::new())),
            workspace_path: Arc::new(RwLock::new(None)),
        };

        service.reload_user_rules().await?;

        Ok(service)
    }

    /// Sets the workspace path and loads project-level rules.
    pub async fn set_workspace(&self, workspace_path: PathBuf) -> BitFunResult<()> {
        *self.workspace_path.write().await = Some(workspace_path);
        self.reload_project_rules().await?;
        Ok(())
    }

    /// Clears the workspace.
    pub async fn clear_workspace(&self) {
        *self.workspace_path.write().await = None;
        self.project_rules.write().await.clear();
    }

    /// Returns all user-level rules.
    pub async fn get_user_rules(&self) -> BitFunResult<Vec<AIRule>> {
        Ok(self.user_rules.read().await.clone())
    }

    /// Returns a single user-level rule.
    pub async fn get_user_rule(&self, name: &str) -> BitFunResult<Option<AIRule>> {
        let rules = self.user_rules.read().await;
        Ok(rules.iter().find(|r| r.name == name).cloned())
    }

    /// Creates a user-level rule.
    pub async fn create_user_rule(&self, request: CreateRuleRequest) -> BitFunResult<AIRule> {
        let rules_dir = self.path_manager.user_rules_dir();
        let rule_name = request.name.clone();
        self.create_rule_internal(&rules_dir, RuleLevel::User, request)
            .await?;
        self.reload_user_rules().await?;

        self.get_user_rule(&rule_name)
            .await?
            .ok_or_else(|| BitFunError::service("Failed to create rule".to_string()))
    }

    /// Updates a user-level rule.
    pub async fn update_user_rule(
        &self,
        name: &str,
        request: UpdateRuleRequest,
    ) -> BitFunResult<AIRule> {
        let rules_dir = self.path_manager.user_rules_dir();
        self.update_rule_internal(&rules_dir, name, request.clone())
            .await?;
        self.reload_user_rules().await?;

        let new_name = request.name.as_deref().unwrap_or(name);
        self.get_user_rule(new_name)
            .await?
            .ok_or_else(|| BitFunError::service("Failed to update rule".to_string()))
    }

    /// Deletes a user-level rule.
    pub async fn delete_user_rule(&self, name: &str) -> BitFunResult<bool> {
        let rules_dir = self.path_manager.user_rules_dir();
        let result = self.delete_rule_internal(&rules_dir, name).await?;
        self.reload_user_rules().await?;
        Ok(result)
    }

    /// Reloads user-level rules.
    pub async fn reload_user_rules(&self) -> BitFunResult<()> {
        let rules_dir = self.path_manager.user_rules_dir();
        let rules = self
            .load_rules_from_dir(&rules_dir, RuleLevel::User)
            .await?;
        *self.user_rules.write().await = rules;
        Ok(())
    }

    /// Returns user-level rule statistics.
    pub async fn get_user_rules_stats(&self) -> BitFunResult<RuleStats> {
        let rules = self.user_rules.read().await;
        Ok(Self::calculate_stats(&rules))
    }

    /// Returns all project-level rules.
    pub async fn get_project_rules(&self) -> BitFunResult<Vec<AIRule>> {
        Ok(self.project_rules.read().await.clone())
    }

    /// Returns all project-level rules for the specified workspace.
    pub async fn get_project_rules_for_workspace(
        &self,
        workspace: &Path,
    ) -> BitFunResult<Vec<AIRule>> {
        self.load_project_rules_for_workspace(workspace).await
    }

    /// Returns a single project-level rule.
    pub async fn get_project_rule(&self, name: &str) -> BitFunResult<Option<AIRule>> {
        let rules = self.project_rules.read().await;
        Ok(rules.iter().find(|r| r.name == name).cloned())
    }

    /// Returns a single project-level rule for the specified workspace.
    pub async fn get_project_rule_for_workspace(
        &self,
        workspace: &Path,
        name: &str,
    ) -> BitFunResult<Option<AIRule>> {
        let rules = self.load_project_rules_for_workspace(workspace).await?;
        Ok(rules.into_iter().find(|r| r.name == name))
    }

    /// Creates a project-level rule.
    pub async fn create_project_rule(&self, request: CreateRuleRequest) -> BitFunResult<AIRule> {
        let workspace_path = self.require_workspace_path().await?;
        self.create_project_rule_for_workspace(&workspace_path, request)
            .await
    }

    /// Creates a project-level rule for the specified workspace.
    pub async fn create_project_rule_for_workspace(
        &self,
        workspace: &Path,
        request: CreateRuleRequest,
    ) -> BitFunResult<AIRule> {
        let rules_dir = self.path_manager.project_rules_dir(workspace);
        let rule_name = request.name.clone();
        self.create_rule_internal(&rules_dir, RuleLevel::Project, request)
            .await?;
        let rules = self.load_project_rules_for_workspace(workspace).await?;
        self.sync_project_rules_cache_if_current(workspace, &rules)
            .await;

        rules
            .into_iter()
            .find(|rule| rule.name == rule_name)
            .ok_or_else(|| BitFunError::service("Failed to create rule".to_string()))
    }

    /// Updates a project-level rule.
    /// Supports rules from both the BitFun and Cursor directories.
    pub async fn update_project_rule(
        &self,
        name: &str,
        request: UpdateRuleRequest,
    ) -> BitFunResult<AIRule> {
        let workspace_path = self.require_workspace_path().await?;
        self.update_project_rule_for_workspace(&workspace_path, name, request)
            .await
    }

    /// Updates a project-level rule for the specified workspace.
    pub async fn update_project_rule_for_workspace(
        &self,
        workspace: &Path,
        name: &str,
        request: UpdateRuleRequest,
    ) -> BitFunResult<AIRule> {
        let rule = self
            .get_project_rule_for_workspace(workspace, name)
            .await?
            .ok_or_else(|| BitFunError::service(format!("Rule '{}' not found", name)))?;

        let rules_dir = rule
            .file_path
            .parent()
            .ok_or_else(|| BitFunError::service("Invalid rule file path".to_string()))?;

        self.update_rule_internal(rules_dir, name, request.clone())
            .await?;
        let rules = self.load_project_rules_for_workspace(workspace).await?;
        self.sync_project_rules_cache_if_current(workspace, &rules)
            .await;

        let new_name = request.name.as_deref().unwrap_or(name);
        rules
            .into_iter()
            .find(|rule| rule.name == new_name)
            .ok_or_else(|| BitFunError::service("Failed to update rule".to_string()))
    }

    /// Deletes a project-level rule.
    /// Supports rules from both the BitFun and Cursor directories.
    pub async fn delete_project_rule(&self, name: &str) -> BitFunResult<bool> {
        let workspace_path = self.require_workspace_path().await?;
        self.delete_project_rule_for_workspace(&workspace_path, name)
            .await
    }

    /// Deletes a project-level rule for the specified workspace.
    /// Supports rules from both the BitFun and Cursor directories.
    pub async fn delete_project_rule_for_workspace(
        &self,
        workspace: &Path,
        name: &str,
    ) -> BitFunResult<bool> {
        let rule = self
            .get_project_rule_for_workspace(workspace, name)
            .await?
            .ok_or_else(|| BitFunError::service(format!("Rule '{}' not found", name)))?;

        let rules_dir = rule
            .file_path
            .parent()
            .ok_or_else(|| BitFunError::service("Invalid rule file path".to_string()))?;

        let result = self.delete_rule_internal(rules_dir, name).await?;
        let rules = self.load_project_rules_for_workspace(workspace).await?;
        self.sync_project_rules_cache_if_current(workspace, &rules)
            .await;
        Ok(result)
    }

    /// Reloads project-level rules.
    /// Loads BitFun rules first, then Cursor rules; for duplicates, the first loaded wins.
    pub async fn reload_project_rules(&self) -> BitFunResult<()> {
        let workspace_path = self.workspace_path.read().await.clone();

        if let Some(workspace) = workspace_path {
            let all_rules = self.load_project_rules_for_workspace(&workspace).await?;
            *self.project_rules.write().await = all_rules;
        } else {
            self.project_rules.write().await.clear();
        }

        Ok(())
    }

    /// Reloads project-level rules for the specified workspace.
    pub async fn reload_project_rules_for_workspace(&self, workspace: &Path) -> BitFunResult<()> {
        let rules = self.load_project_rules_for_workspace(workspace).await?;
        self.sync_project_rules_cache_if_current(workspace, &rules)
            .await;
        Ok(())
    }

    /// Returns project-level rule statistics.
    pub async fn get_project_rules_stats(&self) -> BitFunResult<RuleStats> {
        let rules = self.project_rules.read().await;
        Ok(Self::calculate_stats(&rules))
    }

    /// Returns project-level rule statistics for the specified workspace.
    pub async fn get_project_rules_stats_for_workspace(
        &self,
        workspace: &Path,
    ) -> BitFunResult<RuleStats> {
        let rules = self.load_project_rules_for_workspace(workspace).await?;
        Ok(Self::calculate_stats(&rules))
    }

    async fn load_project_rules_for_workspace(
        &self,
        workspace: &Path,
    ) -> BitFunResult<Vec<AIRule>> {
        let mut all_rules = Vec::new();
        let mut loaded_names = std::collections::HashSet::new();

        let bitfun_rules_dir = self.path_manager.project_rules_dir(workspace);
        let bitfun_rules = self
            .load_rules_from_dir(&bitfun_rules_dir, RuleLevel::Project)
            .await?;

        for rule in bitfun_rules {
            loaded_names.insert(rule.name.clone());
            all_rules.push(rule);
        }

        let cursor_rules_dir = workspace.join(".cursor").join("rules");
        if cursor_rules_dir.exists() {
            let cursor_rules = self
                .load_rules_from_dir(&cursor_rules_dir, RuleLevel::Project)
                .await?;

            for rule in cursor_rules {
                if !loaded_names.contains(&rule.name) {
                    loaded_names.insert(rule.name.clone());
                    all_rules.push(rule);
                } else {
                    debug!(
                        "Skipping Cursor rule '{}' (already loaded from BitFun)",
                        rule.name
                    );
                }
            }
        }

        all_rules.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(all_rules)
    }

    fn format_system_prompt(&self, user_rules: &[AIRule], project_rules: &[AIRule]) -> String {
        if user_rules.is_empty() && project_rules.is_empty() {
            return String::new();
        }

        let apply_intelligently_rules: Vec<_> = project_rules
            .iter()
            .filter(|r| r.enabled && r.apply_type == RuleApplyType::ApplyIntelligently)
            .collect();

        let always_apply_rules: Vec<_> = project_rules
            .iter()
            .filter(|r| r.enabled && r.apply_type == RuleApplyType::AlwaysApply)
            .collect();

        let enabled_user_rules: Vec<_> = user_rules.iter().filter(|r| r.enabled).collect();

        if always_apply_rules.is_empty()
            && apply_intelligently_rules.is_empty()
            && enabled_user_rules.is_empty()
        {
            return String::new();
        }

        let mut prompt = r#"# Rules
<rules>
The rules section has a number of possible rules/memories/context that you should consider. In each subsection, we provide instructions about what information the subsection contains and how you should consider/follow the contents of the subsection.

"#.to_string();

        if !apply_intelligently_rules.is_empty() {
            prompt.push_str(r#"<agent_requestable_workspace_rules description="These are workspace-level rules that you should follow. Use the Read tool with the provided absolute path to fetch full contents.">
"#);
            for rule in apply_intelligently_rules {
                let description = rule.description.as_deref().unwrap_or(&rule.name);
                prompt.push_str(&format!(
                    "- {}: {}\n",
                    rule.file_path.display().to_string().replace("\\", "/"),
                    description
                ));
            }
            prompt.push_str("</agent_requestable_workspace_rules>\n");
        }

        if !always_apply_rules.is_empty() {
            prompt.push_str(r#"<always_applied_workspace_rules description="These are workspace-level rules that you must always follow.">
"#);
            for rule in always_apply_rules {
                prompt.push_str(&format!("- {}\n", rule.content));
            }
            prompt.push_str("</always_applied_workspace_rules>\n");
        }

        if !enabled_user_rules.is_empty() {
            prompt.push_str(r#"<user_rules description="These are rules set by the user that you should follow if appropriate.">
"#);
            for rule in enabled_user_rules {
                prompt.push_str(&format!("- {}\n", rule.content));
            }
            prompt.push_str("</user_rules>\n");
        }

        prompt.push_str("</rules>\n\n");
        prompt
    }

    pub async fn build_system_prompt_for(
        &self,
        workspace_root: Option<&Path>,
    ) -> BitFunResult<String> {
        let user_rules = self.user_rules.read().await.clone();
        let project_rules = match workspace_root {
            Some(workspace_root) => {
                self.load_project_rules_for_workspace(workspace_root)
                    .await?
            }
            None => Vec::new(),
        };

        Ok(self.format_system_prompt(&user_rules, &project_rules))
    }

    pub async fn get_rules_for_file_with_workspace(
        &self,
        file_path: &str,
        workspace_root: Option<&Path>,
    ) -> FileRulesResult {
        let workspace_path = match workspace_root {
            Some(path) => path,
            None => {
                debug!("No workspace path set, skipping file-specific rules");
                return FileRulesResult {
                    matched_count: 0,
                    formatted_content: None,
                };
            }
        };

        let project_rules = match self.load_project_rules_for_workspace(workspace_path).await {
            Ok(rules) => rules,
            Err(e) => {
                warn!(
                    "Failed to load project rules for file '{}': {}",
                    file_path, e
                );
                return FileRulesResult {
                    matched_count: 0,
                    formatted_content: None,
                };
            }
        };

        let file_path_obj = Path::new(file_path);
        let relative_path = if file_path_obj.is_absolute() {
            file_path_obj
                .strip_prefix(workspace_path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| file_path.to_string())
        } else {
            file_path.to_string()
        };

        let relative_path = relative_path.replace("\\", "/");
        let file_name = Path::new(&relative_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let mut matching_rules: Vec<String> = Vec::new();

        for rule in &project_rules {
            if rule.apply_type != RuleApplyType::ApplyToSpecificFiles || !rule.enabled {
                continue;
            }

            if let Some(ref globs_str) = rule.globs {
                if self.matches_glob_pattern(globs_str, &relative_path, &file_name) {
                    matching_rules.push(rule.content.clone());
                    debug!(
                        "Rule '{}' matched for file '{}' (glob: {})",
                        rule.name, relative_path, globs_str
                    );
                }
            }
        }

        if matching_rules.is_empty() {
            FileRulesResult {
                matched_count: 0,
                formatted_content: None,
            }
        } else {
            let mut formatted = String::from("Rules relevant to this file:\n");
            for rule_content in &matching_rules {
                formatted.push_str(&format!("\n- {}", rule_content));
            }

            FileRulesResult {
                matched_count: matching_rules.len(),
                formatted_content: Some(formatted),
            }
        }
    }

    /// Builds the system prompt.
    pub async fn build_system_prompt(&self) -> BitFunResult<String> {
        let user_rules = self.user_rules.read().await.clone();
        let project_rules = self.project_rules.read().await.clone();
        Ok(self.format_system_prompt(&user_rules, &project_rules))
    }

    /// Gets matching "Apply to Specific Files" rules for a given file path.
    /// Returns the matched count and formatted content.
    pub async fn get_rules_for_file(&self, file_path: &str) -> FileRulesResult {
        let workspace_path = self.workspace_path.read().await.clone();
        self.get_rules_for_file_with_workspace(file_path, workspace_path.as_deref())
            .await
    }

    /// Checks whether a file matches the given glob patterns.
    fn matches_glob_pattern(&self, globs_str: &str, relative_path: &str, file_name: &str) -> bool {
        let patterns: Vec<&str> = globs_str.split(',').map(|s| s.trim()).collect();

        let mut glob_set_builder = GlobSetBuilder::new();
        let mut valid_patterns = false;

        for pattern in patterns {
            if pattern.is_empty() {
                continue;
            }

            let adjusted_pattern = if !pattern.contains('/') && !pattern.contains('\\') {
                format!("**/{}", pattern)
            } else {
                pattern.to_string()
            };

            match Glob::new(&adjusted_pattern) {
                Ok(glob) => {
                    glob_set_builder.add(glob);
                    valid_patterns = true;
                }
                Err(e) => {
                    warn!("Invalid glob pattern '{}': {}", pattern, e);
                }
            }
        }

        if !valid_patterns {
            return false;
        }

        match glob_set_builder.build() {
            Ok(glob_set) => glob_set.is_match(relative_path) || glob_set.is_match(file_name),
            Err(e) => {
                warn!("Failed to build glob set: {}", e);
                false
            }
        }
    }

    /// Loads all rules from a directory.
    async fn load_rules_from_dir(&self, dir: &Path, level: RuleLevel) -> BitFunResult<Vec<AIRule>> {
        let mut rules = Vec::new();

        if !dir.exists() {
            return Ok(rules);
        }

        let mut entries = match tokio::fs::read_dir(dir).await {
            Ok(entries) => entries,
            Err(e) => {
                warn!("Failed to read rules directory {:?}: {}", dir, e);
                return Ok(rules);
            }
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();

            if path.extension().is_some_and(|ext| ext == "mdc") {
                match self.load_rule_from_file(&path, level).await {
                    Ok(rule) => rules.push(rule),
                    Err(e) => {
                        warn!("Failed to load rule from {:?}: {}", path, e);
                    }
                }
            }
        }

        rules.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(rules)
    }

    /// Loads a single rule from a file.
    async fn load_rule_from_file(&self, path: &Path, level: RuleLevel) -> BitFunResult<AIRule> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to read file {:?}: {}", path, e)))?;

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .ok_or_else(|| BitFunError::service("Invalid file name".to_string()))?;

        AIRule::from_mdc(name, level, path.to_path_buf(), &content).map_err(BitFunError::service)
    }

    /// Creates a rule file.
    async fn create_rule_internal(
        &self,
        dir: &Path,
        level: RuleLevel,
        request: CreateRuleRequest,
    ) -> BitFunResult<()> {
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to create directory: {}", e)))?;

        let file_path = dir.join(filename_from_rule_name(&request.name));

        if file_path.exists() {
            return Err(BitFunError::service(format!(
                "Rule '{}' already exists",
                request.name
            )));
        }

        let mut frontmatter = match request.apply_type {
            RuleApplyType::AlwaysApply => RuleMetadata::always_apply(),
            RuleApplyType::ApplyIntelligently => {
                let desc = request.description.unwrap_or_else(|| request.name.clone());
                RuleMetadata::apply_intelligently(desc)
            }
            RuleApplyType::ApplyToSpecificFiles => {
                let globs = request.globs.unwrap_or_else(|| "*".to_string());
                RuleMetadata::apply_to_specific_files(globs)
            }
            RuleApplyType::ApplyManually => RuleMetadata::apply_manually(),
        };

        if level == RuleLevel::User {
            frontmatter = RuleMetadata::always_apply();
        }

        frontmatter.enabled = request.enabled;

        let mdc_content = format_mdc_content(&frontmatter, &request.content);

        tokio::fs::write(&file_path, mdc_content)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to write file: {}", e)))?;

        Ok(())
    }

    /// Updates a rule file.
    async fn update_rule_internal(
        &self,
        dir: &Path,
        name: &str,
        request: UpdateRuleRequest,
    ) -> BitFunResult<()> {
        let old_file_path = dir.join(filename_from_rule_name(name));

        if !old_file_path.exists() {
            return Err(BitFunError::service(format!("Rule '{}' not found", name)));
        }

        let content = tokio::fs::read_to_string(&old_file_path)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to read file: {}", e)))?;

        let (mut frontmatter, mut body) =
            parse_mdc_content(&content).map_err(BitFunError::service)?;

        if let Some(apply_type) = request.apply_type {
            match apply_type {
                RuleApplyType::AlwaysApply => {
                    frontmatter.always_apply = true;
                    frontmatter.description = None;
                    frontmatter.globs = None;
                }
                RuleApplyType::ApplyIntelligently => {
                    frontmatter.always_apply = false;
                    frontmatter.description = request.description.or(frontmatter.description);
                    frontmatter.globs = None;
                }
                RuleApplyType::ApplyToSpecificFiles => {
                    frontmatter.always_apply = false;
                    frontmatter.description = None;
                    frontmatter.globs = request.globs.or(frontmatter.globs);
                }
                RuleApplyType::ApplyManually => {
                    frontmatter.always_apply = false;
                    frontmatter.description = None;
                    frontmatter.globs = None;
                }
            }
        } else {
            if request.description.is_some() {
                frontmatter.description = request.description;
            }
            if request.globs.is_some() {
                frontmatter.globs = request.globs;
            }
        }

        if let Some(new_content) = request.content {
            body = new_content;
        }

        if let Some(enabled) = request.enabled {
            frontmatter.enabled = enabled;
        }

        let mdc_content = format_mdc_content(&frontmatter, &body);

        let new_file_path = if let Some(new_name) = &request.name {
            if new_name != name {
                let new_path = dir.join(filename_from_rule_name(new_name));
                if new_path.exists() {
                    return Err(BitFunError::service(format!(
                        "Rule '{}' already exists",
                        new_name
                    )));
                }
                tokio::fs::remove_file(&old_file_path).await.map_err(|e| {
                    BitFunError::service(format!("Failed to delete old file: {}", e))
                })?;
                new_path
            } else {
                old_file_path
            }
        } else {
            old_file_path
        };

        tokio::fs::write(&new_file_path, mdc_content)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to write file: {}", e)))?;

        Ok(())
    }

    /// Deletes a rule file.
    async fn delete_rule_internal(&self, dir: &Path, name: &str) -> BitFunResult<bool> {
        let file_path = dir.join(filename_from_rule_name(name));

        if !file_path.exists() {
            return Ok(false);
        }

        tokio::fs::remove_file(&file_path)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to delete file: {}", e)))?;

        Ok(true)
    }

    /// Calculates statistics.
    fn calculate_stats(rules: &[AIRule]) -> RuleStats {
        let mut by_apply_type = std::collections::HashMap::new();
        let mut enabled_count = 0;

        for rule in rules {
            let type_name = format!("{:?}", rule.apply_type).to_lowercase();
            *by_apply_type.entry(type_name).or_insert(0) += 1;
            if rule.enabled {
                enabled_count += 1;
            }
        }

        RuleStats {
            total_rules: rules.len(),
            enabled_rules: enabled_count,
            disabled_rules: rules.len() - enabled_count,
            by_apply_type,
        }
    }

    /// Toggles the enabled state of a user-level rule.
    pub async fn toggle_user_rule(&self, name: &str) -> BitFunResult<AIRule> {
        let rule = self
            .get_user_rule(name)
            .await?
            .ok_or_else(|| BitFunError::service(format!("Rule '{}' not found", name)))?;

        let new_enabled = !rule.enabled;
        self.update_user_rule(
            name,
            UpdateRuleRequest {
                name: None,
                apply_type: None,
                description: None,
                globs: None,
                content: None,
                enabled: Some(new_enabled),
            },
        )
        .await
    }

    /// Toggles the enabled state of a project-level rule.
    pub async fn toggle_project_rule(&self, name: &str) -> BitFunResult<AIRule> {
        let workspace_path = self.require_workspace_path().await?;
        self.toggle_project_rule_for_workspace(&workspace_path, name)
            .await
    }

    /// Toggles the enabled state of a project-level rule for the specified workspace.
    pub async fn toggle_project_rule_for_workspace(
        &self,
        workspace: &Path,
        name: &str,
    ) -> BitFunResult<AIRule> {
        let rule = self
            .get_project_rule_for_workspace(workspace, name)
            .await?
            .ok_or_else(|| BitFunError::service(format!("Rule '{}' not found", name)))?;

        let new_enabled = !rule.enabled;
        self.update_project_rule_for_workspace(
            workspace,
            name,
            UpdateRuleRequest {
                name: None,
                apply_type: None,
                description: None,
                globs: None,
                content: None,
                enabled: Some(new_enabled),
            },
        )
        .await
    }

    async fn require_workspace_path(&self) -> BitFunResult<PathBuf> {
        self.workspace_path
            .read()
            .await
            .clone()
            .ok_or_else(|| BitFunError::service("No workspace set".to_string()))
    }

    async fn sync_project_rules_cache_if_current(&self, workspace: &Path, rules: &[AIRule]) {
        let current_workspace = self.workspace_path.read().await.clone();
        if current_workspace.as_deref() == Some(workspace) {
            *self.project_rules.write().await = rules.to_vec();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AIRulesService;
    use crate::infrastructure::PathManager;
    use std::sync::Arc;
    use uuid::Uuid;

    #[tokio::test]
    async fn loading_missing_project_rules_does_not_create_rules_dir() {
        let test_root =
            std::env::temp_dir().join(format!("bitfun-ai-rules-test-{}", Uuid::new_v4()));
        let workspace_root = test_root.join("workspace");
        std::fs::create_dir_all(&workspace_root).expect("test workspace should be created");

        let path_manager = Arc::new(PathManager::with_user_root_for_tests(
            test_root.join("user-root"),
        ));
        let service = AIRulesService::new(path_manager.clone())
            .await
            .expect("ai rules service should initialize");

        let rules = service
            .get_project_rules_for_workspace(&workspace_root)
            .await
            .expect("loading project rules should succeed");

        assert!(rules.is_empty());
        assert!(!path_manager.project_rules_dir(&workspace_root).exists());

        let _ = std::fs::remove_dir_all(&test_root);
    }
}
