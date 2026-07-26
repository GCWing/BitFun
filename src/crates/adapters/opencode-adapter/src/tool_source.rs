use bitfun_product_domains::external_sources::{
    EcosystemId, ExternalSourceAssetKind, ExternalSourceContext, ExternalSourceDiagnostic,
    ExternalSourceHealth, ExternalSourceProviderError, ExternalSourceRecord, ExternalSourceScope,
    ExternalToolCapability, ExternalToolDefinition, ExternalToolProviderIdentity,
    ExternalToolProviderSnapshot, ExternalToolRuntimeKind, ExternalToolSourceProvider,
    ExternalToolStaticStatus, ExternalWatchRoot, PreparedExternalToolExport,
    PreparedExternalToolTarget, SourceKey, SourceQualifiedToolId, SourceQualifiedToolTargetId,
};
use regex::Regex;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[cfg(test)]
use std::sync::Arc;

const PROVIDER_ID: &str = "opencode.tools";
const ECOSYSTEM_ID: &str = "opencode";
const MAX_TOOL_FILES: usize = 1024;
const MAX_TOOL_FILE_BYTES: u64 = 512 * 1024;

const TOOL_SHIM: &str = r#"
const __bitfunSchema = (schema) => Object.assign(schema, {
  describe(description) { this.description = description; return this; },
  optional() { this.__optional = true; return this; },
  default(value) { this.__default = value; this.__optional = true; return this; },
  min(value) {
    if (this.type === "string") this.minLength = value;
    else if (this.type === "array") this.minItems = value;
    else this.minimum = value;
    return this;
  },
  max(value) {
    if (this.type === "string") this.maxLength = value;
    else if (this.type === "array") this.maxItems = value;
    else this.maximum = value;
    return this;
  },
  int() { this.type = "integer"; return this; },
});
const tool = (definition) => definition;
tool.schema = {
  string: () => __bitfunSchema({ type: "string" }),
  number: () => __bitfunSchema({ type: "number" }),
  boolean: () => __bitfunSchema({ type: "boolean" }),
  enum: (values) => __bitfunSchema({ type: "string", enum: values }),
  array: (items) => __bitfunSchema({ type: "array", items }),
  object: (properties) => __bitfunSchema({ type: "object", properties, additionalProperties: false }),
};
"#;

#[derive(Debug, Clone)]
pub struct OpenCodeToolProviderOptions {
    pub user_config_dir: PathBuf,
    pub legacy_user_config_dir: Option<PathBuf>,
    pub explicit_config_dir: Option<PathBuf>,
    pub project_config_enabled: bool,
}

impl OpenCodeToolProviderOptions {
    pub fn from_environment() -> Self {
        let home = dirs::home_dir();
        let user_config_dir = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|home| home.join(".config")))
            .unwrap_or_else(|| PathBuf::from(".config"))
            .join("opencode");
        Self {
            user_config_dir,
            legacy_user_config_dir: home.map(|home| home.join(".opencode")),
            explicit_config_dir: std::env::var_os("OPENCODE_CONFIG_DIR").map(PathBuf::from),
            project_config_enabled: !environment_truthy("OPENCODE_DISABLE_PROJECT_CONFIG"),
        }
    }
}

impl Default for OpenCodeToolProviderOptions {
    fn default() -> Self {
        Self::from_environment()
    }
}

pub struct OpenCodeToolProvider {
    options: OpenCodeToolProviderOptions,
    #[cfg(test)]
    directory_reader: Option<Arc<dyn Fn(&Path) -> std::io::Result<fs::ReadDir> + Send + Sync>>,
}

impl OpenCodeToolProvider {
    pub fn new(options: OpenCodeToolProviderOptions) -> Self {
        Self {
            options,
            #[cfg(test)]
            directory_reader: None,
        }
    }

