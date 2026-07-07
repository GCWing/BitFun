use bitfun_product_domains::canvas::types::{
    CanvasDiagnostic, CanvasDiagnosticCategory, CanvasDiagnosticSeverity,
};

#[cfg(feature = "canvas-runtime")]
use oxc::allocator::Allocator;
#[cfg(feature = "canvas-runtime")]
use oxc::ast::ast::{
    BindingIdentifier, BindingPattern, Class, ExportDefaultDeclarationKind, Function,
    ImportDeclaration, ImportDeclarationSpecifier, ImportOrExportKind, JSXMemberExpression,
    ModuleExportName, Statement,
};
#[cfg(feature = "canvas-runtime")]
use oxc::parser::Parser;
#[cfg(feature = "canvas-runtime")]
use oxc::span::{GetSpan, SourceType};
#[cfg(feature = "canvas-runtime")]
use std::collections::{BTreeMap, BTreeSet};
#[cfg(feature = "canvas-runtime")]
use std::path::Path;

#[cfg(feature = "canvas-runtime")]
use super::diagnostics::oxc_diagnostics_to_canvas;
use super::{compile_error, line_column};

#[cfg(feature = "canvas-runtime")]
pub(super) fn validate_canvas_import_shadowing(
    source: &str,
    analysis: &CanvasModuleAnalysis,
) -> Vec<CanvasDiagnostic> {
    analysis
        .import_bindings
        .local_names()
        .into_iter()
        .filter_map(|name| {
            let declaration_offset = analysis.local_binding_offsets.get(&name).copied()?;
            let (line, column) = line_column(source, declaration_offset);
            Some(CanvasDiagnostic {
                severity: CanvasDiagnosticSeverity::Error,
                category: CanvasDiagnosticCategory::TypeScript,
                message: format!(
                    "`{}` is imported from bitfun/canvas and also declared locally",
                    name
                ),
                code: Some("canvas.compile.sdk_name_shadowed".to_string()),
                line: Some(line),
                column: Some(column),
                suggested_fix: Some(format!(
                    "Remove the local `{}` declaration or rename it; use the imported Canvas SDK component directly.",
                    name
                )),
            })
        })
        .collect()
}

#[cfg(feature = "canvas-runtime")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CanvasModuleAnalysis {
    pub(super) import_bindings: CanvasSdkImportBindings,
    import_removal_spans: Vec<(usize, usize)>,
    default_export: Option<CanvasDefaultExport>,
    pub(super) local_binding_offsets: BTreeMap<String, usize>,
}

#[cfg(feature = "canvas-runtime")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CanvasDefaultExport {
    Declaration {
        start: usize,
        end: usize,
        expression_start: usize,
    },
    Identifier {
        start: usize,
        end: usize,
        name: String,
    },
}

#[cfg(feature = "canvas-runtime")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CanvasSdkImportSource {
    Canvas,
    React,
}

#[cfg(feature = "canvas-runtime")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CanvasSdkNamedImport {
    pub(super) local: String,
    canonical: String,
    target_expression: String,
}

#[cfg(feature = "canvas-runtime")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CanvasSdkNamespaceImport {
    pub(super) local: String,
    pub(super) source: CanvasSdkImportSource,
}

#[cfg(feature = "canvas-runtime")]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct CanvasSdkImportBindings {
    named: Vec<CanvasSdkNamedImport>,
    pub(super) namespaces: Vec<CanvasSdkNamespaceImport>,
}

#[cfg(feature = "canvas-runtime")]
impl CanvasSdkImportBindings {
    pub(super) fn local_names(&self) -> Vec<String> {
        let mut names = self
            .named
            .iter()
            .map(|binding| binding.local.clone())
            .chain(
                self.namespaces
                    .iter()
                    .map(|namespace| namespace.local.clone()),
            )
            .collect::<Vec<_>>();
        names.sort();
        names.dedup();
        names
    }

