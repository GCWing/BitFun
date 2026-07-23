use crate::{
    command_source::{opencode_user_config_dir, strip_jsonc},
    source_adapter::statically_discover_hook_events,
};
use bitfun_product_domains::external_hook_catalog::{
    ExternalHookCatalogEntry, ExternalHookHandlerKind, ExternalHookMapping,
    ExternalHookMatcherSummary, ExternalHookNativeActivation, ExternalHookProjectionStatus,
    ExternalHookProviderIdentity, ExternalHookProviderSnapshot, ExternalHookSource,
    ExternalHookSourceKind, ExternalHookSourceProvider,
};
use bitfun_product_domains::external_hook_contributions::ExternalHookPoint;
use bitfun_product_domains::external_sources::{
    EcosystemId, ExternalSourceAssetKind, ExternalSourceContext, ExternalSourceDiagnostic,
    ExternalSourceDiagnosticSeverity, ExternalSourceHealth, ExternalSourceProviderError,
    ExternalSourceScope, SourceKey,
};
use bitfun_static_hook_support::{
    bounded_project_ancestors, read_bounded_file, regular_file_exists, BoundedFileRead,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const PROVIDER_ID: &str = "opencode.hooks";
const ECOSYSTEM_ID: &str = "opencode";
const MAX_PLUGIN_FILES: usize = 128;
const MAX_PLUGIN_DIRECTORY_ENTRIES: usize = 1024;
const MAX_HOOK_ENTRIES: usize = 2048;
const MAX_PLUGIN_FILE_BYTES: usize = 512 * 1024;
const MAX_CONFIG_FILE_BYTES: usize = 1024 * 1024;
const MAX_PACKAGE_DECLARATIONS: usize = 128;
const MAX_PROJECT_ANCESTORS: usize = 32;
const GLOBAL_CONFIG_NAMES: &[&str] = &["config.json", "opencode.json", "opencode.jsonc"];
const DIRECTORY_CONFIG_NAMES: &[&str] = &["opencode.json", "opencode.jsonc"];

#[derive(Debug, Clone)]
pub struct OpenCodeHookProviderOptions {
    pub user_config_dir: PathBuf,
    pub legacy_user_config_dir: Option<PathBuf>,
    pub explicit_config_dir: Option<PathBuf>,
    pub project_config_enabled: bool,
    pub project_root_override: Option<PathBuf>,
}

impl OpenCodeHookProviderOptions {
    pub fn from_environment() -> Self {
        let home = dirs::home_dir();
        let user_config_dir = opencode_user_config_dir(
            std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
            home.clone(),
        );
        Self {
            user_config_dir,
            legacy_user_config_dir: home.map(|home| home.join(".opencode")),
            explicit_config_dir: std::env::var_os("OPENCODE_CONFIG_DIR").map(PathBuf::from),
            project_config_enabled: !environment_truthy("OPENCODE_DISABLE_PROJECT_CONFIG"),
            project_root_override: None,
        }
    }
}

impl Default for OpenCodeHookProviderOptions {
    fn default() -> Self {
        Self::from_environment()
    }
}

pub struct OpenCodeHookProvider {
    options: OpenCodeHookProviderOptions,
}

impl OpenCodeHookProvider {
    pub fn new(options: OpenCodeHookProviderOptions) -> Self {
        Self { options }
    }

    fn project_roots(&self, workspace_root: &Path) -> Vec<PathBuf> {
        let boundary = self
            .options
            .project_root_override
            .as_deref()
            .unwrap_or(workspace_root);
        bounded_project_ancestors(workspace_root, boundary, MAX_PROJECT_ANCESTORS)
    }
}

impl Default for OpenCodeHookProvider {
    fn default() -> Self {
        Self::new(OpenCodeHookProviderOptions::default())
    }
}

impl ExternalHookSourceProvider for OpenCodeHookProvider {
    fn identity(&self) -> ExternalHookProviderIdentity {
        ExternalHookProviderIdentity::new(PROVIDER_ID, ECOSYSTEM_ID, "OpenCode Hooks")
            .expect("static OpenCode Hook provider identity must be valid")
    }

    fn discover(
        &self,
        context: &ExternalSourceContext,
    ) -> Result<ExternalHookProviderSnapshot, ExternalSourceProviderError> {
        if context
            .workspace_root
            .as_ref()
            .is_some_and(|workspace_root| !workspace_root.is_absolute())
        {
            return Err(ExternalSourceProviderError::new(
                "opencode.hook.workspace_invalid",
                "workspace root must be absolute",
                false,
            ));
        }

        let mut layers = vec![HookLayer::new(
            self.options.user_config_dir.clone(),
            ExternalSourceScope::UserGlobal,
            "OpenCode user configuration",
            "OpenCode user plugins",
            GLOBAL_CONFIG_NAMES,
            true,
        )];
        if let Some(path) = &self.options.legacy_user_config_dir {
            layers.push(HookLayer::new(
                path.clone(),
                ExternalSourceScope::UserGlobal,
                "OpenCode legacy configuration",
                "OpenCode legacy plugins",
                GLOBAL_CONFIG_NAMES,
                true,
            ));
        }
        if self.options.project_config_enabled {
            if let Some(workspace_root) = &context.workspace_root {
                for project_root in self.project_roots(workspace_root) {
                    layers.push(HookLayer::new(
                        project_root.clone(),
                        ExternalSourceScope::Project,
                        "OpenCode project configuration",
                        "OpenCode project plugins",
                        DIRECTORY_CONFIG_NAMES,
                        false,
                    ));
                    layers.push(HookLayer::new(
                        project_root.join(".opencode"),
                        ExternalSourceScope::Project,
                        "OpenCode project directory configuration",
                        "OpenCode project plugins",
                        DIRECTORY_CONFIG_NAMES,
                        true,
                    ));
                }
            }
        }
        // OpenCode applies OPENCODE_CONFIG_DIR after automatically discovered
        // project directories, so preserve that precedence in the projection.
        if let Some(path) = &self.options.explicit_config_dir {
            layers.push(HookLayer::new(
                path.clone(),
                ExternalSourceScope::WorkspaceLocal,
                "OpenCode explicit configuration",
                "OpenCode explicit plugins",
                DIRECTORY_CONFIG_NAMES,
                true,
            ));
        }
        deduplicate_layers(&mut layers);

        let mut sources = Vec::new();
        let mut entries = Vec::new();
        let mut diagnostics = Vec::new();
        let mut remaining_files = MAX_PLUGIN_FILES;
        let mut remaining_entries = MAX_HOOK_ENTRIES;
        let mut remaining_packages = MAX_PACKAGE_DECLARATIONS;
        let mut package_limit_reported = false;
        let mut file_limit_reported = false;
        let mut directory_limit_reported = false;
        let mut entry_limit_reported = false;
        for layer in layers {
            discover_package_declarations(
                &layer,
                &mut remaining_packages,
                &mut package_limit_reported,
                &mut sources,
                &mut diagnostics,
            )?;
            if layer.scan_plugin_dirs {
                for directory_name in ["plugin", "plugins"] {
                    discover_plugin_files(
                        &layer,
                        directory_name,
                        &mut remaining_files,
                        &mut remaining_entries,
                        &mut file_limit_reported,
                        &mut directory_limit_reported,
                        &mut entry_limit_reported,
                        &mut sources,
                        &mut entries,
                        &mut diagnostics,
                    )?;
                }
            }
        }
        diagnostics
            .sort_by(|left, right| (&left.code, &left.message).cmp(&(&right.code, &right.message)));

        let snapshot = ExternalHookProviderSnapshot {
            provider: self.identity(),
            sources,
            entries,
            diagnostics,
        };
        snapshot.validate().map_err(|error| {
            ExternalSourceProviderError::new(
                "opencode.hook.snapshot_invalid",
                error.to_string(),
                false,
            )
        })?;
        Ok(snapshot)
    }
}

struct HookLayer {
    root: PathBuf,
    scope: ExternalSourceScope,
    config_label: &'static str,
    plugin_label: &'static str,
    config_names: &'static [&'static str],
    scan_plugin_dirs: bool,
}