    #[cfg(test)]
    fn with_directory_reader(
        mut self,
        reader: impl Fn(&Path) -> std::io::Result<fs::ReadDir> + Send + Sync + 'static,
    ) -> Self {
        self.directory_reader = Some(Arc::new(reader));
        self
    }

    fn read_directory(&self, path: &Path) -> std::io::Result<fs::ReadDir> {
        #[cfg(test)]
        if let Some(reader) = &self.directory_reader {
            return reader(path);
        }
        fs::read_dir(path)
    }

    fn tool_directories(&self, context: &ExternalSourceContext) -> Vec<ToolDirectory> {
        let mut directories = Vec::new();
        push_tool_directories(
            &mut directories,
            &self.options.user_config_dir,
            ExternalSourceScope::UserGlobal,
            "OpenCode user tools",
        );
        if let Some(explicit_config_dir) = &self.options.explicit_config_dir {
            push_tool_directories(
                &mut directories,
                explicit_config_dir,
                ExternalSourceScope::UserGlobal,
                "OpenCode explicit user tools",
            );
        }
        if self.options.project_config_enabled {
            if let Some(workspace_root) = &context.workspace_root {
                let project_root = find_project_root(workspace_root);
                for directory in directories_between(&project_root, workspace_root) {
                    push_tool_directories(
                        &mut directories,
                        &directory.join(".opencode"),
                        ExternalSourceScope::Project,
                        "OpenCode project tools",
                    );
                }
            }
        }
        if let Some(legacy) = &self.options.legacy_user_config_dir {
            if legacy != &self.options.user_config_dir
                && self.options.explicit_config_dir.as_ref() != Some(legacy)
            {
                push_tool_directories(
                    &mut directories,
                    legacy,
                    ExternalSourceScope::UserGlobal,
                    "OpenCode legacy tools",
                );
            }
        }
        deduplicate_directories(directories)
    }
}

impl Default for OpenCodeToolProvider {
    fn default() -> Self {
        Self::new(OpenCodeToolProviderOptions::default())
    }
}

impl ExternalToolSourceProvider for OpenCodeToolProvider {
    fn identity(&self) -> ExternalToolProviderIdentity {
        ExternalToolProviderIdentity::new(PROVIDER_ID, ECOSYSTEM_ID, "OpenCode")
            .expect("static OpenCode tool provider identity must be valid")
    }