    pub(super) fn canonical_component_for_local(&self, local: &str) -> Option<&str> {
        self.named
            .iter()
            .find(|binding| binding.local == local)
            .map(|binding| binding.canonical.as_str())
    }

    pub(super) fn canonical_for_local(&self, local: &str) -> Option<&str> {
        self.named
            .iter()
            .find(|binding| binding.local == local)
            .map(|binding| binding.canonical.as_str())
    }

    pub(super) fn canonical_component_for_member(
        &self,
        member: &JSXMemberExpression<'_>,
    ) -> Option<String> {
        let root = member.get_identifier()?.name.as_str();
        let namespace = self
            .namespaces
            .iter()
            .find(|namespace| namespace.local == root)?;
        if namespace.source != CanvasSdkImportSource::Canvas {
            return None;
        }
        Some(member.property.name.to_string())
    }

    fn insert_named(&mut self, local: String, canonical: String, target_expression: String) {
        if let Some(existing) = self.named.iter_mut().find(|binding| binding.local == local) {
            existing.canonical = canonical;
            existing.target_expression = target_expression;
            return;
        }
        self.named.push(CanvasSdkNamedImport {
            local,
            canonical,
            target_expression,
        });
    }

    fn insert_namespace(&mut self, local: String, source: CanvasSdkImportSource) {
        if let Some(existing) = self
            .namespaces
            .iter_mut()
            .find(|namespace| namespace.local == local)
        {
            existing.source = source;
            return;
        }
        self.namespaces
            .push(CanvasSdkNamespaceImport { local, source });
    }
}

#[cfg(feature = "canvas-runtime")]
pub(super) fn analyze_canvas_module(
    source: &str,
) -> Result<CanvasModuleAnalysis, Vec<CanvasDiagnostic>> {
    let path = Path::new("Canvas.tsx");
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path).unwrap_or(SourceType::tsx());
    let parse_return = Parser::new(&allocator, source, source_type).parse();
    if !parse_return.diagnostics.is_empty() {
        return Err(oxc_diagnostics_to_canvas(
            source,
            parse_return.diagnostics.into_iter(),
            "canvas.compile.oxc.parse",
        ));
    }

    let mut bindings = CanvasSdkImportBindings::default();
    let mut import_removal_spans = BTreeSet::new();
    let mut default_export = None;
    let mut local_binding_offsets = BTreeMap::new();
    let mut diagnostics = Vec::new();
    for statement in &parse_return.program.body {
        match statement {
            Statement::ImportDeclaration(declaration) => {
                collect_canvas_import_bindings(
                    source,
                    declaration,
                    &mut bindings,
                    &mut import_removal_spans,
                    &mut diagnostics,
                );
            }
            Statement::ExportDefaultDeclaration(declaration) => {
                default_export = Some(match &declaration.declaration {
                    ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
                        CanvasDefaultExport::Declaration {
                            start: declaration.span.start as usize,
                            end: declaration.span.end as usize,
                            expression_start: function.span.start as usize,
                        }
                    }
                    ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                        CanvasDefaultExport::Declaration {
                            start: declaration.span.start as usize,
                            end: declaration.span.end as usize,
                            expression_start: class.span.start as usize,
                        }
                    }
                    ExportDefaultDeclarationKind::Identifier(identifier) => {
                        CanvasDefaultExport::Identifier {
                            start: declaration.span.start as usize,
                            end: declaration.span.end as usize,
                            name: identifier.name.to_string(),
                        }
                    }
                    expression => CanvasDefaultExport::Declaration {
                        start: declaration.span.start as usize,
                        end: declaration.span.end as usize,
                        expression_start: expression.span().start as usize,
                    },
                });
            }
            Statement::FunctionDeclaration(function) => {
                collect_function_binding(function, &mut local_binding_offsets);
            }
            Statement::ClassDeclaration(class) => {
                collect_class_binding(class, &mut local_binding_offsets);
            }
            Statement::VariableDeclaration(declaration) => {
                for declarator in &declaration.declarations {
                    collect_binding_pattern_offsets(&declarator.id, &mut local_binding_offsets);
                }
            }
            _ => {
                collect_default_exportable_statement_binding(statement, &mut local_binding_offsets);
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(CanvasModuleAnalysis {
            import_bindings: bindings,
            import_removal_spans: import_removal_spans.into_iter().collect(),
            default_export,
            local_binding_offsets,
        })
    } else {
        Err(diagnostics)
    }
}

