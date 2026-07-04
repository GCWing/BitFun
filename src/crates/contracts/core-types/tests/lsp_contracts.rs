use bitfun_core_types::lsp::{
    resolve_lsp_plugin_command_for_target, CapabilitiesConfig, LspPlugin, LspPluginRuntimeArch,
    LspPluginRuntimePlatform, LspPluginRuntimeTarget,
};

#[test]
fn lsp_plugin_manifest_defaults_preserve_legacy_shape() {
    let plugin: LspPlugin = serde_json::from_value(serde_json::json!({
        "id": "rust-analyzer",
        "name": "Rust Analyzer",
        "version": "1.0.0",
        "author": "BitFun",
        "description": "Rust language support",
        "server": {
            "command": "bin/${platform}/${arch}/rust-analyzer",
            "args": ["--stdio"]
        },
        "languages": ["rust"],
        "file_extensions": [".rs"],
        "capabilities": {
            "completion": true,
            "definition": true
        }
    }))
    .expect("legacy manifest should parse");

    assert_eq!(plugin.server.env.len(), 0);
    assert_eq!(plugin.server.runtime, None);
    assert_eq!(plugin.settings.len(), 0);
    assert_eq!(plugin.checksum, "");
    assert_eq!(plugin.min_bitfun_version, "");
    assert!(plugin.capabilities.completion);
    assert!(plugin.capabilities.definition);
    assert!(!plugin.capabilities.hover);
}

#[test]
fn lsp_capability_config_missing_fields_default_to_false() {
    let config: CapabilitiesConfig =
        serde_json::from_value(serde_json::json!({})).expect("empty capabilities should parse");

    assert!(!config.completion);
    assert!(!config.hover);
    assert!(!config.definition);
    assert!(!config.references);
    assert!(!config.rename);
    assert!(!config.formatting);
    assert!(!config.diagnostics);
    assert!(!config.inlay_hints);
}

#[test]
fn lsp_plugin_command_placeholder_resolution_is_contract_owned() {
    let command = "bin/${platform}/${os}/${arch}/server";
    let target =
        LspPluginRuntimeTarget::new(LspPluginRuntimePlatform::Macos, LspPluginRuntimeArch::Arm64);

    assert_eq!(
        resolve_lsp_plugin_command_for_target(command, target),
        "bin/darwin/darwin/arm64/server"
    );
}
