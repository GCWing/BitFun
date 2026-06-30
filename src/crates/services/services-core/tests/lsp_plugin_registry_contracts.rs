use bitfun_core_types::lsp::{CapabilitiesConfig, LspPlugin, ServerConfig};
use bitfun_services_core::lsp::{
    resolve_plugin_command_for_target, LspPluginRegistryError, LspPluginRuntimeArch,
    LspPluginRuntimePlatform, LspPluginRuntimeTarget, PluginRegistry,
};
use std::collections::HashMap;

fn plugin(id: &str, languages: &[&str], extensions: &[&str]) -> LspPlugin {
    LspPlugin {
        id: id.to_string(),
        name: id.to_string(),
        version: "1.0.0".to_string(),
        author: "BitFun".to_string(),
        description: "test plugin".to_string(),
        server: ServerConfig {
            command: "server".to_string(),
            args: vec![],
            env: HashMap::new(),
            runtime: None,
        },
        languages: languages.iter().map(|value| value.to_string()).collect(),
        file_extensions: extensions.iter().map(|value| value.to_string()).collect(),
        capabilities: CapabilitiesConfig {
            completion: true,
            hover: false,
            definition: false,
            references: false,
            rename: false,
            formatting: false,
            diagnostics: false,
            inlay_hints: false,
        },
        settings: HashMap::new(),
        checksum: String::new(),
        min_bitfun_version: String::new(),
    }
}

#[test]
fn registry_preserves_language_extension_and_file_path_lookup() {
    let mut registry = PluginRegistry::new();
    registry
        .register(plugin("rust", &["rust"], &[".rs"]))
        .expect("plugin should register");

    assert_eq!(registry.count(), 1);
    assert!(registry.is_registered("rust"));
    assert_eq!(registry.find_by_language("rust").unwrap().id, "rust");
    assert_eq!(registry.find_by_extension("rs").unwrap().id, "rust");
    assert_eq!(registry.find_by_extension(".rs").unwrap().id, "rust");
    assert_eq!(
        registry
            .find_by_file_path("workspace/src/main.rs")
            .unwrap()
            .id,
        "rust"
    );
}

#[test]
fn registry_unregister_removes_plugin_indexes() {
    let mut registry = PluginRegistry::new();
    registry
        .register(plugin("python", &["python"], &[".py"]))
        .expect("plugin should register");

    registry
        .unregister("python")
        .expect("plugin should unregister");

    assert_eq!(registry.count(), 0);
    assert!(!registry.is_registered("python"));
    assert!(registry.find_by_language("python").is_none());
    assert!(registry.find_by_extension("py").is_none());
}

#[test]
fn registry_unregister_preserves_indexes_owned_by_newer_plugin() {
    let mut registry = PluginRegistry::new();
    registry
        .register(plugin("legacy-rust", &["rust"], &[".rs"]))
        .expect("legacy plugin should register");
    registry
        .register(plugin("current-rust", &["rust"], &[".rs"]))
        .expect("current plugin should register");

    assert_eq!(
        registry.find_by_language("rust").unwrap().id,
        "current-rust"
    );
    assert_eq!(registry.find_by_extension("rs").unwrap().id, "current-rust");

    registry
        .unregister("legacy-rust")
        .expect("legacy plugin should unregister without clearing current indexes");

    assert_eq!(registry.count(), 1);
    assert_eq!(
        registry.find_by_language("rust").unwrap().id,
        "current-rust"
    );
    assert_eq!(registry.find_by_extension("rs").unwrap().id, "current-rust");
}

#[test]
fn registry_duplicate_and_missing_errors_keep_legacy_messages() {
    let mut registry = PluginRegistry::new();
    registry
        .register(plugin("rust", &["rust"], &[".rs"]))
        .expect("plugin should register");

    assert_eq!(
        registry.register(plugin("rust", &["rust"], &[".rs"])),
        Err(LspPluginRegistryError::AlreadyRegistered(
            "rust".to_string()
        ))
    );
    assert_eq!(
        registry
            .register(plugin("rust", &["rust"], &[".rs"]))
            .unwrap_err()
            .to_string(),
        "Plugin already registered: rust"
    );
    assert_eq!(
        registry.unregister("missing"),
        Err(LspPluginRegistryError::NotFound("missing".to_string()))
    );
    assert_eq!(
        registry.unregister("missing").unwrap_err().to_string(),
        "Plugin not found: missing"
    );
}

#[test]
fn registry_supported_extensions_matches_desktop_api_shape() {
    let mut registry = PluginRegistry::new();
    registry
        .register(plugin("rust", &["rust"], &[".rs"]))
        .expect("rust plugin should register");
    registry
        .register(plugin(
            "typescript",
            &["typescript", "javascript"],
            &[".ts", ".tsx"],
        ))
        .expect("typescript plugin should register");

    let summary = registry.supported_extensions();

    assert_eq!(summary.extension_to_language.get(".rs").unwrap(), "rust");
    assert_eq!(
        summary.extension_to_language.get(".ts").unwrap(),
        "typescript"
    );
    assert_eq!(
        summary.extension_to_language.get(".tsx").unwrap(),
        "typescript"
    );
    assert!(summary
        .supported_languages
        .iter()
        .any(|language| language == "rust"));
    assert!(summary
        .supported_languages
        .iter()
        .any(|language| language == "typescript"));
    assert!(summary
        .supported_languages
        .iter()
        .any(|language| language == "javascript"));
}

#[test]
fn plugin_command_placeholder_resolution_is_target_driven() {
    let command = "bin/${platform}/${os}/${arch}/server";
    let target =
        LspPluginRuntimeTarget::new(LspPluginRuntimePlatform::Macos, LspPluginRuntimeArch::Arm64);

    assert_eq!(
        resolve_plugin_command_for_target(command, target),
        "bin/darwin/darwin/arm64/server"
    );
}