#[cfg(feature = "canvas-runtime")]
fn collect_canvas_import_bindings(
    source: &str,
    declaration: &ImportDeclaration<'_>,
    bindings: &mut CanvasSdkImportBindings,
    import_removal_spans: &mut BTreeSet<(usize, usize)>,
    diagnostics: &mut Vec<CanvasDiagnostic>,
) {
    let Some(source_kind) = canvas_sdk_import_source(declaration.source.value.as_str()) else {
        return;
    };
    import_removal_spans.insert((
        declaration.span.start as usize,
        declaration.span.end as usize,
    ));

    let Some(specifiers) = declaration.specifiers.as_ref() else {
        return;
    };
    for specifier in specifiers {
        match specifier {
            ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                if declaration.import_kind == ImportOrExportKind::Type
                    || specifier.import_kind == ImportOrExportKind::Type
                {
                    continue;
                }
                let imported = module_export_name(&specifier.imported);
                let local = specifier.local.name.to_string();
                match named_import_target(source_kind, imported.as_str()) {
                    Some(target) => {
                        bindings.insert_named(local, target.canonical, target.expression);
                    }
                    None => diagnostics.push(unsupported_sdk_import_diagnostic(
                        source,
                        specifier.span.start as usize,
                        imported.as_str(),
                        source_kind,
                    )),
                }
            }
            ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                let local = specifier.local.name.to_string();
                if is_reserved_canvas_runtime_binding(local.as_str()) {
                    diagnostics.push(reserved_sdk_import_binding_diagnostic(
                        source,
                        specifier.span.start as usize,
                        local.as_str(),
                    ));
                } else {
                    bindings.insert_namespace(local, source_kind);
                }
            }
            ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                let local = specifier.local.name.to_string();
                if source_kind == CanvasSdkImportSource::React {
                    if is_reserved_canvas_runtime_binding(local.as_str()) {
                        diagnostics.push(reserved_sdk_import_binding_diagnostic(
                            source,
                            specifier.span.start as usize,
                            local.as_str(),
                        ));
                    } else {
                        bindings.insert_namespace(local, source_kind);
                    }
                } else {
                    diagnostics.push(unsupported_sdk_import_diagnostic(
                        source,
                        specifier.span.start as usize,
                        "default",
                        source_kind,
                    ));
                }
            }
        }
    }
}

#[cfg(feature = "canvas-runtime")]
fn collect_default_exportable_statement_binding(
    statement: &Statement<'_>,
    local_binding_offsets: &mut BTreeMap<String, usize>,
) {
    match statement {
        Statement::TSTypeAliasDeclaration(declaration) => {
            local_binding_offsets
                .entry(declaration.id.name.to_string())
                .or_insert(declaration.id.span.start as usize);
        }
        Statement::TSInterfaceDeclaration(declaration) => {
            local_binding_offsets
                .entry(declaration.id.name.to_string())
                .or_insert(declaration.id.span.start as usize);
        }
        Statement::TSEnumDeclaration(declaration) => {
            local_binding_offsets
                .entry(declaration.id.name.to_string())
                .or_insert(declaration.id.span.start as usize);
        }
        _ => {}
    }
}

#[cfg(feature = "canvas-runtime")]
fn collect_function_binding(
    function: &Function<'_>,
    local_binding_offsets: &mut BTreeMap<String, usize>,
) {
    if let Some(id) = function.id.as_ref() {
        collect_binding_identifier(id, local_binding_offsets);
    }
}

