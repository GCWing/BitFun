//! Project Context service implementation
//!
//! Provides project context document management.

use super::builtin_documents::{find_builtin_document, get_builtin_categories, BUILTIN_DOCUMENTS};
use super::cancellation::{register_generation, unregister_generation};
use super::types::{
    CategoryInfo, ContextDocumentStatus, ContextSegment, CustomCategory, DocumentPriority,
    FileConflictAction, ImportedDocument, ProjectContextConfig,
};
use crate::agentic::coordination::get_global_coordinator;
use crate::agentic::tools::pipeline::SubagentParentInfo;
use crate::service::bootstrap::ensure_workspace_gitignore_ignores_bitfun;
use crate::util::errors::{BitFunError, BitFunResult};
use log::{debug, warn};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Config file name
const CONFIG_FILE_NAME: &str = "project-context-config.json";

const CANCELLED_PATH_SENTINEL: &str = "__CANCELLED__";

/// Context filter
#[derive(Debug, Clone, Default)]
struct ContextFilter {
    /// Included category list (allowlist mode)
    include: Option<Vec<String>>,
    /// Excluded category list
    exclude: Vec<String>,
}

/// Project Context service
pub struct ProjectContextService;

impl ProjectContextService {
    /// Creates a service instance.
    pub fn new() -> Self {
        Self
    }