    fn discover(
        &self,
        context: &ExternalSourceContext,
    ) -> Result<ExternalToolProviderSnapshot, ExternalSourceProviderError> {
        if context
            .workspace_root
            .as_ref()
            .is_some_and(|workspace_root| !workspace_root.is_absolute())
        {
            return Err(ExternalSourceProviderError::new(
                "opencode.tool.workspace_invalid",
                "workspace root must be absolute",
                false,
            ));
        }
        let provider = self.identity();
        let mut sources = Vec::new();
        let mut tools = Vec::new();
        let mut diagnostics = Vec::new();
        let working_directory = self.working_directory(context);

        for directory in self.tool_directories(context) {
            let source_key = source_key(&directory.path)?;
            match fs::metadata(&directory.path) {
                Ok(metadata) if metadata.is_dir() => {}
                Ok(_) => continue,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    diagnostics.push(ExternalSourceDiagnostic::warning(
                        "opencode.tool.directory_metadata_failed",
                        format!("Failed to inspect {}: {error}", directory.display_name),
                        Some(source_key),
                    ));
                    continue;
                }
            }
            let mut entries = Vec::new();
            let mut source_has_restrictions = false;
            let directory_entries = match self.read_directory(&directory.path) {
                Ok(entries) => entries,
                Err(error) => {
                    diagnostics.push(ExternalSourceDiagnostic::warning(
                        "opencode.tool.directory_read_failed",
                        format!("Failed to read {}: {error}", directory.display_name),
                        Some(source_key),
                    ));
                    continue;
                }
            };
            for entry in directory_entries {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(error) => {
                        source_has_restrictions = true;
                        diagnostics.push(ExternalSourceDiagnostic::warning(
                            "opencode.tool.directory_entry_failed",
                            format!(
                                "Failed to read an entry in {}: {error}",
                                directory.display_name
                            ),
                            Some(source_key.clone()),
                        ));
                        continue;
                    }
                };
                let path = entry.path();
                if !is_tool_extension(&path) {
                    continue;
                }
                match fs::metadata(&path) {
                    Ok(metadata) if metadata.is_file() => {}
                    Ok(_) => continue,
                    Err(error) => {
                        source_has_restrictions = true;
                        diagnostics.push(ExternalSourceDiagnostic::warning(
                            "opencode.tool.file_metadata_failed",
                            format!(
                                "Failed to inspect tool file '{}': {error}",
                                tool_file_label(&path)
                            ),
                            Some(source_key.clone()),
                        ));
                        continue;
                    }
                }
                entries.push(path);
                if entries.len() > MAX_TOOL_FILES {
                    break;
                }
            }
            entries.sort();
            if entries.len() > MAX_TOOL_FILES {
                diagnostics.push(ExternalSourceDiagnostic::warning(
                    "opencode.tool.file_limit",
                    format!(
                        "{} contains more than {MAX_TOOL_FILES} supported files; additional files were ignored",
                        directory.display_name
                    ),
                    Some(source_key.clone()),
                ));
                entries.truncate(MAX_TOOL_FILES);
            }
            let mut source_versions = Vec::new();
            for path in entries {
                let content = match read_bounded_tool_file(&path) {
                    Ok(Some(content)) => content,
                    Ok(None) => {
                        source_has_restrictions = true;
                        diagnostics.push(ExternalSourceDiagnostic::warning(
                            "opencode.tool.file_too_large",
                            format!(
                                "Tool file '{}' exceeds the supported size limit",
                                tool_file_label(&path)
                            ),
                            Some(source_key.clone()),
                        ));
                        continue;
                    }
                    Err(error) => {
                        source_has_restrictions = true;
                        diagnostics.push(ExternalSourceDiagnostic::warning(
                            "opencode.tool.file_read_failed",
                            format!("Failed to read '{}': {error}", tool_file_label(&path)),
                            Some(source_key.clone()),
                        ));
                        continue;
                    }
                };
                let content_version = content_version(content.as_bytes());
                source_versions.push((path.clone(), content_version.clone()));
                let exports = discover_exports(&content);
                if exports.is_empty() {
                    source_has_restrictions = true;
                    diagnostics.push(ExternalSourceDiagnostic::warning(
                        "opencode.tool.export_missing",
                        format!(
                            "Tool file '{}' has no supported exports",
                            tool_file_label(&path)
                        ),
                        Some(source_key.clone()),
                    ));
                    continue;
                }
                let status = static_status(&path, &content);
                source_has_restrictions |= !matches!(status, ExternalToolStaticStatus::Ready);
                let namespace = path
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or("tool");
                let target = SourceQualifiedToolTargetId::new(
                    source_key.clone(),
                    path.file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or(namespace),
                )
                .map_err(contract_error)?;
                let description_preview = description_preview(&content);
                for export_name in exports {
                    let name = if export_name == "default" {
                        namespace.to_string()
                    } else {
                        format!("{namespace}_{export_name}")
                    };
                    if !is_model_callable_tool_name(&name) {
                        source_has_restrictions = true;
                        diagnostics.push(ExternalSourceDiagnostic::warning(
                            "opencode.tool.name_unsupported",
                            format!(
                                "Tool export '{}' in '{}' does not map to a portable tool name",
                                export_name,
                                tool_file_label(&path)
                            ),
                            Some(source_key.clone()),
                        ));
                        continue;
                    }
                    tools.push(ExternalToolDefinition {
                        id: SourceQualifiedToolId::new(target.clone(), export_name)
                            .map_err(contract_error)?,
                        name,
                        description_preview: description_preview.clone(),
                        module_path: normalize_path(&path).to_string_lossy().into_owned(),
                        working_directory: normalize_path(&working_directory)
                            .to_string_lossy()
                            .into_owned(),
                        runtime_kind: runtime_kind(&path),
                        capabilities: vec![
                            ExternalToolCapability::FileSystem,
                            ExternalToolCapability::Network,
                            ExternalToolCapability::Process,
                            ExternalToolCapability::Environment,
                        ],
                        content_version: content_version.clone(),
                        static_status: status.clone(),
                    });
                }
            }
            if !source_versions.is_empty() {
                sources.push(ExternalSourceRecord {
                    key: source_key,
                    ecosystem_id: EcosystemId::new(ECOSYSTEM_ID).map_err(contract_error)?,
                    display_name: directory.display_name,
                    source_kind: "standalone_tools".to_string(),
                    scope: directory.scope,
                    location: normalize_path(&directory.path)
                        .to_string_lossy()
                        .into_owned(),
                    execution_domain_id: context.execution_domain_id.clone(),
                    health: if source_has_restrictions {
                        ExternalSourceHealth::Partial
                    } else {
                        ExternalSourceHealth::Available
                    },
                    content_version: aggregate_content_version(&source_versions),
                    diagnostics: Vec::new(),
                });
            }
        }
        tools.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
        for diagnostic in &mut diagnostics {
            diagnostic.asset_kind = ExternalSourceAssetKind::Tool;
        }
        for source in &mut sources {
            for diagnostic in &mut source.diagnostics {
                diagnostic.asset_kind = ExternalSourceAssetKind::Tool;
            }
        }
        let snapshot = ExternalToolProviderSnapshot {
            provider,
            sources,
            tools,
            diagnostics,
        };
        snapshot.validate().map_err(contract_error)?;
        Ok(snapshot)
    }

    fn prepare_target(
        &self,
        context: &ExternalSourceContext,
        target_id: &SourceQualifiedToolTargetId,
        expected_content_version: &str,
    ) -> Result<PreparedExternalToolTarget, ExternalSourceProviderError> {
        if target_id.source.provider_id.as_str() != PROVIDER_ID {
            return Err(ExternalSourceProviderError::new(
                "opencode.tool.target_invalid",
                "tool target belongs to another provider",
                false,
            ));
        }
        let directory = self
            .tool_directories(context)
            .into_iter()
            .find(|directory| {
                source_key(&directory.path)
                    .ok()
                    .is_some_and(|source| source == target_id.source)
            })
            .ok_or_else(|| {
                ExternalSourceProviderError::new(
                    "opencode.tool.source_missing",
                    "tool source is no longer configured",
                    true,
                )
            })?;
        let local_id = target_id.local_id.as_str();
        let relative = Path::new(local_id);
        if relative.components().count() != 1
            || relative.file_name().and_then(|name| name.to_str()) != Some(local_id)
            || !is_tool_extension(relative)
        {
            return Err(ExternalSourceProviderError::new(
                "opencode.tool.target_invalid",
                "tool target is not a supported file in its source directory",
                false,
            ));
        }
        let path = directory.path.join(relative);
        let content = read_bounded_tool_file(&path)
            .map_err(|error| io_error("opencode.tool.file_read_failed", &path, error))?
            .ok_or_else(|| {
                ExternalSourceProviderError::new(
                    "opencode.tool.file_too_large",
                    "tool changed after preview and now exceeds the file size limit",
                    true,
                )
            })?;
        if content_version(content.as_bytes()) != expected_content_version {
            return Err(ExternalSourceProviderError::new(
                "opencode.tool.stale_revision",
                "tool changed after preview; refresh before enabling it",
                true,
            ));
        }
        if !matches!(
            static_status(&path, &content),
            ExternalToolStaticStatus::Ready
        ) {
            return Err(ExternalSourceProviderError::new(
                "opencode.tool.unsupported",
                "tool is outside the supported single-file JavaScript subset",
                false,
            ));
        }
        let namespace = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("tool");
        let expected_tools = discover_exports(&content)
            .into_iter()
            .filter_map(|export_name| {
                let tool_name = if export_name == "default" {
                    namespace.to_string()
                } else {
                    format!("{namespace}_{export_name}")
                };
                is_model_callable_tool_name(&tool_name).then_some(PreparedExternalToolExport {
                    export_name,
                    tool_name,
                })
            })
            .collect::<Vec<_>>();
        if expected_tools.is_empty() {
            return Err(ExternalSourceProviderError::new(
                "opencode.tool.export_missing",
                "tool has no supported exports",
                false,
            ));
        }
        let replaced = allowed_import_regex().replace_all(&content, "");
        let removed_import = matches!(replaced, std::borrow::Cow::Owned(_));
        let module_source = replaced.into_owned();
        let module_source = if removed_import {
            format!("{TOOL_SHIM}\n{module_source}")
        } else {
            module_source
        };
        Ok(PreparedExternalToolTarget {
            target_id: target_id.clone(),
            content_version: expected_content_version.to_string(),
            module_source,
            module_url: file_url(&path)?,
            working_directory: normalize_path(&self.working_directory(context))
                .to_string_lossy()
                .into_owned(),
            worktree_root: context.workspace_root.as_ref().map(|workspace| {
                normalize_path(&find_project_root(workspace))
                    .to_string_lossy()
                    .into_owned()
            }),
            expected_tools,
        })
    }

    fn watch_roots(&self, context: &ExternalSourceContext) -> Vec<ExternalWatchRoot> {
        let mut roots = BTreeMap::new();
        for directory in self.tool_directories(context) {
            if let Some(parent) = directory.path.parent() {
                if let Some(existing) = nearest_existing_path(parent.to_path_buf()) {
                    roots.entry(existing).or_insert(false);
                }
            }
            roots
                .entry(directory.path)
                .and_modify(|recursive| *recursive = true)
                .or_insert(true);
        }
        roots
            .into_iter()
            .map(|(path, recursive)| ExternalWatchRoot { path, recursive })
            .collect()
    }
}