#[cfg(feature = "canvas-runtime")]
fn collect_class_binding(class: &Class<'_>, local_binding_offsets: &mut BTreeMap<String, usize>) {
    if let Some(id) = class.id.as_ref() {
        collect_binding_identifier(id, local_binding_offsets);
    }
}

#[cfg(feature = "canvas-runtime")]
fn collect_binding_pattern_offsets(
    pattern: &BindingPattern<'_>,
    local_binding_offsets: &mut BTreeMap<String, usize>,
) {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => {
            collect_binding_identifier(identifier, local_binding_offsets);
        }
        BindingPattern::ObjectPattern(pattern) => {
            for property in &pattern.properties {
                collect_binding_pattern_offsets(&property.value, local_binding_offsets);
            }
            if let Some(rest) = pattern.rest.as_ref() {
                collect_binding_pattern_offsets(&rest.argument, local_binding_offsets);
            }
        }
        BindingPattern::ArrayPattern(pattern) => {
            for element in pattern.elements.iter().flatten() {
                collect_binding_pattern_offsets(element, local_binding_offsets);
            }
            if let Some(rest) = pattern.rest.as_ref() {
                collect_binding_pattern_offsets(&rest.argument, local_binding_offsets);
            }
        }
        BindingPattern::AssignmentPattern(pattern) => {
            collect_binding_pattern_offsets(&pattern.left, local_binding_offsets);
        }
    }
}

#[cfg(feature = "canvas-runtime")]
fn collect_binding_identifier(
    identifier: &BindingIdentifier<'_>,
    local_binding_offsets: &mut BTreeMap<String, usize>,
) {
    local_binding_offsets
        .entry(identifier.name.to_string())
        .or_insert(identifier.span.start as usize);
}

#[cfg(feature = "canvas-runtime")]
pub(super) fn canvas_runtime_binding_prelude(import_bindings: &CanvasSdkImportBindings) -> String {
    let mut local_bindings = sdk_runtime_exports()
        .iter()
        .map(|name| {
            (
                (*name).to_string(),
                format!("__BitfunCanvasSDK.{}", property_access(name)),
            )
        })
        .collect::<BTreeMap<_, _>>();

    for binding in &import_bindings.named {
        if is_reserved_canvas_runtime_binding(binding.local.as_str()) {
            continue;
        }
        local_bindings.insert(binding.local.clone(), binding.target_expression.clone());
    }
    for namespace in &import_bindings.namespaces {
        local_bindings.insert(
            namespace.local.clone(),
            match namespace.source {
                CanvasSdkImportSource::Canvas => "__BitfunCanvasSDK".to_string(),
                CanvasSdkImportSource::React => "__BitfunCanvasReactCompat".to_string(),
            },
        );
    }

    let mut prelude = String::from(
        "const __BitfunCanvasSDK = window.BitfunCanvasSDK;\n\
const __BitfunCanvasRuntime = window.BitfunCanvasRuntime;\n\
const __BitfunCanvasReactCompat = Object.freeze({ ...__BitfunCanvasSDK, createElement: __BitfunCanvasRuntime.h, Fragment: __BitfunCanvasRuntime.Fragment });\n\
const h = __BitfunCanvasRuntime.h;\n\
const Fragment = __BitfunCanvasRuntime.Fragment;\n",
    );
    for (local, expression) in local_bindings {
        if local == "h" || local == "Fragment" {
            continue;
        }
        prelude.push_str("const ");
        prelude.push_str(&local);
        prelude.push_str(" = ");
        prelude.push_str(&expression);
        prelude.push_str(";\n");
    }
    prelude
}

#[cfg(feature = "canvas-runtime")]
fn canvas_sdk_import_source(source: &str) -> Option<CanvasSdkImportSource> {
    match source {
        "bitfun/canvas" | "cursor/canvas" => Some(CanvasSdkImportSource::Canvas),
        "react" => Some(CanvasSdkImportSource::React),
        _ => None,
    }
}

