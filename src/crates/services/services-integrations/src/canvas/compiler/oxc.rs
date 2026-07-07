use bitfun_product_domains::canvas::types::CanvasDiagnostic;

#[cfg(feature = "canvas-runtime")]
use oxc::allocator::Allocator;
#[cfg(feature = "canvas-runtime")]
use oxc::codegen::{Codegen, CodegenOptions, CodegenReturn};
#[cfg(feature = "canvas-runtime")]
use oxc::parser::Parser;
#[cfg(feature = "canvas-runtime")]
use oxc::semantic::SemanticBuilder;
#[cfg(feature = "canvas-runtime")]
use oxc::span::SourceType;
#[cfg(feature = "canvas-runtime")]
use oxc::transformer::{HelperLoaderMode, JsxOptions, JsxRuntime, TransformOptions, Transformer};
#[cfg(feature = "canvas-runtime")]
use std::path::Path;

#[cfg(feature = "canvas-runtime")]
use super::analysis::{
    analyze_canvas_module, canvas_runtime_binding_prelude, rewrite_canvas_module_for_runtime,
    validate_canvas_import_shadowing,
};
#[cfg(not(feature = "canvas-runtime"))]
use super::compile_error;
#[cfg(feature = "canvas-runtime")]
use super::diagnostics::oxc_diagnostics_to_canvas;
#[cfg(feature = "canvas-runtime")]
use super::sdk_contract::validate_canvas_sdk_contracts;

#[cfg(feature = "canvas-runtime")]
pub(super) fn compile_canvas_component_js_with_oxc(
    source: &str,
) -> Result<String, Vec<CanvasDiagnostic>> {
    let analysis = analyze_canvas_module(source)?;
    let shadow_diagnostics = validate_canvas_import_shadowing(source, &analysis);
    if !shadow_diagnostics.is_empty() {
        return Err(shadow_diagnostics);
    }
    let module = rewrite_canvas_module_for_runtime(source, &analysis)?;
    let path = Path::new("Canvas.tsx");
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path).unwrap_or(SourceType::tsx());
    let parse_return = Parser::new(&allocator, &module, source_type).parse();
    if !parse_return.diagnostics.is_empty() {
        return Err(oxc_diagnostics_to_canvas(
            &module,
            parse_return.diagnostics.into_iter(),
            "canvas.compile.oxc.parse",
        ));
    }

    let mut program = parse_return.program;
    let sdk_contract_diagnostics =
        validate_canvas_sdk_contracts(&module, &program, &analysis.import_bindings);
    if !sdk_contract_diagnostics.is_empty() {
        return Err(sdk_contract_diagnostics);
    }

    let semantic_return = SemanticBuilder::new()
        .with_excess_capacity(2.0)
        .with_enum_eval(true)
        .build(&program);
    if !semantic_return.diagnostics.is_empty() {
        return Err(oxc_diagnostics_to_canvas(
            &module,
            semantic_return.diagnostics.into_iter(),
            "canvas.compile.oxc.semantic",
        ));
    }

    let mut options = TransformOptions {
        jsx: JsxOptions {
            runtime: JsxRuntime::Classic,
            pragma: Some("h".to_string()),
            pragma_frag: Some("Fragment".to_string()),
            development: false,
            ..JsxOptions::enable()
        },
        ..TransformOptions::default()
    };
    options.typescript.jsx_pragma = "h".into();
    options.typescript.jsx_pragma_frag = "Fragment".into();
    options.helper_loader.mode = HelperLoaderMode::External;

    let transformer_return = Transformer::new(&allocator, path, &options)
        .build_with_scoping(semantic_return.semantic.into_scoping(), &mut program);
    if !transformer_return.diagnostics.is_empty() {
        return Err(oxc_diagnostics_to_canvas(
            &module,
            transformer_return.diagnostics.into_iter(),
            "canvas.compile.oxc.transform",
        ));
    }

    let CodegenReturn { code, .. } = Codegen::new()
        .with_options(CodegenOptions::default())
        .build(&program);

    Ok(format!(
        "{}\n{code}",
        canvas_runtime_binding_prelude(&analysis.import_bindings)
    ))
}

#[cfg(not(feature = "canvas-runtime"))]
pub(super) fn compile_canvas_component_js_with_oxc(
    _source: &str,
) -> Result<String, Vec<CanvasDiagnostic>> {
    Err(vec![compile_error(
        "Canvas TSX compilation requires the `canvas-runtime` feature",
        "canvas.compile.feature_disabled",
    )])
}