impl OpenCodeToolProvider {
    fn working_directory(&self, context: &ExternalSourceContext) -> PathBuf {
        let effective_global_config = self
            .options
            .explicit_config_dir
            .as_ref()
            .unwrap_or(&self.options.user_config_dir);
        context
            .workspace_root
            .clone()
            .or_else(|| effective_global_config.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| effective_global_config.clone())
    }
}

#[derive(Debug, Clone)]
struct ToolDirectory {
    path: PathBuf,
    scope: ExternalSourceScope,
    display_name: String,
}

fn push_tool_directories(
    directories: &mut Vec<ToolDirectory>,
    base: &Path,
    scope: ExternalSourceScope,
    display_name: &str,
) {
    for folder in ["tool", "tools"] {
        directories.push(ToolDirectory {
            path: base.join(folder),
            scope,
            display_name: format!("{display_name} ({folder})"),
        });
    }
}

fn deduplicate_directories(directories: Vec<ToolDirectory>) -> Vec<ToolDirectory> {
    let mut seen = BTreeSet::new();
    directories
        .into_iter()
        .filter(|directory| seen.insert(normalize_path(&directory.path)))
        .collect()
}

fn source_key(path: &Path) -> Result<SourceKey, ExternalSourceProviderError> {
    let location = normalize_path(path).to_string_lossy().into_owned();
    let digest = Sha256::digest(location.as_bytes());
    SourceKey::new(
        PROVIDER_ID,
        format!("directory-{}", hex::encode(&digest[..12])),
    )
    .map_err(contract_error)
}