#[cfg(feature = "canvas-runtime")]
struct NamedImportTarget {
    canonical: String,
    expression: String,
}

#[cfg(feature = "canvas-runtime")]
fn named_import_target(source: CanvasSdkImportSource, imported: &str) -> Option<NamedImportTarget> {
    match source {
        CanvasSdkImportSource::Canvas => {
            sdk_runtime_exports()
                .contains(&imported)
                .then(|| NamedImportTarget {
                    canonical: imported.to_string(),
                    expression: format!("__BitfunCanvasSDK.{}", property_access(imported)),
                })
        }
        CanvasSdkImportSource::React => match imported {
            "useState" | "useRef" | "useEffect" | "useCallback" | "useMemo" => {
                Some(NamedImportTarget {
                    canonical: imported.to_string(),
                    expression: format!("__BitfunCanvasSDK.{}", property_access(imported)),
                })
            }
            "Fragment" => Some(NamedImportTarget {
                canonical: "Fragment".to_string(),
                expression: "__BitfunCanvasRuntime.Fragment".to_string(),
            }),
            "createElement" => Some(NamedImportTarget {
                canonical: "createElement".to_string(),
                expression: "__BitfunCanvasRuntime.h".to_string(),
            }),
            _ => None,
        },
    }
}

#[cfg(feature = "canvas-runtime")]
fn module_export_name(name: &ModuleExportName<'_>) -> String {
    match name {
        ModuleExportName::IdentifierName(identifier) => identifier.name.to_string(),
        ModuleExportName::IdentifierReference(identifier) => identifier.name.to_string(),
        ModuleExportName::StringLiteral(literal) => literal.value.to_string(),
    }
}

#[cfg(feature = "canvas-runtime")]
fn unsupported_sdk_import_diagnostic(
    source: &str,
    offset: usize,
    imported: &str,
    import_source: CanvasSdkImportSource,
) -> CanvasDiagnostic {
    let (line, column) = line_column(source, offset);
    let module = match import_source {
        CanvasSdkImportSource::Canvas => "bitfun/canvas",
        CanvasSdkImportSource::React => "react",
    };
    CanvasDiagnostic {
        severity: CanvasDiagnosticSeverity::Error,
        category: CanvasDiagnosticCategory::TypeScript,
        message: format!("`{}` is not exported by {}", imported, module),
        code: Some("canvas.compile.sdk_import_unsupported".to_string()),
        line: Some(line),
        column: Some(column),
        suggested_fix: Some(
            "Import a supported Canvas SDK or React compatibility export.".to_string(),
        ),
    }
}

#[cfg(feature = "canvas-runtime")]
fn reserved_sdk_import_binding_diagnostic(
    source: &str,
    offset: usize,
    local: &str,
) -> CanvasDiagnostic {
    let (line, column) = line_column(source, offset);
    CanvasDiagnostic {
        severity: CanvasDiagnosticSeverity::Error,
        category: CanvasDiagnosticCategory::TypeScript,
        message: format!("`{}` is reserved by the Canvas runtime", local),
        code: Some("canvas.compile.sdk_import_reserved".to_string()),
        line: Some(line),
        column: Some(column),
        suggested_fix: Some("Use a different local import alias.".to_string()),
    }
}

#[cfg(feature = "canvas-runtime")]
fn is_reserved_canvas_runtime_binding(name: &str) -> bool {
    matches!(
        name,
        "h" | "Fragment"
            | "__BitfunCanvasSDK"
            | "__BitfunCanvasRuntime"
            | "__BitfunCanvasReactCompat"
    )
}

#[cfg(feature = "canvas-runtime")]
pub(super) fn property_access(name: &str) -> String {
    if is_identifier(name) {
        name.to_string()
    } else {
        format!("{name:?}")
    }
}

