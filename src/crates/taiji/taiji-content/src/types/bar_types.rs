use chrono::{DateTime, Utc};

/// K-line bar data (minimal rendering subset).
///
/// This is intentionally a separate type from [`taiji_engine::types::bar::RawBar`].
/// The engine's `RawBar` carries additional fields (`freq`, `id`, `amount`,
/// `open_interest`, `delta`, and `Symbol` wrapping) that the renderer does not
/// need. Keeping a minimal subset here avoids pulling taiji-engine into the
/// rendering crate and keeps the rendering surface independent of pipeline
/// implementation details.
#[derive(Debug, Clone)]
pub struct RawBar {
    pub symbol: String,
    pub dt: DateTime<Utc>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub vol: f64,
}
