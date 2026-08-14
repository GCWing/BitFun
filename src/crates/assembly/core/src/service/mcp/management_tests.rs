use super::{McpManagementMutation, McpManagementTransport};

#[test]
fn management_mutation_debug_redacts_connection_secrets() {
    let mutation = McpManagementMutation {
        transport: McpManagementTransport::Stdio,
        command: Some("secret-command".to_string()),
        args: vec!["--token=secret-argument".to_string()],
        env: std::collections::HashMap::from([("API_TOKEN".to_string(), "secret-env".to_string())]),
        headers: std::collections::HashMap::from([(
            "Authorization".to_string(),
            "Bearer secret-header".to_string(),
        )]),
        url: Some("https://example.com?token=secret-url".to_string()),
        auto_start: true,
        enabled: true,
        oauth: Some(serde_json::json!({"clientSecret": "secret-oauth"})),
        xaa: Some(serde_json::json!({"token": "secret-xaa"})),
    };

    let debug = format!("{mutation:?}");
    for secret in [
        "secret-command",
        "secret-argument",
        "secret-env",
        "secret-header",
        "secret-url",
        "secret-oauth",
        "secret-xaa",
    ] {
        assert!(!debug.contains(secret), "debug leaked {secret}");
    }
    assert!(debug.contains("API_TOKEN"));
    assert!(debug.contains("Authorization"));
}
