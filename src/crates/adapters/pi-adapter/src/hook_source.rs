use openbitfun_product_domains::external_hook_catalog::*;
use openbitfun_product_domains::external_sources::*;
use openbitfun_static_hook_support::{
    read_bounded_file, regular_file_exists, BoundedFileRead, StaticHookCatalog,
};
use oxc_parse::{
    allocator::Allocator,
    ast::ast::*,
    ast_visit::{walk, Visit},
    parser::Parser,
    span::SourceType,
};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

const MAX_FILE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct PiHookProviderOptions {
    pub agent_dir: PathBuf,
}

impl Default for PiHookProviderOptions {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        let agent_dir = std::env::var("PI_CODING_AGENT_DIR")
            .ok()
            .filter(|value| !value.is_empty())
            .map(|value| expand_path(&value, &home, Path::new(".")))
            .unwrap_or_else(|| home.join(".pi/agent"));
        Self { agent_dir }
    }
}

#[derive(Default)]
pub struct PiHookProvider {
    options: PiHookProviderOptions,
}
impl PiHookProvider {
    pub fn new(options: PiHookProviderOptions) -> Self {
        Self { options }
    }
}

fn error() -> ExternalSourceProviderError {
    ExternalSourceProviderError::new(
        "pi.hook.read_failed",
        "Could not read PI extension sources",
        true,
    )
}

fn expand_path(value: &str, home: &Path, base: &Path) -> PathBuf {
    if value == "~" {
        home.to_path_buf()
    } else if let Some(path) = value.strip_prefix("~/") {
        home.join(path)
    } else {
        base.join(value)
    }
}

fn read(path: &Path) -> Result<Option<Vec<u8>>, ExternalSourceProviderError> {
    if !regular_file_exists(path).map_err(|_| error())? {
        return Ok(None);
    }
    match read_bounded_file(path, MAX_FILE_BYTES).map_err(|_| error())? {
        BoundedFileRead::Content(bytes) => Ok(Some(bytes)),
        BoundedFileRead::TooLarge => Err(ExternalSourceProviderError::new(
            "pi.hook.file_limit",
            "PI source exceeds the static discovery size limit",
            false,
        )),
    }
}

fn children(path: &Path) -> Result<Vec<PathBuf>, ExternalSourceProviderError> {
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err(error()),
    };
    let mut paths = Vec::new();
    for entry in entries.take(1025) {
        paths.push(entry.map_err(|_| error())?.path());
    }
    if paths.len() > 1024 {
        return Err(ExternalSourceProviderError::new(
            "pi.hook.directory_limit",
            "PI extension directory exceeds the static discovery limit",
            false,
        ));
    }
    paths.sort();
    Ok(paths)
}

fn extension_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("ts" | "js")
    ) && !path.to_string_lossy().ends_with(".d.ts")
}

impl ExternalHookSourceProvider for PiHookProvider {
    fn identity(&self) -> ExternalHookProviderIdentity {
        ExternalHookProviderIdentity::new("pi.hooks", "pi", "PI Hooks").expect("static provider")
    }

