//! Stable method-name schema and naming validation.

pub const INITIALIZE: &str = "app/initialize";
pub const HEALTH: &str = "app/health";
pub const SEARCH_SESSION_CONTENT: &str = "search/sessionContent";

/// Validate the App Server `domain/lowerCamelCaseOperation` convention.
pub fn is_valid_method_name(method: &str) -> bool {
    let Some((domain, operation)) = method.split_once('/') else {
        return false;
    };
    if domain.is_empty() || operation.is_empty() || operation.contains('/') {
        return false;
    }
    is_lower_camel_identifier(domain) && is_lower_camel_identifier(operation)
}

fn is_lower_camel_identifier(value: &str) -> bool {
    value
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
        && value.chars().all(|ch| ch.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::is_valid_method_name;

    #[test]
    fn method_names_require_one_domain_separator_and_lower_camel_parts() {
        assert!(is_valid_method_name("app/initialize"));
        assert!(is_valid_method_name("session/forkBeforeTurn"));
        assert!(!is_valid_method_name("initialize"));
        assert!(!is_valid_method_name("App/initialize"));
        assert!(!is_valid_method_name("app/Initialize"));
        assert!(!is_valid_method_name("app/session/restore"));
        assert!(!is_valid_method_name("app/restore_session"));
    }
}