fn discover_exports(content: &str) -> Vec<String> {
    let mut exports = BTreeSet::new();
    if default_export_regex().is_match(content) {
        exports.insert("default".to_string());
    }
    for captures in named_export_regex().captures_iter(content) {
        if let Some(name) = captures.get(1) {
            exports.insert(name.as_str().to_string());
        }
    }
    exports.into_iter().collect()
}

fn static_status(path: &Path, content: &str) -> ExternalToolStaticStatus {
    if path.extension().and_then(|value| value.to_str()) == Some("ts") {
        return ExternalToolStaticStatus::Unsupported {
            reason: "TypeScript tools are recognized but are not executable yet".to_string(),
        };
    }
    let without_allowed_import = allowed_import_regex().replace_all(content, "");
    if other_import_regex().is_match(&without_allowed_import)
        || dynamic_import_regex().is_match(&without_allowed_import)
        || require_regex().is_match(&without_allowed_import)
    {
        return ExternalToolStaticStatus::Unsupported {
            reason: "Only single-file JavaScript tools without supported imports can run"
                .to_string(),
        };
    }
    ExternalToolStaticStatus::Ready
}

fn description_preview(content: &str) -> String {
    description_regex()
        .captures(content)
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().to_string())
        .unwrap_or_default()
}

fn content_version(content: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(content)))
}