    /// Returns all category IDs.
    pub fn get_categories(&self) -> Vec<&'static str> {
        get_builtin_categories()
    }

    /// Returns document statuses (built-in docs + imported docs).
    pub async fn get_document_statuses(
        &self,
        workspace: &Path,
    ) -> BitFunResult<Vec<ContextDocumentStatus>> {
        let config = self.load_config_and_cleanup(workspace).await?;

        let mut statuses = Vec::new();

        for doc in BUILTIN_DOCUMENTS {
            let status = self.scan_document_status(workspace, doc, &config).await;
            statuses.push(status);
        }

        for imported_doc in &config.imported_documents {
            let exists = Path::new(&imported_doc.file_path).exists();

            let token_count = if exists {
                self.estimate_tokens(Path::new(&imported_doc.file_path))
                    .await
            } else {
                0
            };

            let enabled = config
                .enabled_documents
                .get(&imported_doc.id)
                .copied()
                .unwrap_or(false);

            statuses.push(ContextDocumentStatus {
                id: imported_doc.id.clone(),
                name: imported_doc.name.clone(),
                category_id: imported_doc.category_id.clone(),
                file_path: Some(imported_doc.file_path.clone()),
                exists,
                enabled,
                can_generate: false,
                priority: imported_doc.priority.clone(),
                token_count,
            });
        }

        Ok(statuses)
    }

    /// Scans a single document status.
    async fn scan_document_status(
        &self,
        workspace: &Path,
        doc: &super::builtin_documents::BuiltinDocument,
        config: &ProjectContextConfig,
    ) -> ContextDocumentStatus {
        let (exists, file_path, token_count) =
            self.find_existing_file(workspace, doc.possible_paths).await;

        let enabled = if exists {
            config
                .enabled_documents
                .get(doc.id)
                .copied()
                .unwrap_or(doc.default_enabled)
        } else {
            false
        };

        ContextDocumentStatus {
            id: doc.id.to_string(),
            name: doc.name.to_string(),
            category_id: doc.category_id.to_string(),
            file_path,
            exists,
            enabled,
            can_generate: doc.can_generate,
            priority: doc.priority.as_str().to_string(),
            token_count,
        }
    }

    /// Finds the first existing file and returns `(exists, file_path, token_count)`.
    async fn find_existing_file(
        &self,
        workspace: &Path,
        possible_paths: &[&str],
    ) -> (bool, Option<String>, u32) {
        for relative_path in possible_paths {
            let full_path = workspace.join(relative_path);
            if full_path.exists() {
                let token_count = self.estimate_tokens(&full_path).await;
                return (
                    true,
                    Some(full_path.to_string_lossy().to_string()),
                    token_count,
                );
            }
        }

        let default_path = possible_paths
            .first()
            .map(|p| workspace.join(p).to_string_lossy().to_string());

        (false, default_path, 0)
    }

    /// Estimates the token count for a file.
    async fn estimate_tokens(&self, path: &Path) -> u32 {
        match fs::read_to_string(path).await {
            Ok(content) => (content.len() / 4) as u32,
            Err(_) => 0,
        }
    }

    /// Toggles whether a document is enabled.
    pub async fn toggle_document(
        &self,
        workspace: &Path,
        doc_id: &str,
        enabled: bool,
    ) -> BitFunResult<()> {
        let mut config = self.load_config(workspace).await.unwrap_or_default();
        config.enabled_documents.insert(doc_id.to_string(), enabled);
        self.save_config(workspace, &config).await
    }

    /// Creates an empty document.
    pub async fn create_document(&self, workspace: &Path, doc_id: &str) -> BitFunResult<PathBuf> {
        let doc = find_builtin_document(doc_id)
            .ok_or_else(|| BitFunError::service(format!("Unknown document id: {}", doc_id)))?;

        let relative_path = doc.possible_paths.first().ok_or_else(|| {
            BitFunError::service(format!("No possible path for document: {}", doc_id))
        })?;

        let full_path = workspace.join(relative_path);

        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| BitFunError::service(format!("Failed to create directory: {}", e)))?;
        }

        let template = doc.default_template;
        fs::write(&full_path, template)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to create document: {}", e)))?;

        debug!("Created document: path={:?}", full_path);

        self.toggle_document(workspace, doc_id, doc.default_enabled)
            .await?;

        Ok(full_path)
    }

    /// Generates document content using AI.
    ///
    /// # Parameters
    /// - workspace: Workspace path
    /// - doc_id: Document ID
    ///
    /// # Returns
    /// Generated file path
    pub async fn generate_document(&self, workspace: &Path, doc_id: &str) -> BitFunResult<PathBuf> {
        let cancel_token = register_generation(doc_id).await;

        let doc = find_builtin_document(doc_id)
            .ok_or_else(|| BitFunError::service(format!("Unknown document id: {}", doc_id)))?;

        if !doc.can_generate {
            unregister_generation(doc_id).await;
            return Err(BitFunError::service(format!(
                "Document '{}' does not support AI generation",
                doc.name
            )));
        }

        let relative_path = if let Some(path) = doc.possible_paths.first() {
            path
        } else {
            unregister_generation(doc_id).await;
            return Err(BitFunError::service(format!(
                "No possible path for document: {}",
                doc_id
            )));
        };

        let full_path = workspace.join(relative_path);

        if full_path.exists() {
            unregister_generation(doc_id).await;
            return Err(BitFunError::service(format!(
                "Document '{}' already exists at {:?}",
                doc.name, full_path
            )));
        }

        if let Some(parent) = full_path.parent() {
            if let Err(e) = fs::create_dir_all(parent).await {
                unregister_generation(doc_id).await;
                return Err(BitFunError::service(format!(
                    "Failed to create directory: {}",
                    e
                )));
            }
        }

        if cancel_token.is_cancelled() {
            debug!(
                "Document generation cancelled before execution: doc_id={}",
                doc_id
            );
            unregister_generation(doc_id).await;
            return Ok(PathBuf::from(CANCELLED_PATH_SENTINEL));
        }

        let coordinator = if let Some(coordinator) = get_global_coordinator() {
            coordinator
        } else {
            unregister_generation(doc_id).await;
            return Err(BitFunError::service(
                "Coordinator not initialized".to_string(),
            ));
        };

        let prompt = doc.generation_prompt.to_string();

        let subagent_parent_info = SubagentParentInfo {
            tool_call_id: format!("generate-doc-{}", doc_id),
            session_id: format!("standalone-generate-{}", uuid::Uuid::new_v4()),
            dialog_turn_id: format!("turn-{}", uuid::Uuid::new_v4()),
        };

        let result = coordinator
            .execute_subagent(
                "GenerateDoc".to_string(),
                prompt,
                subagent_parent_info.clone(),
                Some(workspace.to_string_lossy().into_owned()),
                None,
                Some(&cancel_token),
                None,
                None,
            )
            .await;

        let result = match result {
            Ok(r) => r,
            Err(e) => {
                if matches!(e, BitFunError::Cancelled(_)) {
                    debug!(
                        "Document generation cancelled during execution: doc_id={}",
                        doc_id
                    );
                    unregister_generation(doc_id).await;
                    return Ok(PathBuf::from(CANCELLED_PATH_SENTINEL));
                }
                unregister_generation(doc_id).await;
                return Err(BitFunError::service(format!(
                    "Generation task failed: {}",
                    e
                )));
            }
        };

        let content = result.text;

        if let Err(e) = fs::write(&full_path, content).await {
            unregister_generation(doc_id).await;
            return Err(BitFunError::service(format!(
                "Failed to write generated document: {}",
                e
            )));
        }

        debug!("Generated document: path={:?}", full_path);

        self.toggle_document(workspace, doc_id, doc.default_enabled)
            .await?;

        unregister_generation(doc_id).await;

        Ok(full_path)
    }

    /// Cancels document generation.
    ///
    /// # Parameters
    /// - doc_id: Document ID
    ///
    /// # Returns
    /// Ok(()) on success; Err if the task does not exist.
    pub async fn cancel_generate_document(&self, doc_id: &str) -> BitFunResult<()> {
        super::cancellation::cancel_generation(doc_id)
            .await
            .map_err(BitFunError::service)
    }

    /// Parses a filter string.
    ///
    /// Supported syntax:
    /// - `include=general,design` - Only include these categories
    /// - `exclude=review` - Exclude these categories
    fn parse_filter(filter_str: &str) -> Option<ContextFilter> {
        let filter_str = filter_str.trim();
        if filter_str.is_empty() {
            return None;
        }

        if let Some(rest) = filter_str.strip_prefix("include=") {
            let categories: Vec<String> = rest
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if !categories.is_empty() {
                return Some(ContextFilter {
                    include: Some(categories),
                    exclude: Vec::new(),
                });
            }
        }

        if let Some(rest) = filter_str.strip_prefix("exclude=") {
            let categories: Vec<String> = rest
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if !categories.is_empty() {
                return Some(ContextFilter {
                    include: None,
                    exclude: categories,
                });
            }
        }

        None
    }

    /// Checks whether a category should be included.
    fn should_include_category(filter: &Option<ContextFilter>, category_id: &str) -> bool {
        match filter {
            None => true,
            Some(f) => {
                if let Some(include) = &f.include {
                    include.iter().any(|c| c == category_id)
                } else {
                    !f.exclude.iter().any(|c| c == category_id)
                }
            }
        }
    }

    /// Returns the category description.
    fn get_category_description(category_id: &str) -> &'static str {
        match category_id {
            "general" => {
                "Project overview, objectives, and key conventions for quickly understanding the codebase."
            }
            "coding" => {
                "Code style and formatting rules that all code must adhere to."
            }
            "design" => {
                "High-level architecture and design patterns."
            }
            "review" => {
                "Guidelines for code reviews and audits."
            }
            _ => "Other project documentation",
        }
    }

    /// Returns the category sort order.
    fn get_category_order(category_id: &str) -> usize {
        match category_id {
            "general" => 0,
            "coding" => 1,
            "design" => 2,
            "review" => 3,
            _ => 999,
        }
    }

    /// Builds a context prompt.
    ///
    /// # Parameters
    /// - workspace: Workspace path
    /// - filter: Optional filter: `include=category1,category2` or `exclude=category1`
    pub async fn build_context_prompt(
        &self,
        workspace: &Path,
        filter: Option<&str>,
    ) -> BitFunResult<String> {
        let filter = filter.and_then(Self::parse_filter);
        let config = self.load_config_and_cleanup(workspace).await?;
        let statuses = self.get_document_statuses(workspace).await?;

        let mut segments: Vec<ContextSegment> = Vec::new();

        for status in statuses.iter().filter(|s| {
            s.enabled && s.exists && Self::should_include_category(&filter, &s.category_id)
        }) {
            if let Some(ref file_path) = status.file_path {
                let path = Path::new(file_path);
                match fs::read_to_string(path).await {
                    Ok(content) => {
                        let token_count = (content.len() / 4) as u32;
                        let priority = match status.priority.as_str() {
                            "high" => DocumentPriority::High,
                            "medium" => DocumentPriority::Medium,
                            _ => DocumentPriority::Low,
                        };

                        segments.push(ContextSegment {
                            doc_id: status.id.clone(),
                            name: status.name.clone(),
                            category_id: status.category_id.clone(),
                            content,
                            token_count,
                            priority,
                        });
                    }
                    Err(e) => {
                        warn!("Failed to read document: doc_id={} error={}", status.id, e);
                    }
                }
            }
        }

        if segments.is_empty() {
            return Ok(String::new());
        }

        segments.sort_by(|a, b| {
            let cat_order_a = Self::get_category_order(&a.category_id);
            let cat_order_b = Self::get_category_order(&b.category_id);
            match cat_order_a.cmp(&cat_order_b) {
                std::cmp::Ordering::Equal => {
                    let priority_order = |p: &DocumentPriority| match p {
                        DocumentPriority::High => 0,
                        DocumentPriority::Medium => 1,
                        DocumentPriority::Low => 2,
                    };
                    priority_order(&a.priority).cmp(&priority_order(&b.priority))
                }
                other => other,
            }
        });

        let mut prompt = String::from("<project_context>\n");
        let mut current_category: Option<String> = None;

        for segment in &segments {
            if current_category.as_ref() != Some(&segment.category_id) {
                if current_category.is_some() {
                    prompt.push('\n');
                }

                let category_name =
                    Self::get_category_name_with_config(&config, &segment.category_id);
                let description =
                    Self::get_category_description_with_config(&config, &segment.category_id);
                prompt.push_str(&format!("<!-- Category: {} -->\n", category_name));
                if !description.is_empty() {
                    prompt.push_str(&format!("<!-- Description: {} -->\n", description));
                }

                current_category = Some(segment.category_id.clone());
            }

            prompt.push_str(&format!(
                "<document name=\"{}\">\n{}\n</document>\n",
                segment.name, segment.content
            ));
        }

        prompt.push_str("</project_context>");

        Ok(prompt)
    }

    /// Returns the category description (supports custom categories).
    fn get_category_description_with_config(
        config: &ProjectContextConfig,
        category_id: &str,
    ) -> String {
        if let Some(custom_cat) = config
            .custom_categories
            .iter()
            .find(|cat| cat.id == category_id)
        {
            return custom_cat
                .description
                .clone()
                .unwrap_or_else(|| custom_cat.name.clone());
        }

        Self::get_category_description(category_id).to_string()
    }

    /// Returns the category display name (supports custom categories).
    fn get_category_name_with_config(config: &ProjectContextConfig, category_id: &str) -> String {
        if let Some(custom_cat) = config
            .custom_categories
            .iter()
            .find(|cat| cat.id == category_id)
        {
            return custom_cat.name.clone();
        }

        Self::get_category_display_name(category_id)
    }

    /// Loads configuration.
    pub async fn load_config(&self, workspace: &Path) -> BitFunResult<ProjectContextConfig> {
        let config_path = self.get_config_path(workspace);

        if !config_path.exists() {
            return Ok(ProjectContextConfig::default());
        }

        let content = fs::read_to_string(&config_path)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to read config: {}", e)))?;

        serde_json::from_str(&content)
            .map_err(|e| BitFunError::service(format!("Failed to parse config: {}", e)))
    }

    /// Loads configuration and cleans up orphaned records.
    ///
    /// Auto-cleanup:
    /// 1. Imported document records whose physical files do not exist
    /// 2. Enabled-state records referencing non-existent documents
    async fn load_config_and_cleanup(
        &self,
        workspace: &Path,
    ) -> BitFunResult<ProjectContextConfig> {
        let mut config = self.load_config(workspace).await.unwrap_or_default();
        let mut modified = false;

        let original_count = config.imported_documents.len();
        config.imported_documents.retain(|doc| {
            let exists = Path::new(&doc.file_path).exists();
            if !exists {
                debug!(
                    "Removing orphaned document: name={} id={} path={}",
                    doc.name, doc.id, doc.file_path
                );
            }
            exists
        });

        if config.imported_documents.len() != original_count {
            modified = true;
            debug!(
                "Removed {} orphaned document(s)",
                original_count - config.imported_documents.len()
            );
        }

        let valid_doc_ids: HashSet<_> = config
            .imported_documents
            .iter()
            .map(|d| d.id.clone())
            .collect();

        let builtin_doc_ids: HashSet<_> =
            BUILTIN_DOCUMENTS.iter().map(|d| d.id.to_string()).collect();

        let original_enabled_count = config.enabled_documents.len();
        config.enabled_documents.retain(|doc_id, _| {
            let is_valid = valid_doc_ids.contains(doc_id) || builtin_doc_ids.contains(doc_id);
            if !is_valid {
                debug!("Removing orphaned enabled state: doc_id={}", doc_id);
            }
            is_valid
        });

        if config.enabled_documents.len() != original_enabled_count {
            modified = true;
            debug!(
                "Removed {} orphaned enabled state(s)",
                original_enabled_count - config.enabled_documents.len()
            );
        }

        if modified {
            self.save_config(workspace, &config).await?;
            debug!("Config cleaned and saved");
        }

        Ok(config)
    }

    /// Saves configuration.
    pub async fn save_config(
        &self,
        workspace: &Path,
        config: &ProjectContextConfig,
    ) -> BitFunResult<()> {
        let config_path = self.get_config_path(workspace);

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                BitFunError::service(format!("Failed to create .bitfun directory: {}", e))
            })?;
        }

        if let Err(e) = ensure_workspace_gitignore_ignores_bitfun(workspace).await {
            warn!(
                "Failed to ensure workspace .gitignore ignores .bitfun: workspace={}, error={}",
                workspace.display(),
                e
            );
        }

        let content = serde_json::to_string_pretty(config)
            .map_err(|e| BitFunError::service(format!("Failed to serialize config: {}", e)))?;

        fs::write(&config_path, content)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to write config: {}", e)))?;

        debug!("Saved project context config: path={:?}", config_path);
        Ok(())
    }

    /// Returns the config file path.
    fn get_config_path(&self, workspace: &Path) -> PathBuf {
        workspace.join(".bitfun").join(CONFIG_FILE_NAME)
    }

    /// Creates a custom category.
    ///
    /// # Parameters
    /// - workspace: Workspace path
    /// - name: Category name
    /// - description: Category description (optional)
    /// - icon: Icon name (lucide-react icon name)
    ///
    /// # Returns
    /// Newly created category ID
    pub async fn create_category(
        &self,
        workspace: &Path,
        name: String,
        description: Option<String>,
        icon: String,
    ) -> BitFunResult<String> {
        let mut config = self.load_config(workspace).await.unwrap_or_default();

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| BitFunError::service(format!("Failed to get timestamp: {}", e)))?
            .as_secs();

        let category_id = format!("custom-{}", timestamp);

        let order = get_builtin_categories().len() + config.custom_categories.len();

        let category = CustomCategory {
            id: category_id.clone(),
            name,
            description,
            icon,
            created_at: timestamp as i64,
            order,
        };

        config.custom_categories.push(category);
        self.save_config(workspace, &config).await?;

        debug!("Created custom category: category_id={}", category_id);

        Ok(category_id)
    }

    /// Updates a custom category.
    ///
    /// # Parameters
    /// - workspace: Workspace path
    /// - category_id: Category ID
    /// - name: New category name
    /// - description: New category description (optional)
    /// - icon: New icon name (lucide-react icon name)
    pub async fn update_category(
        &self,
        workspace: &Path,
        category_id: &str,
        name: String,
        description: Option<String>,
        icon: String,
    ) -> BitFunResult<()> {
        let mut config = self.load_config(workspace).await?;

        let category = config
            .custom_categories
            .iter_mut()
            .find(|cat| cat.id == category_id)
            .ok_or_else(|| {
                BitFunError::service(format!("Custom category not found: {}", category_id))
            })?;

        category.name = name;
        category.description = description;
        category.icon = icon;

        self.save_config(workspace, &config).await?;

        debug!("Updated custom category: category_id={}", category_id);

        Ok(())
    }

    /// Deletes a custom category.
    ///
    /// # Parameters
    /// - workspace: Workspace path
    /// - category_id: Category ID
    ///
    /// Note: Imported documents under this category will also be deleted.
    pub async fn delete_category(&self, workspace: &Path, category_id: &str) -> BitFunResult<()> {
        if get_builtin_categories().contains(&category_id) {
            return Err(BitFunError::service(format!(
                "Cannot delete builtin category: {}",
                category_id
            )));
        }

        let mut config = self.load_config(workspace).await?;

        let category_index = config
            .custom_categories
            .iter()
            .position(|cat| cat.id == category_id)
            .ok_or_else(|| {
                BitFunError::service(format!("Custom category not found: {}", category_id))
            })?;

        let imported_doc_paths: Vec<String> = config
            .imported_documents
            .iter()
            .filter(|doc| doc.category_id == category_id)
            .map(|doc| doc.file_path.clone())
            .collect();

        let imported_docs_count = imported_doc_paths.len();

        if !imported_doc_paths.is_empty() {
            for doc_path in &imported_doc_paths {
                let path = Path::new(doc_path);
                if path.exists() {
                    fs::remove_file(&path).await.map_err(|e| {
                        BitFunError::service(format!(
                            "Failed to remove imported document file: {}",
                            e
                        ))
                    })?;
                    debug!("Removed imported document file: path={:?}", path);
                }
            }

            config
                .imported_documents
                .retain(|doc| doc.category_id != category_id);

            debug!(
                "Deleted {} imported documents from category: category_id={}",
                imported_docs_count, category_id
            );
        }

        config.custom_categories.remove(category_index);

        self.save_config(workspace, &config).await?;

        debug!("Deleted custom category: category_id={}", category_id);

        Ok(())
    }

    /// Returns all category info (built-in + custom).
    ///
    /// # Parameters
    /// - workspace: Workspace path
    ///
    /// # Returns
    /// Category info list, sorted by `order`
    pub async fn get_all_categories(&self, workspace: &Path) -> BitFunResult<Vec<CategoryInfo>> {
        let config = self.load_config_and_cleanup(workspace).await?;

        let mut categories = Vec::new();

        for builtin_id in get_builtin_categories() {
            let description = Self::get_category_description(builtin_id);
            let order = Self::get_category_order(builtin_id);
            let icon = Self::get_category_icon(builtin_id);

            categories.push(CategoryInfo {
                id: builtin_id.to_string(),
                name: Self::get_category_display_name(builtin_id),
                description: Some(description.to_string()),
                icon,
                is_builtin: true,
                order,
            });
        }

        for custom_cat in &config.custom_categories {
            categories.push(CategoryInfo {
                id: custom_cat.id.clone(),
                name: custom_cat.name.clone(),
                description: custom_cat.description.clone(),
                icon: custom_cat.icon.clone(),
                is_builtin: false,
                order: custom_cat.order,
            });
        }

        categories.sort_by_key(|cat| cat.order);

        Ok(categories)
    }

    /// Returns the category display name.
    fn get_category_display_name(category_id: &str) -> String {
        match category_id {
            "general" => "General",
            "coding" => "Coding Standards",
            "design" => "Design",
            "review" => "Review",
            _ => "Other",
        }
        .to_string()
    }

    /// Returns the category icon name.
    fn get_category_icon(category_id: &str) -> String {
        match category_id {
            "general" => "FileText",
            "coding" => "Code",
            "design" => "Boxes",
            "review" => "GitPullRequest",
            _ => "FolderOpen",
        }
        .to_string()
    }

    /// Imports a document into the specified category.
    ///
    /// # Parameters
    /// - workspace: Workspace path
    /// - source_path: Source file path (absolute path)
    /// - name: Document name (file name)
    /// - category_id: Target category ID (built-in or custom)
    /// - priority: Priority (`high`/`medium`/`low`)
    /// - on_conflict: File conflict action
    ///
    /// # Returns
    /// Imported document info
    pub async fn import_document(
        &self,
        workspace: &Path,
        source_path: &Path,
        name: String,
        category_id: String,
        priority: String,
        on_conflict: FileConflictAction,
    ) -> BitFunResult<ImportedDocument> {
        if !source_path.exists() {
            return Err(BitFunError::service(format!(
                "Source file does not exist: {:?}",
                source_path
            )));
        }

        let config = self.load_config(workspace).await.unwrap_or_default();
        let category_exists = get_builtin_categories().contains(&category_id.as_str())
            || config
                .custom_categories
                .iter()
                .any(|cat| cat.id == category_id);

        if !category_exists {
            return Err(BitFunError::service(format!(
                "Category not found: {}",
                category_id
            )));
        }

        let doc_id = uuid::Uuid::new_v4().to_string();

        let target_dir = workspace.join(".bitfun").join("docs").join(&category_id);
        let target_file_name = format!("{}.{}", doc_id, Self::get_file_extension(&name));
        let target_path = target_dir.join(&target_file_name);

        fs::create_dir_all(&target_dir)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to create directory: {}", e)))?;

        let final_target_path = if target_path.exists() {
            match on_conflict {
                FileConflictAction::Skip => {
                    return Err(BitFunError::service(format!(
                        "File already exists and conflict action is skip: {:?}",
                        target_path
                    )));
                }
                FileConflictAction::Overwrite => target_path,
                FileConflictAction::Rename => {
                    let mut counter = 1;
                    loop {
                        let new_name =
                            format!("{}-{}.{}", doc_id, counter, Self::get_file_extension(&name));
                        let new_path = target_dir.join(&new_name);
                        if !new_path.exists() {
                            break new_path;
                        }
                        counter += 1;
                        if counter > 1000 {
                            return Err(BitFunError::service(
                                "Failed to generate unique filename".to_string(),
                            ));
                        }
                    }
                }
            }
        } else {
            target_path
        };

        fs::copy(source_path, &final_target_path)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to copy file: {}", e)))?;

        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| BitFunError::service(format!("Failed to get timestamp: {}", e)))?
            .as_secs() as i64;

        let imported_doc = ImportedDocument {
            id: doc_id.clone(),
            name,
            category_id,
            file_path: final_target_path.to_string_lossy().to_string(),
            priority,
            created_at,
        };

        let mut config = self.load_config(workspace).await.unwrap_or_default();
        config.imported_documents.push(imported_doc.clone());
        self.save_config(workspace, &config).await?;

        debug!(
            "Imported document: doc_id={} category_id={}",
            doc_id, imported_doc.category_id
        );

        Ok(imported_doc)
    }

    /// Returns the file extension.
    fn get_file_extension(filename: &str) -> String {
        filename
            .rsplit('.')
            .next()
            .filter(|ext| !ext.is_empty())
            .unwrap_or("md")
            .to_string()
    }

    /// Toggles whether an imported document is enabled.
    ///
    /// # Parameters
    /// - workspace: Workspace path
    /// - doc_id: Document ID
    /// - enabled: Whether enabled
    pub async fn toggle_imported_document(
        &self,
        workspace: &Path,
        doc_id: &str,
        enabled: bool,
    ) -> BitFunResult<()> {
        let mut config = self.load_config(workspace).await?;

        let doc_exists = config.imported_documents.iter().any(|doc| doc.id == doc_id);

        if !doc_exists {
            return Err(BitFunError::service(format!(
                "Imported document not found: {}",
                doc_id
            )));
        }

        config.enabled_documents.insert(doc_id.to_string(), enabled);

        self.save_config(workspace, &config).await?;

        debug!(
            "Toggled imported document: doc_id={} enabled={}",
            doc_id, enabled
        );

        Ok(())
    }

    /// Deletes an imported document.
    ///
    /// # Parameters
    /// - workspace: Workspace path
    /// - doc_id: Document ID
    pub async fn delete_imported_document(
        &self,
        workspace: &Path,
        doc_id: &str,
    ) -> BitFunResult<()> {
        let mut config = self.load_config(workspace).await?;

        let doc_index = config
            .imported_documents
            .iter()
            .position(|doc| doc.id == doc_id)
            .ok_or_else(|| {
                BitFunError::service(format!("Imported document not found: {}", doc_id))
            })?;

        let doc = &config.imported_documents[doc_index];

        let doc_path = Path::new(&doc.file_path);
        if doc_path.exists() {
            fs::remove_file(&doc_path).await.map_err(|e| {
                BitFunError::service(format!("Failed to remove document file: {}", e))
            })?;
            debug!("Removed imported document file: path={:?}", doc_path);
        }

        config.imported_documents.remove(doc_index);

        self.save_config(workspace, &config).await?;

        debug!("Deleted imported document: doc_id={}", doc_id);

        Ok(())
    }

    /// Deletes a context document (generic API).
    ///
    /// Supports deleting both built-in and imported documents:
    /// - Built-in: delete only the physical file; keep definition (can be recreated)
    /// - Imported: delete the physical file and remove it from configuration
    ///
    /// # Parameters
    /// - workspace: Workspace path
    /// - doc_id: Document ID
    pub async fn delete_context_document(
        &self,
        workspace: &Path,
        doc_id: &str,
    ) -> BitFunResult<()> {
        let config = self.load_config(workspace).await?;

        if let Some(_doc_index) = config
            .imported_documents
            .iter()
            .position(|doc| doc.id == doc_id)
        {
            return self.delete_imported_document(workspace, doc_id).await;
        }

        if let Some(doc) = find_builtin_document(doc_id) {
            let relative_path = doc.possible_paths.first().ok_or_else(|| {
                BitFunError::service(format!("No possible path for document: {}", doc_id))
            })?;

            let full_path = workspace.join(relative_path);

            if full_path.exists() {
                fs::remove_file(&full_path).await.map_err(|e| {
                    BitFunError::service(format!("Failed to remove document file: {}", e))
                })?;
                debug!("Removed builtin document file: path={:?}", full_path);
            }

            debug!(
                "Deleted builtin document: doc_id={} (definition preserved)",
                doc_id
            );
            Ok(())
        } else {
            Err(BitFunError::service(format!(
                "Unknown document id: {}",
                doc_id
            )))
        }
    }

    /// Returns all imported documents.
    ///
    /// # Parameters
    /// - workspace: Workspace path
    ///
    /// # Returns
    /// Imported document list
    pub async fn get_imported_documents(
        &self,
        workspace: &Path,
    ) -> BitFunResult<Vec<ImportedDocument>> {
        let config = self.load_config(workspace).await.unwrap_or_default();
        Ok(config.imported_documents)
    }
}

impl Default for ProjectContextService {
    fn default() -> Self {
        Self::new()
    }
}