impl HookLayer {
    fn new(
        root: PathBuf,
        scope: ExternalSourceScope,
        config_label: &'static str,
        plugin_label: &'static str,
        config_names: &'static [&'static str],
        scan_plugin_dirs: bool,
    ) -> Self {
        Self {
            root,
            scope,
            config_label,
            plugin_label,
            config_names,
            scan_plugin_dirs,
        }
    }
}

fn deduplicate_layers(layers: &mut Vec<HookLayer>) {
    let mut seen = BTreeSet::new();
    layers.retain(|layer| seen.insert(layer.root.clone()));
}

fn discover_package_declarations(
    layer: &HookLayer,
    remaining_packages: &mut usize,
    package_limit_reported: &mut bool,
    sources: &mut Vec<ExternalHookSource>,
    diagnostics: &mut Vec<ExternalSourceDiagnostic>,
) -> Result<(), ExternalSourceProviderError> {
    for config_name in layer.config_names {
        let config_path = layer.root.join(config_name);
        match regular_file_exists(&config_path) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(error) => {
                return Err(ExternalSourceProviderError::new(
                    "opencode.hook.config_metadata_failed",
                    format!("OpenCode Hook configuration metadata is unavailable: {error}"),
                    true,
                ))
            }
        }
        let bytes = match read_bounded_file(&config_path, MAX_CONFIG_FILE_BYTES) {
            Ok(BoundedFileRead::Content(bytes)) => bytes,
            Ok(BoundedFileRead::TooLarge) => {
                diagnostics.push(hook_warning(
                    "opencode.hook.config_too_large",
                    "OpenCode Hook configuration exceeds the 1 MiB inspection limit",
                    None,
                ));
                continue;
            }
            Err(error) => {
                return Err(ExternalSourceProviderError::new(
                    "opencode.hook.config_unreadable",
                    format!("OpenCode Hook configuration could not be read: {error}"),
                    true,
                ))
            }
        };
        let value = std::str::from_utf8(&bytes)
            .ok()
            .and_then(|source| serde_json::from_str::<Value>(&strip_jsonc(source)).ok());
        let Some(value) = value else {
            diagnostics.push(hook_warning(
                "opencode.hook.config_parse_failed",
                "OpenCode Hook configuration is not valid JSON or JSONC",
                None,
            ));
            continue;
        };
        let Some(packages) = value.get("plugin").and_then(Value::as_array) else {
            continue;
        };
        for (index, package) in packages.iter().enumerate() {
            if *remaining_packages == 0 {
                if !*package_limit_reported {
                    diagnostics.push(hook_warning(
                        "opencode.hook.package_limit",
                        "Additional OpenCode plugin declarations were omitted after the 128 item inspection limit",
                        None,
                    ));
                    *package_limit_reported = true;
                }
                return Ok(());
            }
            *remaining_packages -= 1;
            let Some(_specifier) = plugin_specifier(package) else {
                continue;
            };
            let source_key = source_key("package", &format!("{}:{index}", config_path.display()));
            let diagnostic = hook_info(
                "opencode.hook.package_declared_only",
                "OpenCode plugin is declared, but its Hook exports are not inspected until a separate runtime host resolves the declaration",
                Some(source_key.clone()),
            );
            sources.push(ExternalHookSource {
                key: source_key.clone(),
                ecosystem_id: ecosystem_id(),
                display_name: "OpenCode configured plugin".to_string(),
                source_kind: ExternalHookSourceKind::PackageDeclaration,
                scope: layer.scope,
                location_hint: format!("{}/{}", layer.config_label, config_name),
                health: ExternalSourceHealth::Partial,
                content_version: content_hash(b"package-declaration:redacted"),
                diagnostics: vec![diagnostic.clone()],
            });
            diagnostics.push(diagnostic);
        }
    }
    Ok(())
}