#[cfg(feature = "canvas-runtime")]
fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first == '$' || first.is_ascii_alphabetic()) {
        return false;
    }
    chars.all(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
}

#[cfg(feature = "canvas-runtime")]
pub(super) fn sdk_runtime_exports() -> &'static [&'static str] {
    &[
        "Stack",
        "Row",
        "Grid",
        "Box",
        "Divider",
        "Spacer",
        "H1",
        "H2",
        "H3",
        "Text",
        "Code",
        "Link",
        "Card",
        "CardHeader",
        "CardBody",
        "Alert",
        "Callout",
        "CollapsibleSection",
        "Empty",
        "Tabs",
        "Pill",
        "Stat",
        "Table",
        "KeyValueList",
        "Timeline",
        "FileTree",
        "ProgressBar",
        "Swatch",
        "UsageBar",
        "TodoList",
        "TodoListCard",
        "DependencyGraph",
        "FlowDiagram",
        "BarChart",
        "LineChart",
        "PieChart",
        "Button",
        "Toggle",
        "Checkbox",
        "Select",
        "Input",
        "TextInput",
        "TextArea",
        "IconButton",
        "DiffStats",
        "DiffView",
        "computeDAGLayout",
        "mergeStyle",
        "colorPalette",
        "usageColorSequence",
        "categoryPaletteLight",
        "categoryPaletteDark",
        "canvasPaletteLight",
        "canvasPaletteDark",
        "canvasTokensLight",
        "canvasTokens",
        "useHostTheme",
        "useCanvasState",
        "useCanvasAction",
        "useState",
        "useRef",
        "useEffect",
        "useCallback",
        "useMemo",
    ]
}

#[cfg(feature = "canvas-runtime")]
pub(super) fn rewrite_canvas_module_for_runtime(
    source: &str,
    analysis: &CanvasModuleAnalysis,
) -> Result<String, Vec<CanvasDiagnostic>> {
    let Some(default_export) = analysis.default_export.as_ref() else {
        return Err(vec![compile_error(
            "Canvas source must default-export a component",
            "canvas.compile.default_function_required",
        )]);
    };

    if let CanvasDefaultExport::Identifier { start, name, .. } = default_export {
        if !analysis.local_binding_offsets.contains_key(name) {
            let (line, column) = line_column(source, *start);
            let mut diagnostic = compile_error(
                "Canvas default export must reference a component declared in the same source file",
                "canvas.compile.default_function_required",
            );
            diagnostic.line = Some(line);
            diagnostic.column = Some(column);
            return Err(vec![diagnostic]);
        }
    }

    let mut replacements = analysis
        .import_removal_spans
        .iter()
        .map(|(start, end)| (*start, *end, String::new()))
        .collect::<Vec<_>>();
    replacements.push(default_export_replacement(source, default_export));
    replacements.sort_by_key(|(start, _, _)| *start);

    let mut rewritten = String::with_capacity(source.len() + 128);
    let mut cursor = 0usize;
    for (start, end, replacement) in replacements {
        if start < cursor {
            continue;
        }
        rewritten.push_str(&source[cursor..start]);
        rewritten.push_str(&replacement);
        cursor = end;
    }
    rewritten.push_str(&source[cursor..]);
    rewritten.push_str("\nwindow.BitfunCanvasRuntime.mount(__BitfunCanvasComponent);\n");
    Ok(rewritten)
}

#[cfg(feature = "canvas-runtime")]
fn default_export_replacement(
    source: &str,
    default_export: &CanvasDefaultExport,
) -> (usize, usize, String) {
    match default_export {
        CanvasDefaultExport::Declaration {
            start,
            end,
            expression_start,
        } => (
            *start,
            *end,
            format!(
                "const __BitfunCanvasComponent = {};",
                source[*expression_start..*end].trim()
            ),
        ),
        CanvasDefaultExport::Identifier { start, end, name } => (
            *start,
            *end,
            format!("const __BitfunCanvasComponent = {name};"),
        ),
    }
}