fn aggregate_content_version(entries: &[(PathBuf, String)]) -> String {
    let mut hasher = Sha256::new();
    for (path, version) in entries {
        hasher.update(normalize_path(path).to_string_lossy().as_bytes());
        hasher.update([0]);
        hasher.update(version.as_bytes());
        hasher.update([0]);
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn normalize_path(path: &Path) -> PathBuf {
    dunce::canonicalize(path).unwrap_or_else(|_| {
        let mut normalized = PathBuf::new();
        for component in path.components() {
            normalized.push(component.as_os_str());
        }
        normalized
    })
}

fn tool_file_label(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("tool file")
        .to_string()
}

fn runtime_kind(path: &Path) -> ExternalToolRuntimeKind {
    match path.extension().and_then(|value| value.to_str()) {
        Some("ts") => ExternalToolRuntimeKind::TypeScript,
        _ => ExternalToolRuntimeKind::JavaScript,
    }
}

fn is_tool_extension(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("js" | "ts")
    )
}

fn is_model_callable_tool_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn read_bounded_tool_file(path: &Path) -> std::io::Result<Option<String>> {
    let file = fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(MAX_TOOL_FILE_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_TOOL_FILE_BYTES {
        return Ok(None);
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

fn file_url(path: &Path) -> Result<String, ExternalSourceProviderError> {
    let normalized = normalize_path(path);
    url::Url::from_file_path(&normalized)
        .map(|url| url.to_string())
        .map_err(|_| {
            ExternalSourceProviderError::new(
                "opencode.tool.module_url_invalid",
                format!(
                    "Could not convert tool file '{}' to an absolute file URL",
                    tool_file_label(&normalized)
                ),
                false,
            )
        })
}

fn io_error(code: &str, path: &Path, error: std::io::Error) -> ExternalSourceProviderError {
    ExternalSourceProviderError::new(
        code,
        format!("Failed to read '{}': {error}", tool_file_label(path)),
        true,
    )
}

fn contract_error(error: impl std::fmt::Display) -> ExternalSourceProviderError {
    ExternalSourceProviderError::new("opencode.tool.contract_invalid", error.to_string(), false)
}

fn environment_truthy(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .is_some_and(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
}

fn find_project_root(start: &Path) -> PathBuf {
    let mut current = normalize_path(start);
    loop {
        if current.join(".git").exists() {
            return current;
        }
        if !current.pop() {
            return normalize_path(start);
        }
    }
}

fn directories_between(root: &Path, opened: &Path) -> Vec<PathBuf> {
    let root = normalize_path(root);
    let mut current = normalize_path(opened);
    let mut directories = Vec::new();
    while current.starts_with(&root) {
        directories.push(current.clone());
        if current == root || !current.pop() {
            break;
        }
    }
    directories.reverse();
    directories
}

fn nearest_existing_path(mut path: PathBuf) -> Option<PathBuf> {
    loop {
        if path.exists() {
            return Some(path);
        }
        if !path.pop() {
            return None;
        }
    }
}

fn allowed_import_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r#"(?m)^\s*import\s*\{\s*tool\s*\}\s*from\s*["']@opencode-ai/plugin["']\s*;?\s*$"#,
        )
        .expect("valid allowed import regex")
    })
}

fn other_import_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"(?m)^\s*(?:import|export\s+.+\s+from)\b").unwrap())
}