fn plugin_specifier(value: &Value) -> Option<&str> {
    value
        .as_str()
        .or_else(|| value.as_array()?.first()?.as_str())
        .filter(|value| !value.trim().is_empty())
}

fn discover_plugin_files(
    layer: &HookLayer,
    directory_name: &str,
    remaining_files: &mut usize,
    remaining_entries: &mut usize,
    file_limit_reported: &mut bool,
    directory_limit_reported: &mut bool,
    entry_limit_reported: &mut bool,
    sources: &mut Vec<ExternalHookSource>,
    entries: &mut Vec<ExternalHookCatalogEntry>,
    diagnostics: &mut Vec<ExternalSourceDiagnostic>,
) -> Result<(), ExternalSourceProviderError> {
    if *remaining_entries == 0 {
        report_entry_limit(entry_limit_reported, diagnostics);
        return Ok(());
    }
    let directory = layer.root.join(directory_name);
    let read_dir = match fs::read_dir(&directory) {
        Ok(read_dir) => read_dir,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(ExternalSourceProviderError::new(
                "opencode.hook.plugin_directory_unreadable",
                format!("OpenCode plugin directory could not be read: {error}"),
                true,
            ))
        }
    };
    let mut paths = Vec::new();
    for (index, entry) in read_dir.enumerate() {
        if index == MAX_PLUGIN_DIRECTORY_ENTRIES {
            if !*directory_limit_reported {
                diagnostics.push(hook_warning(
                    "opencode.hook.plugin_directory_limit",
                    "Additional OpenCode plugin directory entries were omitted after the 1024 item inspection limit",
                    None,
                ));
                *directory_limit_reported = true;
            }
            break;
        }
        let entry = entry.map_err(|error| {
            ExternalSourceProviderError::new(
                "opencode.hook.plugin_directory_entry_unreadable",
                format!("OpenCode plugin directory entry could not be read: {error}"),
                true,
            )
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            ExternalSourceProviderError::new(
                "opencode.hook.plugin_metadata_failed",
                format!("OpenCode plugin metadata is unavailable: {error}"),
                true,
            )
        })?;
        let is_file = if file_type.is_symlink() {
            match fs::metadata(&path) {
                Ok(metadata) => metadata.is_file(),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    diagnostics.push(hook_warning(
                        "opencode.hook.plugin_symlink_unavailable",
                        "An OpenCode plugin symlink target is unavailable and was omitted",
                        None,
                    ));
                    false
                }
                Err(error) => {
                    return Err(ExternalSourceProviderError::new(
                        "opencode.hook.plugin_metadata_failed",
                        format!("OpenCode plugin symlink metadata is unavailable: {error}"),
                        true,
                    ));
                }
            }
        } else {
            file_type.is_file()
        };
        if is_file
            && matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("js" | "ts")
            )
            && !path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".d.ts"))
        {
            paths.push(path);
        }
    }
    paths.sort();

    for path in paths {
        if *remaining_files == 0 {
            if !*file_limit_reported {
                diagnostics.push(hook_warning(
                    "opencode.hook.plugin_file_limit",
                    "Additional OpenCode plugin files were omitted after the 128 file inspection limit",
                    None,
                ));
                *file_limit_reported = true;
            }
            break;
        }
        if *remaining_entries == 0 {
            report_entry_limit(entry_limit_reported, diagnostics);
            break;
        }
        *remaining_files -= 1;
        let source_key = source_key("plugin", &path.to_string_lossy());
        let location_hint = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| format!("{} ({directory_name})/{name}", layer.plugin_label))
            .unwrap_or_else(|| layer.plugin_label.to_string());
        let bytes = match read_bounded_file(&path, MAX_PLUGIN_FILE_BYTES) {
            Ok(BoundedFileRead::Content(bytes)) => bytes,
            Ok(BoundedFileRead::TooLarge) => {
                let diagnostic = hook_warning(
                    "opencode.hook.plugin_too_large",
                    "OpenCode plugin exceeds the 512 KiB static inspection limit",
                    Some(source_key.clone()),
                );
                sources.push(source(
                    source_key,
                    layer.scope,
                    location_hint,
                    ExternalSourceHealth::Unavailable,
                    "unavailable:too-large".to_string(),
                    vec![diagnostic.clone()],
                ));
                diagnostics.push(diagnostic);
                continue;
            }
            Err(error) => {
                return Err(ExternalSourceProviderError::new(
                    "opencode.hook.plugin_unreadable",
                    format!(
                        "OpenCode plugin could not be read for static Hook inspection: {error}"
                    ),
                    true,
                ))
            }
        };
        let parsed = std::str::from_utf8(&bytes)
            .map_err(|_| "source is not UTF-8".to_string())
            .and_then(|source| statically_discover_hook_events(&path, source));
        match parsed {
            Ok(discovery) => {
                let version = redacted_discovery_version(&discovery);
                let mut source_diagnostics = Vec::new();
                let has_opaque = !discovery.opaque_events.is_empty()
                    || !discovery.dynamic_registrations.is_empty();
                for event in discovery.events {
                    if !take_entry(remaining_entries, entry_limit_reported, diagnostics) {
                        break;
                    }
                    entries.push(entry(
                        &source_key,
                        event.native_event,
                        &event.registration_id,
                    ));
                }
                for event in discovery.opaque_events {
                    if !take_entry(remaining_entries, entry_limit_reported, diagnostics) {
                        break;
                    }
                    entries.push(opaque_entry(
                        &source_key,
                        event.native_event,
                        &event.registration_id,
                        ExternalHookMatcherSummary::Unavailable,
                    ));
                }
                for registration_id in discovery.dynamic_registrations {
                    if !take_entry(remaining_entries, entry_limit_reported, diagnostics) {
                        break;
                    }
                    entries.push(opaque_entry(
                        &source_key,
                        "<dynamic>".to_string(),
                        &registration_id,
                        ExternalHookMatcherSummary::Dynamic,
                    ));
                }
                if has_opaque {
                    let diagnostic = hook_info(
                        "opencode.hook.registration_opaque",
                        "OpenCode plugin contains Hook registration that cannot be mapped safely by static inspection",
                        Some(source_key.clone()),
                    );
                    source_diagnostics.push(diagnostic.clone());
                    diagnostics.push(diagnostic);
                }
                sources.push(source(
                    source_key,
                    layer.scope,
                    location_hint,
                    if source_diagnostics.is_empty() {
                        ExternalSourceHealth::Available
                    } else {
                        ExternalSourceHealth::Partial
                    },
                    version,
                    source_diagnostics,
                ));
            }
            Err(_) => {
                let diagnostic = hook_warning(
                    "opencode.hook.plugin_parse_failed",
                    "OpenCode plugin could not be parsed statically; no handler was loaded or executed",
                    Some(source_key.clone()),
                );
                sources.push(source(
                    source_key,
                    layer.scope,
                    location_hint,
                    ExternalSourceHealth::Degraded,
                    "invalid:parse".to_string(),
                    vec![diagnostic.clone()],
                ));
                diagnostics.push(diagnostic);
            }
        }
    }
    Ok(())
}