    fn discover(
        &self,
        context: &ExternalSourceContext,
    ) -> Result<ExternalHookProviderSnapshot, ExternalSourceProviderError> {
        if context
            .workspace_root
            .as_ref()
            .is_some_and(|path| !path.is_absolute())
        {
            return Err(ExternalSourceProviderError::new(
                "pi.hook.workspace_invalid",
                "Workspace root must be absolute",
                false,
            ));
        }
        let mut catalog = StaticHookCatalog::new(self.identity());
        let mut layers = vec![(
            self.options.agent_dir.clone(),
            ExternalSourceScope::UserGlobal,
        )];
        if let Some(root) = &context.workspace_root {
            layers.push((root.join(".pi"), ExternalSourceScope::Project));
        }
        let mut seen = BTreeSet::new();
        for (root, scope) in layers {
            for path in children(&root.join("extensions"))? {
                discover_entry(&path, scope, &mut catalog, &mut seen)?;
            }
            let settings = root.join("settings.json");
            let Some(bytes) = read(&settings)? else {
                continue;
            };
            let index = catalog.source(
                &settings.to_string_lossy(),
                &settings.to_string_lossy(),
                ExternalHookSourceKind::Settings,
                scope,
            )?;
            let value: Value = match serde_json::from_slice(&bytes) {
                Ok(value) => value,
                Err(_) => {
                    catalog.warning(
                        index,
                        "pi.hook.settings_invalid",
                        "PI settings are not valid JSON",
                    );
                    continue;
                }
            };
            if let Some(entries) = value.get("extensions") {
                if let Some(entries) = entries.as_array() {
                    for entry in entries.iter().take(129) {
                        if seen.len() >= 128 {
                            return Err(ExternalSourceProviderError::new(
                                "pi.hook.source_limit",
                                "PI extension source limit reached",
                                false,
                            ));
                        }
                        let Some(entry) = entry.as_str() else {
                            catalog.warning(
                                index,
                                "pi.hook.entry_invalid",
                                "PI extension path must be a string",
                            );
                            continue;
                        };
                        if entry.contains(['*', '?', '!'])
                            || entry.contains("://")
                            || entry.starts_with("npm:")
                        {
                            catalog.warning(
                                index,
                                "pi.hook.entry_opaque",
                                "Dynamic extension selectors require the PI runtime",
                            );
                            continue;
                        }
                        let path = expand_path(entry, &dirs::home_dir().unwrap_or_default(), &root);
                        if !path.exists() {
                            catalog.warning(
                                index,
                                "pi.hook.entry_missing",
                                "A configured PI extension is missing",
                            );
                        }
                        discover_entry(&path, scope, &mut catalog, &mut seen)?;
                    }
                    if entries.len() > 128 {
                        catalog.warning(
                            index,
                            "pi.hook.entry_limit",
                            "PI extension declaration limit reached",
                        );
                    }
                } else {
                    catalog.warning(
                        index,
                        "pi.hook.entry_invalid",
                        "PI extensions must be an array",
                    );
                }
            }
            if value
                .get("packages")
                .and_then(Value::as_array)
                .is_some_and(|packages| !packages.is_empty())
            {
                catalog.warning(index, "pi.hook.packages_opaque", "Package-selected extensions require PI package resolution; only local extension entries are inspected");
                catalog.event(
                    index,
                    "package-extensions",
                    ExternalHookHandlerKind::Function,
                    ExternalHookMatcherSummary::Unavailable,
                    ExternalHookProjectionStatus::Opaque,
                    ExternalHookNativeActivation::Unknown,
                )?;
            }
        }
        catalog.finish()
    }
}

fn discover_entry(
    path: &Path,
    scope: ExternalSourceScope,
    catalog: &mut StaticHookCatalog,
    seen: &mut BTreeSet<PathBuf>,
) -> Result<(), ExternalSourceProviderError> {
    if seen.len() >= 128 {
        return Err(ExternalSourceProviderError::new(
            "pi.hook.source_limit",
            "PI extension source limit reached",
            false,
        ));
    }
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(error()),
    };
    let canonical = std::fs::canonicalize(path).map_err(|_| error())?;
    if !seen.insert(canonical) {
        return Ok(());
    }
    if metadata.is_dir() {
        let manifest = path.join("package.json");
        if let Some(bytes) = read(&manifest)? {
            let index = catalog.source(
                &manifest.to_string_lossy(),
                &manifest.to_string_lossy(),
                ExternalHookSourceKind::PackageDeclaration,
                scope,
            )?;
            match serde_json::from_slice::<Value>(&bytes) {
                Ok(value) => {
                    if let Some(entries) = value.pointer("/pi/extensions") {
                        if let Some(entries) = entries.as_array() {
                            for entry in entries.iter().take(128) {
                                if let Some(relative) = entry.as_str().filter(|value| {
                                    !value.contains(['*', '?'])
                                        && !Path::new(value).is_absolute()
                                        && !value.split(['/', '\\']).any(|part| part == "..")
                                }) {
                                    let target = path.join(relative);
                                    if !target.exists() {
                                        catalog.warning(
                                            index,
                                            "pi.hook.entry_missing",
                                            "A declared PI package extension is missing",
                                        );
                                    }
                                    discover_entry(&target, scope, catalog, seen)?;
                                } else {
                                    catalog.warning(
                                        index,
                                        "pi.hook.entry_opaque",
                                        "PI package extension entry cannot be resolved statically",
                                    );
                                }
                            }
                            if entries.len() > 128 {
                                catalog.warning(
                                    index,
                                    "pi.hook.entry_limit",
                                    "PI package extension declaration limit reached",
                                );
                            }
                        } else {
                            catalog.warning(
                                index,
                                "pi.hook.entry_invalid",
                                "PI package extensions must be an array",
                            );
                        }
                        return Ok(());
                    }
                }
                Err(_) => {
                    catalog.warning(
                        index,
                        "pi.hook.package_invalid",
                        "PI package manifest is not valid JSON",
                    );
                    return Ok(());
                }
            }
        }
        for name in ["index.ts", "index.js"] {
            let entry = path.join(name);
            if regular_file_exists(&entry).map_err(|_| error())? {
                return discover_entry(&entry, scope, catalog, seen);
            }
        }
        return Ok(());
    }
    if !metadata.is_file() || !extension_file(path) {
        return Ok(());
    }
    let index = catalog.source(
        &path.to_string_lossy(),
        &path.to_string_lossy(),
        ExternalHookSourceKind::PluginFile,
        scope,
    )?;
    let Some(bytes) = read(path)? else {
        return Ok(());
    };
    let discovery = std::str::from_utf8(&bytes)
        .ok()
        .and_then(|source| events(path, source));
    let Some((events, dynamic)) = discovery else {
        catalog.warning(
            index,
            "pi.hook.parse_failed",
            "PI extension could not be parsed without execution",
        );
        catalog.event(
            index,
            "extension",
            ExternalHookHandlerKind::Function,
            ExternalHookMatcherSummary::Unavailable,
            ExternalHookProjectionStatus::Opaque,
            ExternalHookNativeActivation::Unknown,
        )?;
        return Ok(());
    };
    for event in events {
        catalog.event(
            index,
            &event,
            ExternalHookHandlerKind::Function,
            ExternalHookMatcherSummary::Dynamic,
            ExternalHookProjectionStatus::NativeOnly,
            ExternalHookNativeActivation::Unknown,
        )?;
    }
    if dynamic {
        catalog.warning(
            index,
            "pi.hook.dynamic_registration",
            "Dynamic or indirect PI registrations require the native runtime",
        );
        catalog.event(
            index,
            "extension",
            ExternalHookHandlerKind::Function,
            ExternalHookMatcherSummary::Unavailable,
            ExternalHookProjectionStatus::Opaque,
            ExternalHookNativeActivation::Unknown,
        )?;
    }
    Ok(())
}

