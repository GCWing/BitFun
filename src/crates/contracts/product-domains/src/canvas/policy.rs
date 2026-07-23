use crate::canvas::types::{
    CanvasDiagnostic, CanvasDiagnosticCategory, CanvasDiagnosticSeverity, CanvasSource,
    CANVAS_SOURCE_LANGUAGE_TSX,
};
use serde::{Deserialize, Serialize};

pub const BITFUN_CANVAS_IMPORT: &str = "bitfun/canvas";
const CURSOR_CANVAS_IMPORT: &str = "cursor/canvas";
const REACT_IMPORT: &str = "react";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasImportPolicyDiagnosticKind {
    RelativeImport,
    DynamicImport,
    UnsupportedImport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasImportPolicyDiagnostic {
    pub kind: CanvasImportPolicyDiagnosticKind,
    pub specifier: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

pub fn validate_canvas_imports(source: &str) -> Vec<CanvasImportPolicyDiagnostic> {
    let mut diagnostics = Vec::new();
    for specifier in module_import_export_specifiers(source) {
        if is_allowed_canvas_import(&specifier.value) {
            continue;
        }
        let kind = if specifier.value.starts_with('.') || specifier.value.starts_with('/') {
            CanvasImportPolicyDiagnosticKind::RelativeImport
        } else {
            CanvasImportPolicyDiagnosticKind::UnsupportedImport
        };
        diagnostics.push(CanvasImportPolicyDiagnostic {
            kind,
            specifier: specifier.value,
            line: Some(specifier.line),
            column: Some(specifier.column),
        });
    }

    for specifier in dynamic_import_specifiers(source) {
        diagnostics.push(CanvasImportPolicyDiagnostic {
            kind: CanvasImportPolicyDiagnosticKind::DynamicImport,
            specifier: specifier.value,
            line: Some(specifier.line),
            column: Some(specifier.column),
        });
    }

    diagnostics
}

fn is_allowed_canvas_import(specifier: &str) -> bool {
    matches!(
        specifier,
        BITFUN_CANVAS_IMPORT | CURSOR_CANVAS_IMPORT | REACT_IMPORT
    )
}

pub fn validate_canvas_source_policy(source: &CanvasSource) -> Vec<CanvasDiagnostic> {
    let mut diagnostics = Vec::new();
    if source.language != CANVAS_SOURCE_LANGUAGE_TSX {
        diagnostics.push(CanvasDiagnostic {
            severity: CanvasDiagnosticSeverity::Error,
            category: CanvasDiagnosticCategory::Unsupported,
            message: format!(
                "Canvas source language '{}' is not supported",
                source.language
            ),
            code: Some("canvas.source.language_unsupported".to_string()),
            line: None,
            column: None,
            suggested_fix: Some("Use a single TSX source file.".to_string()),
        });
    }

    if !source.filename.ends_with(".tsx") {
        diagnostics.push(CanvasDiagnostic {
            severity: CanvasDiagnosticSeverity::Error,
            category: CanvasDiagnosticCategory::Unsupported,
            message: format!("Canvas filename '{}' must end with .tsx", source.filename),
            code: Some("canvas.source.filename_unsupported".to_string()),
            line: None,
            column: None,
            suggested_fix: Some("Use a .tsx filename.".to_string()),
        });
    }

    if !has_default_export(&source.source) {
        diagnostics.push(CanvasDiagnostic {
            severity: CanvasDiagnosticSeverity::Error,
            category: CanvasDiagnosticCategory::TypeScript,
            message: "Canvas source must default-export a React component".to_string(),
            code: Some("canvas.source.default_export_missing".to_string()),
            line: None,
            column: None,
            suggested_fix: Some(
                "Add `export default function ...` or `export default ...`.".to_string(),
            ),
        });
    }

    for import_diagnostic in validate_canvas_imports(&source.source) {
        diagnostics.push(CanvasDiagnostic {
            severity: CanvasDiagnosticSeverity::Error,
            category: CanvasDiagnosticCategory::ImportPolicy,
            message: import_policy_message(&import_diagnostic),
            code: Some(import_policy_code(&import_diagnostic.kind).to_string()),
            line: import_diagnostic.line,
            column: import_diagnostic.column,
            suggested_fix: Some("Import Canvas primitives from bitfun/canvas only.".to_string()),
        });
    }

    diagnostics
}

fn has_default_export(source: &str) -> bool {
    source.contains("export default")
}

fn import_policy_code(kind: &CanvasImportPolicyDiagnosticKind) -> &'static str {
    match kind {
        CanvasImportPolicyDiagnosticKind::RelativeImport => "canvas.import.relative",
        CanvasImportPolicyDiagnosticKind::DynamicImport => "canvas.import.dynamic",
        CanvasImportPolicyDiagnosticKind::UnsupportedImport => "canvas.import.unsupported",
    }
}

fn import_policy_message(diagnostic: &CanvasImportPolicyDiagnostic) -> String {
    match diagnostic.kind {
        CanvasImportPolicyDiagnosticKind::RelativeImport => {
            format!(
                "Relative import '{}' is not allowed in Canvas source",
                diagnostic.specifier
            )
        }
        CanvasImportPolicyDiagnosticKind::DynamicImport => {
            format!(
                "Dynamic import '{}' is not allowed in Canvas source",
                diagnostic.specifier
            )
        }
        CanvasImportPolicyDiagnosticKind::UnsupportedImport => {
            format!(
                "Import '{}' is not allowed; Canvas source may only import from {}",
                diagnostic.specifier, BITFUN_CANVAS_IMPORT
            )
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LocatedSpecifier {
    value: String,
    line: u32,
    column: u32,
}

fn module_import_export_specifiers(source: &str) -> Vec<LocatedSpecifier> {
    module_import_export_specifiers_by_scan(source)
}

fn module_import_export_specifiers_by_scan(source: &str) -> Vec<LocatedSpecifier> {
    source
        .lines()
        .enumerate()
        .filter_map(|(line_index, line)| {
            let trimmed = line.trim_start();
            if !(trimmed.starts_with("import ")
                || (trimmed.starts_with("export ") && trimmed.contains(" from ")))
            {
                return None;
            }
            let leading_columns = line.len() - trimmed.len();
            let (quote_index, value) = quoted_specifier(trimmed)?;
            Some(LocatedSpecifier {
                value,
                line: line_index as u32 + 1,
                column: leading_columns as u32 + quote_index as u32 + 2,
            })
        })
        .collect()
}

fn dynamic_import_specifiers(source: &str) -> Vec<LocatedSpecifier> {
    dynamic_import_specifiers_by_scan(source)
}

fn dynamic_import_specifiers_by_scan(source: &str) -> Vec<LocatedSpecifier> {
    let mut specifiers = Vec::new();
    let mut consumed = 0usize;
    let mut rest = source;
    while let Some(index) = rest.find("import(") {
        let import_offset = consumed + index;
        consumed += index + "import(".len();
        rest = &rest[index + "import(".len()..];
        let trimmed_len = rest.len() - rest.trim_start().len();
        let trimmed = rest.trim_start();
        if let Some((quote_index, value)) = quoted_specifier(trimmed) {
            let (line, column) = line_column(source, consumed + trimmed_len + quote_index + 1);
            specifiers.push(LocatedSpecifier {
                value,
                line,
                column,
            });
        } else {
            let (line, column) = line_column(source, import_offset);
            specifiers.push(LocatedSpecifier {
                value: "<dynamic>".to_string(),
                line,
                column,
            });
        }
    }
    specifiers
}

fn quoted_specifier(text: &str) -> Option<(usize, String)> {
    let single = quoted_specifier_with(text, '\'');
    let double = quoted_specifier_with(text, '"');
    match (single, double) {
        (Some((single_index, single_value)), Some((double_index, double_value))) => {
            Some(if single_index < double_index {
                (single_index, single_value)
            } else {
                (double_index, double_value)
            })
        }
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn quoted_specifier_with(text: &str, quote: char) -> Option<(usize, String)> {
    let start = text.find(quote)?;
    let after = &text[start + quote.len_utf8()..];
    let end = after.find(quote)?;
    Some((start, after[..end].to_string()))
}

fn line_column(source: &str, offset: usize) -> (u32, u32) {
    let mut line = 1u32;
    let mut column = 1u32;
    for (index, ch) in source.char_indices() {
        if index >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}