fn take_entry(
    remaining_entries: &mut usize,
    entry_limit_reported: &mut bool,
    diagnostics: &mut Vec<ExternalSourceDiagnostic>,
) -> bool {
    if *remaining_entries == 0 {
        report_entry_limit(entry_limit_reported, diagnostics);
        return false;
    }
    *remaining_entries -= 1;
    true
}

fn report_entry_limit(
    entry_limit_reported: &mut bool,
    diagnostics: &mut Vec<ExternalSourceDiagnostic>,
) {
    if !*entry_limit_reported {
        diagnostics.push(hook_warning(
            "opencode.hook.entry_limit",
            "Additional OpenCode Hook registrations were omitted after the 2048 entry inspection limit",
            None,
        ));
        *entry_limit_reported = true;
    }
}

fn source(
    key: SourceKey,
    scope: ExternalSourceScope,
    location_hint: String,
    health: ExternalSourceHealth,
    content_version: String,
    diagnostics: Vec<ExternalSourceDiagnostic>,
) -> ExternalHookSource {
    ExternalHookSource {
        key,
        ecosystem_id: ecosystem_id(),
        display_name: location_hint.clone(),
        source_kind: ExternalHookSourceKind::PluginFile,
        scope,
        location_hint,
        health,
        content_version,
        diagnostics,
    }
}

