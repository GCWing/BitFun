use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelSelectorKind {
    Primary,
    Fast,
    Explicit(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelSelectorError {
    PrimaryUnavailable,
    FastUnavailable,
}

impl fmt::Display for ModelSelectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PrimaryUnavailable => write!(f, "Primary model not configured or invalid"),
            Self::FastUnavailable => write!(
                f,
                "Fast model not configured or invalid, and primary model not configured or invalid"
            ),
        }
    }
}

impl std::error::Error for ModelSelectorError {}

pub fn classify_model_selector(model_id: &str) -> ModelSelectorKind {
    let trimmed = model_id.trim();
    match trimmed {
        "" | "auto" | "default" | "primary" => ModelSelectorKind::Primary,
        "fast" => ModelSelectorKind::Fast,
        _ => ModelSelectorKind::Explicit(trimmed.to_string()),
    }
}

pub fn resolve_required_model_selector(
    model_id: &str,
    mut resolve_selection: impl FnMut(&str) -> Option<String>,
    mut resolve_reference: impl FnMut(&str) -> Option<String>,
) -> Result<String, ModelSelectorError> {
    match classify_model_selector(model_id) {
        ModelSelectorKind::Primary => {
            resolve_selection("primary").ok_or(ModelSelectorError::PrimaryUnavailable)
        }
        ModelSelectorKind::Fast => {
            resolve_selection("fast").ok_or(ModelSelectorError::FastUnavailable)
        }
        ModelSelectorKind::Explicit(model_ref) => {
            Ok(resolve_reference(&model_ref).unwrap_or(model_ref))
        }
    }
}

pub fn resolve_cache_model_selector(
    model_id: &str,
    mut resolve_selection: impl FnMut(&str) -> Option<String>,
    mut resolve_reference: impl FnMut(&str) -> Option<String>,
) -> String {
    match classify_model_selector(model_id) {
        ModelSelectorKind::Primary => {
            resolve_selection("primary").unwrap_or_else(|| "primary".to_string())
        }
        ModelSelectorKind::Fast => resolve_selection("fast").unwrap_or_else(|| "fast".to_string()),
        ModelSelectorKind::Explicit(model_ref) => {
            resolve_reference(&model_ref).unwrap_or(model_ref)
        }
    }
}
