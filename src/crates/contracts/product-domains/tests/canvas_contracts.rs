use bitfun_product_domains::canvas::{
    parse_canvas_artifact_ref, validate_canvas_imports, validate_canvas_source_policy,
    CanvasArtifact, CanvasArtifactRef, CanvasDiagnostic, CanvasDiagnosticCategory,
    CanvasDiagnosticSeverity, CanvasId, CanvasImportPolicyDiagnosticKind, CanvasRevision,
    CanvasScope, CanvasSessionId, CanvasSource, CanvasStatus, CanvasWorkspaceId,
    CANVAS_SOURCE_LANGUAGE_TSX,
};

#[test]
fn canvas_artifact_ref_uses_logical_uri_not_path() {
    let reference =
        CanvasArtifactRef::new(CanvasSessionId::new("session 1"), CanvasId::new("canvas 1"));

    let uri = reference.to_uri();

    assert_eq!(uri, "bitfun-canvas://session/session%201/canvas/canvas%201");
    assert!(!uri.contains("/Users/"));
    assert!(!uri.contains("\\"));

    let parsed = parse_canvas_artifact_ref(&uri).expect("reference should parse");
    assert_eq!(parsed, reference);
}

#[test]
fn canvas_artifact_ref_rejects_unsafe_path_segments() {
    for uri in [
        "bitfun-canvas://session/..%2Fother/canvas/canvas_1",
        "bitfun-canvas://session/../canvas/canvas_1",
        "bitfun-canvas://session/session_1/canvas/canvas%2Fwith%2Fslash",
        "bitfun-canvas://session/session%5C1/canvas/canvas_1",
    ] {
        assert!(
            parse_canvas_artifact_ref(uri).is_err(),
            "unsafe Canvas artifact ref should fail: {uri}"
        );
    }
}

#[test]
fn canvas_artifact_ref_rejects_non_canvas_uri() {
    let error =
        parse_canvas_artifact_ref("file:///Users/user/project/canvas.tsx").expect_err("must fail");

    assert_eq!(
        serde_json::to_value(error).unwrap(),
        serde_json::json!("invalid_scheme")
    );
}

#[test]
fn canvas_artifact_serializes_with_camel_case_fields() {
    let artifact = CanvasArtifact {
        id: CanvasId::new("canvas_1"),
        scope: CanvasScope::Session,
        session_id: CanvasSessionId::new("session_1"),
        workspace_id: CanvasWorkspaceId::new("workspace_1"),
        title: "Review Matrix".to_string(),
        description: Some("Generated from review notes".to_string()),
        source_revision: CanvasRevision::new("rev_1"),
        latest_compiled_revision: None,
        last_known_good_revision: None,
        status: CanvasStatus::SourceSaved,
        created_at: 1_000,
        updated_at: 1_001,
    };

    let value = serde_json::to_value(&artifact).unwrap();

    assert_eq!(value["sourceRevision"], "rev_1");
    assert_eq!(value["sessionId"], "session_1");
    assert_eq!(value["workspaceId"], "workspace_1");
    assert!(value.get("latest_compiled_revision").is_none());
}

#[test]
fn canvas_source_defaults_to_tsx_contract() {
    let source = CanvasSource::new_tsx(
        CanvasId::new("canvas_1"),
        CanvasRevision::new("rev_1"),
        "review.tsx",
        "export default function Review() { return null; }",
        "0.1.0",
        42,
    );

    assert_eq!(source.language, CANVAS_SOURCE_LANGUAGE_TSX);
    assert_eq!(source.filename, "review.tsx");
}

#[test]
fn canvas_diagnostic_shape_is_structured() {
    let diagnostic = CanvasDiagnostic {
        severity: CanvasDiagnosticSeverity::Error,
        category: CanvasDiagnosticCategory::ImportPolicy,
        message: "Only bitfun/canvas imports are allowed".to_string(),
        code: Some("canvas.import.unsupported".to_string()),
        line: Some(2),
        column: Some(8),
        suggested_fix: Some("Import UI helpers from bitfun/canvas.".to_string()),
    };

    let value = serde_json::to_value(&diagnostic).unwrap();

    assert_eq!(value["severity"], "error");
    assert_eq!(value["category"], "import_policy");
    assert_eq!(
        value["suggestedFix"],
        "Import UI helpers from bitfun/canvas."
    );
}

#[test]
fn canvas_import_policy_rejects_relative_and_dynamic_imports() {
    let source = r#"
import { Stack } from 'bitfun/canvas';
import React from 'react';
import helper from './helper';
export * from './exports';
const later = import('lodash');
"#;

    let diagnostics = validate_canvas_imports(source);

    assert_eq!(diagnostics.len(), 3);
    assert_eq!(
        diagnostics[0].kind,
        CanvasImportPolicyDiagnosticKind::RelativeImport
    );
    assert_eq!(diagnostics[0].specifier, "./helper");
    assert_eq!(diagnostics[0].line, Some(4));
    assert_eq!(
        diagnostics[1].kind,
        CanvasImportPolicyDiagnosticKind::RelativeImport
    );
    assert_eq!(diagnostics[1].specifier, "./exports");
    assert_eq!(diagnostics[1].line, Some(5));
    assert_eq!(
        diagnostics[2].kind,
        CanvasImportPolicyDiagnosticKind::DynamicImport
    );
    assert_eq!(diagnostics[2].specifier, "lodash");
    assert_eq!(diagnostics[2].line, Some(6));
}

#[test]
fn canvas_import_policy_allows_canvas_compat_imports() {
    let source = r#"
import { useState, useEffect } from 'react';
import { Stack } from 'cursor/canvas';
import { Text } from 'bitfun/canvas';
"#;

    let diagnostics = validate_canvas_imports(source);

    assert!(diagnostics.is_empty());
}

#[test]
fn canvas_source_policy_returns_structured_diagnostics() {
    let mut source = CanvasSource::new_tsx(
        CanvasId::new("canvas_1"),
        CanvasRevision::new("rev_1"),
        "canvas.jsx",
        "import React from 'react'; function C() { return null; }",
        "0.1.0",
        1,
    );
    source.language = "jsx".to_string();

    let diagnostics = validate_canvas_source_policy(&source);

    assert_eq!(diagnostics.len(), 3);
    assert_eq!(
        diagnostics[0].category,
        CanvasDiagnosticCategory::Unsupported
    );
    assert_eq!(
        diagnostics[0].code.as_deref(),
        Some("canvas.source.language_unsupported")
    );
    assert_eq!(
        diagnostics[1].category,
        CanvasDiagnosticCategory::Unsupported
    );
    assert_eq!(
        diagnostics[2].category,
        CanvasDiagnosticCategory::TypeScript
    );
    assert_eq!(
        diagnostics[2].category,
        CanvasDiagnosticCategory::TypeScript
    );
}
