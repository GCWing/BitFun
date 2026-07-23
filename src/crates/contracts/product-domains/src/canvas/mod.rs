//! Canvas domain contracts and pure policy helpers.
//!
//! Canvas is an Agent-created display artifact. This module owns only stable
//! DTOs, pure policy, runtime artifact shapes, and narrow ports. TSX
//! compilation, HTML assembly, filesystem persistence, and UI integration
//! belong outside `product-domains`.

pub mod policy;
pub mod ports;
pub mod reference;
pub mod runtime;
pub mod types;

pub use policy::{
    validate_canvas_imports, validate_canvas_source_policy, CanvasImportPolicyDiagnostic,
    CanvasImportPolicyDiagnosticKind, BITFUN_CANVAS_IMPORT,
};
pub use ports::{
    CanvasPortError, CanvasPortErrorKind, CanvasPortFuture, CanvasPortResult, CanvasStoragePort,
};
pub use reference::{
    is_safe_canvas_ref_segment, parse_canvas_artifact_ref, CanvasArtifactRefParseError,
};
pub use runtime::*;
pub use types::*;