fn entry(
    source: &SourceKey,
    native_event: String,
    registration_id: &str,
) -> ExternalHookCatalogEntry {
    let mapping = match native_event.as_str() {
        "tool.execute.before" => Some(ExternalHookMapping {
            hook_point: ExternalHookPoint::ToolBefore,
        }),
        "tool.execute.after" => Some(ExternalHookMapping {
            hook_point: ExternalHookPoint::ToolAfter,
        }),
        _ => None,
    };
    let stable_key = format!(
        "opencode-hook:{}",
        short_hash(format!("{}:{registration_id}:{native_event}", source.stable_key()).as_bytes()),
    );
    ExternalHookCatalogEntry {
        content_version: content_hash(
            format!("static:{registration_id}:{native_event}:any:function").as_bytes(),
        ),
        stable_key,
        source: source.clone(),
        native_event,
        matcher: ExternalHookMatcherSummary::Any,
        handler_kind: ExternalHookHandlerKind::Function,
        projection_status: if mapping.is_some() {
            ExternalHookProjectionStatus::Mapped
        } else {
            ExternalHookProjectionStatus::NativeOnly
        },
        native_activation: ExternalHookNativeActivation::Unknown,
        mapping,
    }
}

fn opaque_entry(
    source: &SourceKey,
    native_event: String,
    registration_id: &str,
    matcher: ExternalHookMatcherSummary,
) -> ExternalHookCatalogEntry {
    let matcher_version = match &matcher {
        ExternalHookMatcherSummary::Dynamic => "dynamic",
        _ => "unavailable",
    };
    ExternalHookCatalogEntry {
        stable_key: format!(
            "opencode-hook:{}",
            short_hash(
                format!(
                    "{}:{registration_id}:{native_event}:opaque",
                    source.stable_key()
                )
                .as_bytes()
            )
        ),
        source: source.clone(),
        native_event: native_event.clone(),
        matcher,
        handler_kind: ExternalHookHandlerKind::Function,
        projection_status: ExternalHookProjectionStatus::Opaque,
        native_activation: ExternalHookNativeActivation::Unknown,
        mapping: None,
        content_version: content_hash(
            format!("opaque:{registration_id}:{native_event}:{matcher_version}").as_bytes(),
        ),
    }
}