fn events(path: &Path, source: &str) -> Option<(Vec<String>, bool)> {
    let allocator = Allocator::default();
    let parsed = Parser::new(
        &allocator,
        source,
        SourceType::from_path(path).unwrap_or(SourceType::ts()),
    )
    .parse();
    if !parsed.diagnostics.is_empty() {
        return None;
    }
    let mut discovery = EventVisitor {
        receiver: String::new(),
        events: Vec::new(),
        dynamic: false,
    };
    let mut found = false;
    for statement in &parsed.program.body {
        if let Statement::ExportDefaultDeclaration(export) = statement {
            if let ExportDefaultDeclarationKind::ArrowFunctionExpression(function) =
                &export.declaration
            {
                if let Some(parameter) = function.params.items.first() {
                    if let BindingPattern::BindingIdentifier(identifier) = &parameter.pattern {
                        discovery.receiver = identifier.name.to_string();
                        discovery.visit_function_body(&function.body);
                        found = true;
                    }
                }
            }
            if let ExportDefaultDeclarationKind::FunctionDeclaration(function) = &export.declaration
            {
                if let Some(parameter) = function.params.items.first() {
                    if let BindingPattern::BindingIdentifier(identifier) = &parameter.pattern {
                        discovery.receiver = identifier.name.to_string();
                        if let Some(body) = &function.body {
                            discovery.visit_function_body(body);
                            found = true;
                        }
                    }
                }
            }
        }
    }
    Some((discovery.events, discovery.dynamic || !found))
}

struct EventVisitor {
    receiver: String,
    events: Vec<String>,
    dynamic: bool,
}
impl<'a> Visit<'a> for EventVisitor {
    fn visit_function(
        &mut self,
        function: &Function<'a>,
        flags: oxc_parse::syntax::scope::ScopeFlags,
    ) {
        if function.params.items.iter().any(|parameter| matches!(&parameter.pattern, BindingPattern::BindingIdentifier(identifier) if identifier.name.as_str() == self.receiver)) { return; }
        walk::walk_function(self, function, flags);
    }