fn dynamic_import_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?s)\bimport(?:\s|/\*.*?\*/|//[^\r\n]*(?:\r?\n|$))*\(").unwrap()
    })
}

fn require_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\brequire\s*\(").unwrap())
}

fn default_export_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"(?m)\bexport\s+default\s+(?:tool\s*\(|\{)").unwrap())
}

fn named_export_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?m)\bexport\s+const\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*=\s*tool\s*\(").unwrap()
    })
}

fn description_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r#"description\s*:\s*["']([^"'\r\n]{1,512})["']"#).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_url_percent_encodes_reserved_and_unicode_path_characters() {
        let path = std::env::current_dir()
            .unwrap()
            .join("nested folder")
            .join("tool%#?-测试.js");

        let encoded = file_url(&path).expect("absolute path must become a file URL");
        let parsed = url::Url::parse(&encoded).expect("valid file URL");

        assert!(encoded.starts_with("file:"));
        assert!(encoded.contains("%25"));
        assert!(encoded.contains("%23"));
        assert!(encoded.contains("%3F"));
        assert!(encoded.contains("%20"));
        assert_eq!(parsed.to_file_path().expect("file URL path"), path);
    }

    #[test]
    fn directory_read_failure_preserves_tools_from_healthy_directories() {
        let temp = tempfile::tempdir().unwrap();
        let global = temp.path().join("global");
        let workspace = temp.path().join("workspace");
        let failed_directory = global.join("tools");
        let healthy_directory = workspace.join(".opencode/tools");
        fs::create_dir_all(&failed_directory).unwrap();
        fs::create_dir_all(&healthy_directory).unwrap();
        fs::write(
            healthy_directory.join("healthy.js"),
            r#"export default { description: "healthy", args: {}, execute() { return "ok" } }"#,
        )
        .unwrap();

        let failed_source = source_key(&failed_directory).unwrap();
        let failed_directory_for_reader = failed_directory.clone();
        let provider = OpenCodeToolProvider::new(OpenCodeToolProviderOptions {
            user_config_dir: global,
            legacy_user_config_dir: None,
            explicit_config_dir: None,
            project_config_enabled: true,
        })
        .with_directory_reader(move |path| {
            if path == failed_directory_for_reader {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "directory is unreadable",
                ));
            }
            fs::read_dir(path)
        });
        let context = ExternalSourceContext {
            workspace_root: Some(workspace),
            execution_domain_id: bitfun_product_domains::external_sources::ExecutionDomainId::new(
                "local-user",
            )
            .unwrap(),
        };

        let snapshot = provider
            .discover(&context)
            .expect("one unreadable directory must not discard healthy directory results");

        assert_eq!(
            snapshot
                .tools
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            vec!["healthy"]
        );
        assert!(snapshot
            .sources
            .iter()
            .all(|source| source.key != failed_source));
        let diagnostic = snapshot
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "opencode.tool.directory_read_failed")
            .expect("failed directory must produce a diagnostic");
        assert_eq!(diagnostic.source.as_ref(), Some(&failed_source));
        assert!(diagnostic.message.contains("OpenCode user tools"));
        assert!(!diagnostic
            .message
            .contains(temp.path().to_string_lossy().as_ref()));
    }
}