fn source_key(kind: &str, identity: &str) -> SourceKey {
    SourceKey::new(
        PROVIDER_ID,
        format!("{kind}-{}", short_hash(identity.as_bytes())),
    )
    .expect("hashed OpenCode Hook source key must be valid")
}

fn ecosystem_id() -> EcosystemId {
    EcosystemId::new(ECOSYSTEM_ID).expect("static ecosystem id must be valid")
}

fn redacted_discovery_version(
    discovery: &crate::source_adapter::StaticHookEventDiscovery,
) -> String {
    let mut hasher = Sha256::new();
    for event in &discovery.events {
        hasher.update(b"event:");
        hasher.update(event.registration_id.as_bytes());
        hasher.update([0]);
        hasher.update(event.native_event.as_bytes());
        hasher.update([0]);
    }
    for event in &discovery.opaque_events {
        hasher.update(b"opaque:");
        hasher.update(event.registration_id.as_bytes());
        hasher.update([0]);
        hasher.update(event.native_event.as_bytes());
        hasher.update([0]);
    }
    for registration_id in &discovery.dynamic_registrations {
        hasher.update(b"dynamic:");
        hasher.update(registration_id.as_bytes());
        hasher.update([0]);
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn content_hash(value: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(value)))
}

fn short_hash(value: &[u8]) -> String {
    hex::encode(Sha256::digest(value))[..24].to_string()
}

fn hook_warning(code: &str, message: &str, source: Option<SourceKey>) -> ExternalSourceDiagnostic {
    ExternalSourceDiagnostic::warning(code, message, source)
        .with_asset_kind(ExternalSourceAssetKind::Hook)
}

fn hook_info(code: &str, message: &str, source: Option<SourceKey>) -> ExternalSourceDiagnostic {
    ExternalSourceDiagnostic {
        severity: ExternalSourceDiagnosticSeverity::Info,
        asset_kind: ExternalSourceAssetKind::Hook,
        code: code.to_string(),
        message: message.to_string(),
        source,
    }
}

fn environment_truthy(name: &str) -> bool {
    std::env::var(name)
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
}