    fn visit_arrow_function_expression(&mut self, function: &ArrowFunctionExpression<'a>) {
        if function.params.items.iter().any(|parameter| matches!(&parameter.pattern, BindingPattern::BindingIdentifier(identifier) if identifier.name.as_str() == self.receiver)) { return; }
        walk::walk_arrow_function_expression(self, function);
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if let Expression::StaticMemberExpression(member) = &call.callee {
            if matches!(&member.object, Expression::Identifier(identifier) if identifier.name.as_str() == self.receiver.as_str())
                && member.property.name == "on"
            {
                if let Some(Argument::StringLiteral(event)) = call.arguments.first() {
                    let event = event.value.as_str();
                    if event.len() <= 160
                        && !event.is_empty()
                        && event
                            .chars()
                            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                    {
                        self.events.push(event.into());
                    } else {
                        self.dynamic = true;
                    }
                } else {
                    self.dynamic = true;
                }
            }
        }
        if let Expression::ComputedMemberExpression(member) = &call.callee {
            if matches!(&member.object, Expression::Identifier(identifier) if identifier.name.as_str() == self.receiver)
            {
                self.dynamic = true;
            }
        }
        if call.arguments.iter().any(|argument| matches!(argument, Argument::Identifier(identifier) if identifier.name.as_str() == self.receiver)) { self.dynamic = true; }
        walk::walk_call_expression(self, call);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(path: impl AsRef<Path>, text: &str) {
        std::fs::create_dir_all(path.as_ref().parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    #[test]
    fn parser_recognizes_functions_and_arrows_without_executing_or_scanning_comments() {
        for declaration in ["export default function(api)", "export default (api) =>"] {
            let source = format!(
                r#"{declaration} {{
                // api.on("fake_comment", () => {{}})
                const example = 'api.on("fake_string", fn)';
                api.on("tool_call", () => {{ throw new Error("DO NOT EXECUTE"); }});
                unrelated.on("fake_receiver", () => {{}});
                function shadow(api) {{ api.on("fake_shadow", () => {{}}); }}
                api.on(eventName, handler);
            }}"#
            );
            let (events, dynamic) = events(Path::new("extension.ts"), &source).unwrap();
            assert_eq!(events, ["tool_call"]);
            assert!(dynamic);
        }
    }
    #[test]
    fn discovers_global_project_settings_and_package_entries_and_redacts_code() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let project = temp.path().join("project");
        write(
            home.join("extensions/a.ts"),
            "export default function(pi) { pi.on('session_start', () => 'SECRET'); }",
        );
        write(
            project.join(".pi/extensions/package/package.json"),
            r#"{"pi":{"extensions":["./main.ts"]}}"#,
        );
        write(
            project.join(".pi/extensions/package/main.ts"),
            "export default (pi) => { pi.on('tool_call', () => {}); }",
        );
        write(
            project.join(".pi/settings.json"),
            r#"{"extensions":["./custom.ts"],"packages":["npm:private-package"]}"#,
        );
        write(
            project.join(".pi/custom.ts"),
            "export default function(api) { api.on('tool_result', () => {}); }",
        );
        let provider = PiHookProvider::new(PiHookProviderOptions {
            agent_dir: home.clone(),
        });
        let context = ExternalSourceContext {
            workspace_root: Some(project),
            execution_domain_id: ExecutionDomainId::new("fixture").unwrap(),
        };
        let snapshot = provider.discover(&context).unwrap();
        let native: BTreeSet<_> = snapshot
            .entries
            .iter()
            .map(|entry| entry.native_event.as_str())
            .collect();
        assert_eq!(
            native,
            BTreeSet::from([
                "session_start",
                "tool_call",
                "tool_result",
                "package-extensions"
            ])
        );
        assert!(snapshot.entries.iter().all(|entry| entry.mapping.is_none()
            && entry.native_activation == ExternalHookNativeActivation::Unknown));
        let encoded = serde_json::to_string(&snapshot).unwrap();
        assert!(!encoded.contains("SECRET"));
        assert!(!encoded.contains("private-package"));
        write(
            home.join("extensions/a.ts"),
            "export default function(pi) { pi.on('session_start', () => 'CHANGED_SECRET'); }",
        );
        assert_eq!(snapshot, provider.discover(&context).unwrap());
        assert!(provider
            .prepare_import(
                &context,
                &snapshot.sources[0].key,
                &snapshot.sources[0].content_version
            )
            .is_err());
    }
    #[test]
    fn invalid_and_indirect_extensions_are_visible_as_opaque() {
        let temp = tempfile::tempdir().unwrap();
        write(
            temp.path().join("extensions/broken.ts"),
            "export default function(",
        );
        write(
            temp.path().join("extensions/indirect.ts"),
            "export { default } from './other';",
        );
        let snapshot = PiHookProvider::new(PiHookProviderOptions {
            agent_dir: temp.path().into(),
        })
        .discover(&ExternalSourceContext {
            workspace_root: None,
            execution_domain_id: ExecutionDomainId::new("fixture").unwrap(),
        })
        .unwrap();
        assert_eq!(snapshot.entries.len(), 2);
        assert!(snapshot
            .entries
            .iter()
            .all(|entry| entry.projection_status == ExternalHookProjectionStatus::Opaque));
        assert!(snapshot
            .sources
            .iter()
            .all(|source| source.health == ExternalSourceHealth::Partial));
    }
}
