use bitfun_product_domains::canvas::types::{
    CanvasDiagnostic, CanvasDiagnosticCategory, CanvasDiagnosticSeverity,
};

use oxc::diagnostics::{OxcDiagnostic, Severity};

use super::line_column;

pub(super) fn oxc_diagnostics_to_canvas(
    source: &str,
    diagnostics: impl IntoIterator<Item = OxcDiagnostic>,
    fallback_code: &str,
) -> Vec<CanvasDiagnostic> {
    diagnostics
        .into_iter()
        .map(|diagnostic| {
            let offset = diagnostic
                .labels
                .first()
                .map(|label| label.inner().offset());
            let (line, column) = offset
                .map(|offset| line_column(source, offset as usize))
                .map_or((None, None), |(line, column)| (Some(line), Some(column)));
            CanvasDiagnostic {
                severity: match diagnostic.severity {
                    Severity::Warning | Severity::Advice => CanvasDiagnosticSeverity::Warning,
                    _ => CanvasDiagnosticSeverity::Error,
                },
                category: CanvasDiagnosticCategory::Compile,
                message: diagnostic.to_string(),
                code: diagnostic
                    .code
                    .is_some()
                    .then(|| format!("canvas.compile.oxc.{}", diagnostic.code))
                    .or_else(|| Some(fallback_code.to_string())),
                line,
                column,
                suggested_fix: diagnostic
                    .help
                    .as_ref()
                    .map(|help| help.to_string())
                    .or_else(|| Some("Fix the Canvas TSX syntax and retry.".to_string())),
            }
        })
        .collect()
}
