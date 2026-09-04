use openbitfun_opencode_adapter::{
    OpenCodeCommandProviderOptions, OpenCodeSubagentProvider, OpenCodeSubagentProviderOptions,
};
use openbitfun_product_domains::external_sources::{
    ExecutionDomainId, ExternalSourceContext, ExternalSourceScope,
};
use openbitfun_product_domains::external_subagents::{
    ExternalSubagentCompatibilityState, ExternalSubagentDiscoveryInput, ExternalSubagentMode,
    ExternalSubagentModelProfileRequest, ExternalSubagentModelRequest,
    ExternalSubagentSourceProvider, ExternalSubagentToolCapability,
};
use openbitfun_product_domains::tool_permissions::{
    PermissionEffect, PermissionEvaluator, PermissionRule,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn context(workspace: PathBuf) -> ExternalSourceContext {
    ExternalSourceContext {
        workspace_root: Some(workspace),
        execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
    }
}

fn provider(temp: &TempDir, workspace: &std::path::Path) -> OpenCodeSubagentProvider {
    OpenCodeSubagentProvider::new(OpenCodeSubagentProviderOptions {
        config: OpenCodeCommandProviderOptions {
            user_config_dir: temp.path().join("user"),
            legacy_user_config_dir: Some(temp.path().join("legacy")),
            explicit_config_file: None,
            explicit_config_dir: None,
            inline_config_content: None,
            project_config_enabled: true,
        },
        project_root_override: Some(workspace.to_path_buf()),
    })
}

fn discover(
    provider: &OpenCodeSubagentProvider,
    workspace: PathBuf,
    suppressed_sources: BTreeSet<openbitfun_product_domains::external_sources::SourceKey>,
) -> openbitfun_product_domains::external_subagents::ExternalSubagentProviderSnapshot {
    provider
        .discover(&ExternalSubagentDiscoveryInput {
            context: context(workspace),
            suppressed_sources,
        })
        .expect("discover OpenCode agents")
}

#[test]
fn opencode_builtin_tools_map_to_provider_neutral_capabilities() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user")).unwrap();
    fs::write(
        temp.path().join("user/opencode.json"),
        r#"{
          "agent": {
            "worker": {
              "prompt": "Make the requested change",
              "mode": "subagent",
              "tools": {
                "bash": true,
                "edit": true,
                "write": true
              }
            }
          }
        }"#,
    )
    .unwrap();

    let snapshot = discover(&provider(&temp, &workspace), workspace, BTreeSet::new());
    let definition = snapshot
        .definitions
        .iter()
        .find(|definition| definition.logical_id == "worker")
        .unwrap();
    let mappings = definition
        .requested_tools
        .selectors
        .iter()
        .map(|selector| (selector.source_name.as_str(), selector.canonical_capability))
        .collect::<Vec<_>>();

    assert_eq!(
        mappings,
        vec![
            ("bash", Some(ExternalSubagentToolCapability::ExecuteCommand)),
            ("edit", Some(ExternalSubagentToolCapability::EditFile)),
            ("write", Some(ExternalSubagentToolCapability::WriteFile)),
        ]
    );
    assert_eq!(
        definition.compatibility,
        ExternalSubagentCompatibilityState::Ready
    );
}

#[test]
fn omo_oracle_flat_permissions_become_provider_neutral_constraints() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user")).unwrap();
    fs::write(
        temp.path().join("user/opencode.json"),
        r#"{
          "agent": {
            "oracle": {
              "description": "Read-only consultation agent",
              "prompt": "Analyze the problem without modifying files",
              "mode": "subagent",
              "permission": {
                "write": "deny",
                "edit": "deny",
                "apply_patch": "deny",
                "task": "deny"
              }
            }
          }
        }"#,
    )
    .unwrap();
    let snapshot = discover(&provider(&temp, &workspace), workspace, BTreeSet::new());
    let definition = &snapshot.definitions[0];

    assert_eq!(
        definition.permission_constraints.rules(),
        [
            PermissionRule::new("edit", "*", PermissionEffect::Deny),
            PermissionRule::new("task", "*", PermissionEffect::Deny),
        ]
    );
    assert_eq!(
        definition.compatibility,
        ExternalSubagentCompatibilityState::ReadyWithDegradation
    );
    assert!(!definition
        .diagnostic_codes
        .contains(&"opencode_agent_permission_not_imported".to_string()));
}

#[test]
fn model_named_inherit_remains_an_opaque_opencode_reference() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user")).unwrap();
    fs::write(
        temp.path().join("user/opencode.json"),
        r#"{
          "agent": {
            "named": {
              "description": "Named model",
              "prompt": "Use the configured model",
              "mode": "subagent",
              "model": "inherit"
            }
          }
        }"#,
    )
    .unwrap();

    let definition =
        &discover(&provider(&temp, &workspace), workspace, BTreeSet::new()).definitions[0];
    assert_eq!(
        definition.requested_model,
        ExternalSubagentModelRequest::Reference {
            provider_hint: None,
            model_name: "inherit".to_string(),
        }
    );
}

#[test]
fn named_variant_is_preserved_as_a_profile_instead_of_guessed_as_reasoning_effort() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user")).unwrap();
    fs::write(
        temp.path().join("user/opencode.json"),
        r#"{
          "agent": {
            "review": {
              "description": "Variant reviewer",
              "prompt": "Review carefully",
              "mode": "subagent",
              "model": "openrouter/vendor/model",
              "variant": "high"
            }
          }
        }"#,
    )
    .unwrap();

    let definition =
        &discover(&provider(&temp, &workspace), workspace, BTreeSet::new()).definitions[0];
    assert_eq!(
        definition.requested_model_profile,
        Some(ExternalSubagentModelProfileRequest::NamedVariant {
            name: "high".to_string(),
        })
    );
    assert!(!definition
        .diagnostic_codes
        .contains(&"opencode_agent_variant_not_imported".to_string()));
    assert!(
        !matches!(
            definition.compatibility,
            ExternalSubagentCompatibilityState::Blocked
                | ExternalSubagentCompatibilityState::Invalid
        ),
        "variant support must not block an otherwise usable agent: {:?}",
        definition.diagnostic_codes
    );
}

#[test]
fn named_variant_without_an_agent_model_remains_inert_like_opencode() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user")).unwrap();
    fs::write(
        temp.path().join("user/opencode.json"),
        r#"{
          "agent": {
            "review": {
              "description": "Default-model reviewer",
              "prompt": "Review carefully",
              "mode": "subagent",
              "variant": "high"
            }
          }
        }"#,
    )
    .unwrap();

    let definition =
        &discover(&provider(&temp, &workspace), workspace, BTreeSet::new()).definitions[0];
    assert_eq!(
        definition.requested_model,
        ExternalSubagentModelRequest::Default
    );
    assert_eq!(definition.requested_model_profile, None);
    assert!(!matches!(
        definition.compatibility,
        ExternalSubagentCompatibilityState::Blocked | ExternalSubagentCompatibilityState::Invalid
    ));
    assert!(!definition
        .diagnostic_codes
        .iter()
        .any(|code| code.contains("variant")));
}

#[test]
fn empty_or_whitespace_variant_without_an_agent_model_is_also_inert() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user")).unwrap();
    fs::write(
        temp.path().join("user/opencode.json"),
        r#"{
          "agent": {
            "empty": {
              "prompt": "Review carefully",
              "mode": "subagent",
              "variant": ""
            },
            "spaced": {
              "prompt": "Review carefully",
              "mode": "subagent",
              "variant": " custom "
            }
          }
        }"#,
    )
    .unwrap();

    let snapshot = discover(&provider(&temp, &workspace), workspace, BTreeSet::new());
    assert_eq!(snapshot.definitions.len(), 2);
    assert!(snapshot.definitions.iter().all(|definition| {
        definition.requested_model_profile.is_none()
            && !matches!(
                definition.compatibility,
                ExternalSubagentCompatibilityState::Blocked
                    | ExternalSubagentCompatibilityState::Invalid
            )
            && !definition
                .diagnostic_codes
                .iter()
                .any(|code| code.contains("variant"))
    }));
}

#[test]
fn named_variant_with_surrounding_whitespace_is_not_rewritten() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user")).unwrap();
    fs::write(
        temp.path().join("user/opencode.json"),
        r#"{
          "agent": {
            "review": {
              "prompt": "Review carefully",
              "mode": "subagent",
              "model": "openrouter/vendor/model",
              "variant": " high "
            }
          }
        }"#,
    )
    .unwrap();

    let definition =
        &discover(&provider(&temp, &workspace), workspace, BTreeSet::new()).definitions[0];
    assert_eq!(definition.requested_model_profile, None);
    assert_eq!(
        definition.compatibility,
        ExternalSubagentCompatibilityState::Invalid
    );
    assert!(definition
        .diagnostic_codes
        .contains(&"opencode_agent_variant_invalid".to_string()));
}

#[test]
fn invalid_variant_text_isolated_to_its_agent_instead_of_failing_discovery() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user")).unwrap();
    fs::write(
        temp.path().join("user/opencode.json"),
        r#"{
          "agent": {
            "bad": {
              "prompt": "Review carefully",
              "mode": "subagent",
              "model": "openrouter/vendor/model",
              "variant": "bad\u0001value"
            },
            "good": {
              "prompt": "Review carefully",
              "mode": "subagent"
            }
          }
        }"#,
    )
    .unwrap();

    let snapshot = discover(&provider(&temp, &workspace), workspace, BTreeSet::new());
    assert_eq!(snapshot.definitions.len(), 2);
    let bad = snapshot
        .definitions
        .iter()
        .find(|definition| definition.logical_id == "bad")
        .unwrap();
    assert_eq!(
        bad.compatibility,
        ExternalSubagentCompatibilityState::Invalid
    );
    assert!(bad
        .diagnostic_codes
        .contains(&"opencode_agent_variant_invalid".to_string()));
}

#[test]
fn current_opencode_agent_permissions_preserve_ordered_resource_rules() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user")).unwrap();
    fs::write(
        temp.path().join("user/opencode.json"),
        r#"{
          "agents": {
            "reviewer": {
              "description": "Current OpenCode reviewer",
              "system": "Review without changing generated files",
              "mode": "subagent",
              "disabled": false,
              "permissions": [
                { "action": "read", "resource": "src/**", "effect": "allow" },
                { "action": "read", "resource": "src/generated/**", "effect": "ask" },
                { "action": "r*", "resource": "docs/**", "effect": "ask" },
                { "action": "edit", "resource": "*", "effect": "deny" }
              ]
            }
          }
        }"#,
    )
    .unwrap();
    fs::write(
        workspace.join("opencode.json"),
        r#"{
          "agents": {
            "reviewer": {
              "permissions": [
                { "action": "read", "resource": "secrets/**", "effect": "deny" }
              ]
            }
          }
        }"#,
    )
    .unwrap();

    let workspace_resource = dunce::canonicalize(&workspace)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    let snapshot = discover(&provider(&temp, &workspace), workspace, BTreeSet::new());
    let definition = &snapshot.definitions[0];

    assert_eq!(
        definition.prompt.expose(),
        "Review without changing generated files"
    );
    assert!(!definition.disabled);
    assert_eq!(
        definition.permission_constraints.rules(),
        [
            PermissionRule::new(
                "read",
                format!("{workspace_resource}/src/**"),
                PermissionEffect::Allow,
            ),
            PermissionRule::new(
                "read",
                format!("{workspace_resource}/src/generated/**"),
                PermissionEffect::Ask,
            ),
            PermissionRule::new(
                "r*",
                format!("{workspace_resource}/docs/**"),
                PermissionEffect::Ask,
            ),
            PermissionRule::new("edit", "*", PermissionEffect::Deny),
            PermissionRule::new(
                "read",
                format!("{workspace_resource}/secrets/**"),
                PermissionEffect::Deny,
            ),
        ]
    );
    assert_eq!(
        definition.compatibility,
        ExternalSubagentCompatibilityState::ReadyWithDegradation
    );
}

#[test]
fn current_path_permission_resources_expand_like_opencode() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    let home = temp.path().join("home");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(home.join(".opencode")).unwrap();
    fs::write(
        home.join(".opencode/opencode.json"),
        r#"{
          "agents": {
            "reviewer": {
              "system": "Review private files without editing them",
              "mode": "subagent",
              "permissions": [
                { "action": "read", "resource": "~/private/**", "effect": "ask" },
                { "action": "edit", "resource": "$HOME\\private\\**", "effect": "deny" },
                { "action": "bash", "resource": "$HOME/private/**", "effect": "deny" }
              ]
            }
          }
        }"#,
    )
    .unwrap();
    let provider = OpenCodeSubagentProvider::new(OpenCodeSubagentProviderOptions {
        config: OpenCodeCommandProviderOptions {
            user_config_dir: home.join(".opencode"),
            legacy_user_config_dir: Some(home.join(".opencode")),
            explicit_config_file: None,
            explicit_config_dir: None,
            inline_config_content: None,
            project_config_enabled: true,
        },
        project_root_override: Some(workspace.clone()),
    });

    let snapshot = discover(&provider, workspace, BTreeSet::new());
    let rules = snapshot.definitions[0].permission_constraints.rules();
    let home = dunce::canonicalize(home)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");

    assert_eq!(rules[0].resource, format!("{home}/private/**"));
    assert_eq!(rules[1].resource, format!(r"{home}\private\**"));
    assert_eq!(
        rules[2].resource, "$HOME/private/**",
        "OpenCode deliberately keeps bash resources as raw shell text"
    );
}

#[test]
fn v2_relative_path_permissions_use_the_opened_location_coordinate() {
    let temp = TempDir::new().unwrap();
    let project = temp.path().join("workspace");
    let opened = project.join("packages/app");
    fs::create_dir_all(project.join(".git")).unwrap();
    fs::create_dir_all(&opened).unwrap();
    fs::create_dir_all(temp.path().join("user")).unwrap();
    fs::write(
        temp.path().join("user/opencode.json"),
        r#"{
          "agents": {
            "reviewer": {
              "system": "Do not read location-local secrets",
              "mode": "subagent",
              "permissions": [
                { "action": "read", "resource": "secrets/**", "effect": "deny" }
              ]
            }
          }
        }"#,
    )
    .unwrap();
    let provider = OpenCodeSubagentProvider::new(OpenCodeSubagentProviderOptions {
        config: OpenCodeCommandProviderOptions {
            user_config_dir: temp.path().join("user"),
            legacy_user_config_dir: None,
            explicit_config_file: None,
            explicit_config_dir: None,
            inline_config_content: None,
            project_config_enabled: true,
        },
        project_root_override: Some(project.clone()),
    });

    let snapshot = discover(&provider, opened.clone(), BTreeSet::new());
    let constraints = &snapshot.definitions[0].permission_constraints;
    let opened = dunce::canonicalize(opened)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    let project = dunce::canonicalize(project)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");

    assert_eq!(
        constraints.rules()[0].resource,
        format!("{opened}/secrets/**")
    );
    assert_eq!(
        PermissionEvaluator::for_current_platform().evaluate_constraint_resource(
            "read",
            &format!("{opened}/secrets/key.txt"),
            constraints,
        ),
        PermissionEffect::Deny
    );
    assert_eq!(
        PermissionEvaluator::for_current_platform().evaluate_constraint_resource(
            "read",
            &format!("{project}/secrets/key.txt"),
            constraints,
        ),
        PermissionEffect::Allow,
        "Core V2 resources are active-Location relative; V1 worktree-relative resource maps stay unsupported"
    );
}

#[test]
fn ambiguous_cross_domain_path_patterns_fail_closed() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user")).unwrap();
    fs::write(
        temp.path().join("user/opencode.json"),
        r#"{
          "agents": {
            "reviewer": {
              "system": "Do not read secret files",
              "mode": "subagent",
              "permissions": [
                { "action": "read", "resource": "**/secret", "effect": "deny" }
              ]
            }
          }
        }"#,
    )
    .unwrap();

    let snapshot = discover(&provider(&temp, &workspace), workspace, BTreeSet::new());
    let definition = &snapshot.definitions[0];

    assert_eq!(
        definition.compatibility,
        ExternalSubagentCompatibilityState::Blocked
    );
    assert!(definition
        .diagnostic_codes
        .contains(&"opencode_agent_permission_resource_domain_ambiguous".to_string()));
}

#[cfg(windows)]
#[test]
fn windows_permission_actions_follow_opencode_case_insensitive_matching() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user")).unwrap();
    fs::write(
        temp.path().join("user/opencode.json"),
        r#"{
          "agents": {
            "reviewer": {
              "system": "Do not read files",
              "mode": "subagent",
              "permissions": [
                { "action": "R*", "resource": "*", "effect": "deny" }
              ]
            }
          }
        }"#,
    )
    .unwrap();

    let snapshot = discover(&provider(&temp, &workspace), workspace, BTreeSet::new());
    let constraints = &snapshot.definitions[0].permission_constraints;

    assert_eq!(constraints.rules()[0].action, "r*");
    assert_eq!(
        PermissionEvaluator::for_current_platform().evaluate_constraint_resource(
            "read",
            "C:/workspace/file.txt",
            constraints,
        ),
        PermissionEffect::Deny
    );
}

#[test]
fn v1_and_current_config_documents_merge_after_per_document_migration() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user")).unwrap();
    fs::write(
        temp.path().join("user/opencode.json"),
        r#"{
          "agent": {
            "reviewer": {
              "prompt": "Review using the user policy",
              "mode": "subagent",
              "tools": { "read": true },
              "permission": { "edit": "deny" }
            }
          }
        }"#,
    )
    .unwrap();
    fs::write(
        workspace.join("opencode.json"),
        r#"{
          "agents": {
            "reviewer": {
              "system": "Review using the project policy",
              "permissions": [
                { "action": "read", "resource": "secrets/**", "effect": "deny" }
              ]
            }
          }
        }"#,
    )
    .unwrap();

    let workspace_resource = dunce::canonicalize(&workspace)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    let snapshot = discover(&provider(&temp, &workspace), workspace, BTreeSet::new());
    let definition = &snapshot.definitions[0];

    assert_eq!(
        definition.prompt.expose(),
        "Review using the project policy"
    );
    assert_eq!(
        definition.permission_constraints.rules(),
        [
            PermissionRule::new("edit", "*", PermissionEffect::Deny),
            PermissionRule::new(
                "read",
                format!("{workspace_resource}/secrets/**"),
                PermissionEffect::Deny,
            ),
        ]
    );
    assert_eq!(
        definition.compatibility,
        ExternalSubagentCompatibilityState::Ready,
        "{:?}",
        definition.diagnostic_codes
    );
}

#[test]
fn later_enabled_document_recreates_agent_after_disabled_tombstone() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user")).unwrap();
    fs::write(
        temp.path().join("user/opencode.json"),
        r#"{
          "agents": {
            "reviewer": {
              "system": "Disabled user definition",
              "mode": "subagent",
              "disabled": true,
              "permissions": [
                { "action": "edit", "resource": "*", "effect": "deny" }
              ]
            }
          }
        }"#,
    )
    .unwrap();
    fs::write(
        workspace.join("opencode.json"),
        r#"{
          "agents": {
            "reviewer": {
              "system": "Enabled project definition",
              "mode": "subagent"
            }
          }
        }"#,
    )
    .unwrap();

    let snapshot = discover(&provider(&temp, &workspace), workspace, BTreeSet::new());
    let definition = &snapshot.definitions[0];

    assert!(!definition.disabled);
    assert_eq!(definition.prompt.expose(), "Enabled project definition");
    assert!(definition.permission_constraints.rules().is_empty());
}

#[test]
fn v1_disable_remains_effective_when_later_overlay_omits_it() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user")).unwrap();
    fs::write(
        temp.path().join("user/opencode.json"),
        r#"{
          "agent": {
            "reviewer": {
              "prompt": "Disabled user definition",
              "mode": "subagent",
              "disable": true,
              "permission": { "edit": "deny" }
            }
          }
        }"#,
    )
    .unwrap();
    fs::write(
        workspace.join("opencode.json"),
        r#"{
          "agent": {
            "reviewer": {
              "prompt": "Project overlay",
              "mode": "subagent"
            }
          }
        }"#,
    )
    .unwrap();

    let snapshot = discover(&provider(&temp, &workspace), workspace, BTreeSet::new());
    let definition = &snapshot.definitions[0];

    assert!(definition.disabled);
    assert_eq!(definition.prompt.expose(), "Project overlay");
    assert_eq!(
        definition.permission_constraints.rules(),
        [PermissionRule::new("edit", "*", PermissionEffect::Deny)]
    );
}

#[test]
fn v1_explicit_false_reenables_without_erasing_merged_fields() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user")).unwrap();
    fs::write(
        temp.path().join("user/opencode.json"),
        r#"{
          "agent": {
            "reviewer": {
              "prompt": "Disabled user definition",
              "mode": "subagent",
              "disable": true,
              "permission": { "edit": "deny" }
            }
          }
        }"#,
    )
    .unwrap();
    fs::write(
        workspace.join("opencode.json"),
        r#"{
          "agent": {
            "reviewer": {
              "prompt": "Explicitly enabled project overlay",
              "mode": "subagent",
              "disable": false
            }
          }
        }"#,
    )
    .unwrap();

    let snapshot = discover(&provider(&temp, &workspace), workspace, BTreeSet::new());
    let definition = &snapshot.definitions[0];

    assert!(!definition.disabled);
    assert_eq!(
        definition.prompt.expose(),
        "Explicitly enabled project overlay"
    );
    assert_eq!(
        definition.permission_constraints.rules(),
        [PermissionRule::new("edit", "*", PermissionEffect::Deny)]
    );
}

#[test]
fn v1_document_readds_after_v2_tombstone_without_losing_constraints() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(workspace.join(".opencode")).unwrap();
    fs::create_dir_all(temp.path().join("user")).unwrap();
    fs::write(
        temp.path().join("user/opencode.json"),
        r#"{
          "agents": {
            "reviewer": {
              "system": "Removed user definition",
              "mode": "subagent",
              "disabled": true
            }
          }
        }"#,
    )
    .unwrap();
    fs::write(
        workspace.join("opencode.json"),
        r#"{
          "agent": {
            "reviewer": {
              "prompt": "V1 project definition",
              "mode": "subagent",
              "permission": { "edit": "deny" }
            }
          }
        }"#,
    )
    .unwrap();
    fs::write(
        workspace.join(".opencode/opencode.json"),
        r#"{
          "agents": {
            "reviewer": {
              "system": "V2 project overlay",
              "mode": "subagent"
            }
          }
        }"#,
    )
    .unwrap();

    let snapshot = discover(&provider(&temp, &workspace), workspace, BTreeSet::new());
    let definition = &snapshot.definitions[0];

    assert_eq!(definition.prompt.expose(), "V2 project overlay");
    assert_eq!(
        definition.permission_constraints.rules(),
        [PermissionRule::new("edit", "*", PermissionEffect::Deny)],
        "the first document after a V2 tombstone re-adds the agent regardless of source schema"
    );
}

#[test]
fn user_markdown_applies_after_project_direct_config() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user/agents")).unwrap();
    fs::write(
        temp.path().join("user/agents/reviewer.md"),
        "---\ndescription: Reviewer\nmode: subagent\ntools:\n  read: true\npermission:\n  edit: deny\n---\nReview using the user policy.",
    )
    .unwrap();
    fs::write(
        workspace.join("opencode.json"),
        r#"{
          "agents": {
            "reviewer": {
              "system": "Review using the project policy",
              "permissions": [
                { "action": "read", "resource": "*", "effect": "deny" }
              ]
            }
          }
        }"#,
    )
    .unwrap();

    let snapshot = discover(&provider(&temp, &workspace), workspace, BTreeSet::new());
    let definition = &snapshot.definitions[0];

    assert_eq!(definition.prompt.expose(), "Review using the user policy.");
    assert_eq!(
        definition.permission_constraints.rules(),
        [
            PermissionRule::new("read", "*", PermissionEffect::Deny),
            PermissionRule::new("edit", "*", PermissionEffect::Deny),
        ]
    );
    assert_eq!(
        definition.compatibility,
        ExternalSubagentCompatibilityState::Ready
    );
}

#[test]
fn user_agent_markdown_permissions_follow_project_direct_config() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user/agents")).unwrap();
    fs::write(
        temp.path().join("user/agents/reviewer.md"),
        "---\ndescription: Reviewer\nmode: subagent\npermissions:\n  - action: read\n    resource: '*'\n    effect: allow\n---\nReview the project.",
    )
    .unwrap();
    fs::write(
        workspace.join("opencode.json"),
        r#"{
          "agents": {
            "reviewer": {
              "permissions": [
                { "action": "read", "resource": "*", "effect": "deny" }
              ]
            }
          }
        }"#,
    )
    .unwrap();

    let snapshot = discover(&provider(&temp, &workspace), workspace, BTreeSet::new());
    let constraints = &snapshot.definitions[0].permission_constraints;

    assert_eq!(
        constraints.rules(),
        [
            PermissionRule::new("read", "*", PermissionEffect::Deny),
            PermissionRule::new("read", "*", PermissionEffect::Allow),
        ]
    );
    assert_eq!(
        PermissionEvaluator::for_current_platform().evaluate_constraint_resource(
            "read",
            "C:/workspace/secrets/key.txt",
            constraints,
        ),
        PermissionEffect::Allow
    );
}

#[test]
fn explicit_alias_keeps_project_opencode_directory_position() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    let project_opencode = workspace.join(".opencode");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(&project_opencode).unwrap();
    fs::write(
        workspace.join("opencode.json"),
        r#"{
          "agents": {
            "reviewer": {
              "system": "Review the project",
              "mode": "subagent",
              "permissions": [
                { "action": "read", "resource": "*", "effect": "allow" }
              ]
            }
          }
        }"#,
    )
    .unwrap();
    fs::write(
        project_opencode.join("opencode.json"),
        r#"{
          "agents": {
            "reviewer": {
              "permissions": [
                { "action": "read", "resource": "*", "effect": "deny" }
              ]
            }
          }
        }"#,
    )
    .unwrap();
    let provider = OpenCodeSubagentProvider::new(OpenCodeSubagentProviderOptions {
        config: OpenCodeCommandProviderOptions {
            user_config_dir: temp.path().join("user"),
            legacy_user_config_dir: Some(temp.path().join("legacy")),
            explicit_config_file: None,
            explicit_config_dir: Some(project_opencode),
            inline_config_content: None,
            project_config_enabled: true,
        },
        project_root_override: Some(workspace.clone()),
    });

    let snapshot = discover(&provider, workspace, BTreeSet::new());

    assert_eq!(
        snapshot.definitions[0].permission_constraints.rules(),
        [
            PermissionRule::new("read", "*", PermissionEffect::Allow),
            PermissionRule::new("read", "*", PermissionEffect::Deny),
        ]
    );
}

#[test]
fn permission_shapes_that_cannot_be_enforced_fail_closed_per_agent() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user")).unwrap();
    fs::write(
        temp.path().join("user/opencode.json"),
        r#"{
          "agent": {
            "active-unenforceable": {
              "prompt": "Do not glob",
              "mode": "subagent",
              "permission": { "glob": "deny" }
            },
            "inactive-unknown": {
              "prompt": "Do not delegate",
              "mode": "subagent",
              "permission": { "call_omo_agent": "deny" }
            },
            "active-pattern": {
              "prompt": "Do not read",
              "mode": "subagent",
              "permission": { "r*": "deny" }
            },
            "ordered-legacy-overlap": {
              "prompt": "Order matters",
              "mode": "subagent",
              "tools": { "read": true },
              "permission": { "read": "allow", "*": "deny" }
            },
            "shorthand-enforceable": {
              "prompt": "Read nothing",
              "mode": "subagent",
              "tools": { "read": true },
              "permission": "deny"
            },
            "nested-resource-map": {
              "prompt": "Restrict commands",
              "mode": "subagent",
              "permission": { "bash": { "rm *": "deny" } }
            },
            "task-enabled": {
              "prompt": "Delegate work",
              "mode": "subagent",
              "tools": { "Task": true }
            }
          }
        }"#,
    )
    .unwrap();

    let snapshot = discover(&provider(&temp, &workspace), workspace, BTreeSet::new());
    let find = |id: &str| {
        snapshot
            .definitions
            .iter()
            .find(|definition| definition.logical_id == id)
            .unwrap()
    };

    assert_eq!(
        find("active-unenforceable").compatibility,
        ExternalSubagentCompatibilityState::Blocked
    );
    assert!(find("active-unenforceable")
        .diagnostic_codes
        .contains(&"opencode_agent_permission_action_not_enforceable".to_string()));
    assert_eq!(
        find("inactive-unknown").compatibility,
        ExternalSubagentCompatibilityState::ReadyWithDegradation
    );
    assert_eq!(
        find("active-pattern").compatibility,
        ExternalSubagentCompatibilityState::Blocked
    );
    assert_eq!(
        find("ordered-legacy-overlap").compatibility,
        ExternalSubagentCompatibilityState::Blocked
    );
    assert!(find("ordered-legacy-overlap")
        .diagnostic_codes
        .contains(&"opencode_agent_legacy_permission_action_pattern_not_imported".to_string()));
    assert_eq!(
        find("shorthand-enforceable").compatibility,
        ExternalSubagentCompatibilityState::Ready
    );
    assert_eq!(
        find("nested-resource-map").compatibility,
        ExternalSubagentCompatibilityState::Blocked
    );
    assert_eq!(
        find("task-enabled").compatibility,
        ExternalSubagentCompatibilityState::Blocked
    );
    assert!(find("task-enabled")
        .diagnostic_codes
        .contains(&"opencode_agent_task_tool_not_imported".to_string()));
}

#[test]
fn global_and_project_agent_fields_deep_merge_with_ordered_provenance() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user")).unwrap();
    fs::write(
        temp.path().join("user/opencode.json"),
        r#"{
          "agent": {
            "review": {
              "description": "Global review",
              "prompt": "Review using the global policy",
              "mode": "subagent",
              "model": "openrouter/anthropic/claude-sonnet-4",
              "tools": { "read": true, "grep": false }
            }
          }
        }"#,
    )
    .unwrap();
    fs::write(
        workspace.join("opencode.jsonc"),
        r#"{
          // Project copy must not alter execution behavior.
          "agent": { "review": { "description": "Project review", "color": "blue" } }
        }"#,
    )
    .unwrap();

    let provider = provider(&temp, &workspace);
    let first = discover(&provider, workspace.clone(), BTreeSet::new());
    let definition = &first.definitions[0];
    assert_eq!(definition.logical_id, "review");
    assert_eq!(definition.description, "Project review");
    assert_eq!(definition.prompt.expose(), "Review using the global policy");
    assert_eq!(definition.provenance.len(), 2);
    assert_eq!(definition.mode, ExternalSubagentMode::Subagent);
    assert_eq!(
        definition.requested_model,
        ExternalSubagentModelRequest::Reference {
            provider_hint: Some("openrouter".to_string()),
            model_name: "anthropic/claude-sonnet-4".to_string(),
        }
    );
    assert_eq!(definition.requested_tools.selectors.len(), 2);
    assert_eq!(
        definition.compatibility,
        ExternalSubagentCompatibilityState::ReadyWithDegradation
    );
    let behavior = definition.behavior_version.clone();

    fs::write(
        workspace.join("opencode.jsonc"),
        r#"{ "agent": { "review": { "description": "Project review updated", "color": "red" } } }"#,
    )
    .unwrap();
    let updated = discover(&provider, workspace.clone(), BTreeSet::new());
    assert_eq!(updated.definitions[0].behavior_version, behavior);
    assert_eq!(updated.definitions[0].description, "Project review updated");
    fs::write(
        workspace.join("opencode.jsonc"),
        r#"{ "agent": { "review": { "description": "Project review updated" } } }"#,
    )
    .unwrap();
    let color_removed = discover(&provider, workspace.clone(), BTreeSet::new());
    assert_eq!(color_removed.definitions[0].behavior_version, behavior);

    let project_source = updated
        .sources
        .iter()
        .find(|source| source.scope == ExternalSourceScope::Project)
        .unwrap()
        .key
        .clone();
    let without_project = discover(&provider, workspace, [project_source].into_iter().collect());
    assert_eq!(without_project.definitions[0].description, "Global review");
    assert_eq!(without_project.definitions[0].provenance.len(), 1);
}

#[test]
fn suppressed_agent_source_remains_discoverable_for_reenable() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user")).unwrap();
    fs::write(
        temp.path().join("user/opencode.json"),
        r#"{
          "agent": {
            "review": {
              "description": "Review agent",
              "prompt": "Review the change",
              "mode": "subagent"
            }
          }
        }"#,
    )
    .unwrap();

    let provider = provider(&temp, &workspace);
    let initial = discover(&provider, workspace.clone(), BTreeSet::new());
    let source_key = initial.sources[0].key.clone();
    fs::write(temp.path().join("user/opencode.json"), "{ invalid").unwrap();
    let suppressed = discover(
        &provider,
        workspace,
        [source_key.clone()].into_iter().collect(),
    );

    assert!(suppressed.definitions.is_empty());
    assert_eq!(suppressed.sources.len(), 1);
    assert_eq!(suppressed.sources[0].key, source_key);
}

#[test]
fn safe_subset_is_fail_closed_and_default_tools_are_explicit() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user")).unwrap();
    fs::write(
        temp.path().join("user/opencode.json"),
        r#"{
          "permission": { "bash": "deny" },
          "agent": {
            "defaulted": { "prompt": "Use safe defaults", "mode": "subagent" },
            "unsafe": {
              "prompt": "do-not-leak-this-prompt",
              "mode": "subagent",
              "permission": { "bash": "allow" },
              "options": { "providerSecret": "do-not-leak-value" },
              "futureField": "do-not-leak-unknown"
            },
            "wrongType": { "prompt": 42, "mode": "subagent" },
            "primaryOnly": { "prompt": "Primary", "mode": "primary" },
            "sampling": { "prompt": "Sampling", "temperature": 0.2, "color": "blue" }
          }
        }"#,
    )
    .unwrap();

    let snapshot = discover(&provider(&temp, &workspace), workspace, BTreeSet::new());
    let find = |id: &str| {
        snapshot
            .definitions
            .iter()
            .find(|item| item.logical_id == id)
            .unwrap()
    };
    let defaulted = find("defaulted");
    assert_eq!(
        defaulted
            .requested_tools
            .selectors
            .iter()
            .map(|item| item.canonical_capability.unwrap())
            .collect::<Vec<_>>(),
        vec![
            ExternalSubagentToolCapability::DirectoryList,
            ExternalSubagentToolCapability::ReadFile,
            ExternalSubagentToolCapability::GlobFiles,
            ExternalSubagentToolCapability::SearchText,
        ]
    );
    assert!(defaulted.requested_tools.uses_conservative_default);
    assert_eq!(
        defaulted.compatibility,
        ExternalSubagentCompatibilityState::Blocked,
        "ambient permission blocks every agent from this source"
    );
    assert_eq!(
        find("unsafe").compatibility,
        ExternalSubagentCompatibilityState::Blocked
    );
    assert_eq!(
        find("wrongType").compatibility,
        ExternalSubagentCompatibilityState::Invalid
    );
    assert_eq!(
        find("primaryOnly").compatibility,
        ExternalSubagentCompatibilityState::Blocked,
        "the source-level ambient permission remains unsupported"
    );
    assert!(!find("primaryOnly")
        .diagnostic_codes
        .contains(&"opencode_primary_agent_not_imported".to_string()));
    assert_eq!(
        find("sampling").compatibility,
        ExternalSubagentCompatibilityState::Blocked
    );
    let debug = format!("{snapshot:?}");
    assert!(!debug.contains("do-not-leak-this-prompt"));
    assert!(!debug.contains("do-not-leak-value"));
    assert!(!debug.contains("do-not-leak-unknown"));
}

#[test]
fn primary_and_all_roles_are_static_agent_profiles_without_role_degradation() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user")).unwrap();
    fs::write(
        temp.path().join("user/opencode.json"),
        r#"{
          "agent": {
            "primary": { "prompt": "Lead the session", "mode": "primary", "tools": { "read": true } },
            "both": { "prompt": "Lead or assist", "mode": "all", "tools": { "read": true } },
            "helper": { "prompt": "Assist only", "mode": "subagent", "tools": { "read": true } }
          }
        }"#,
    )
    .unwrap();

    let snapshot = discover(&provider(&temp, &workspace), workspace, BTreeSet::new());
    let find = |id: &str| {
        snapshot
            .definitions
            .iter()
            .find(|definition| definition.logical_id == id)
            .unwrap()
    };

    assert_eq!(find("primary").mode, ExternalSubagentMode::Primary);
    assert_eq!(find("both").mode, ExternalSubagentMode::All);
    assert_eq!(find("helper").mode, ExternalSubagentMode::Subagent);
    for id in ["primary", "both", "helper"] {
        assert_eq!(
            find(id).compatibility,
            ExternalSubagentCompatibilityState::Ready
        );
        assert!(!find(id)
            .diagnostic_codes
            .iter()
            .any(|code| code.contains("primary_facet") || code.contains("primary_agent")));
    }
}

#[test]
fn markdown_agent_directories_are_supported_and_legacy_modes_are_visible_but_blocked() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user/agents/review")).unwrap();
    fs::create_dir_all(temp.path().join("user/mode")).unwrap();
    fs::write(
        temp.path().join("user/agents/review/security.md"),
        "---\ndescription: Security review\nmode: subagent\ntools:\n  read: true\n---\nReview security boundaries.",
    )
    .unwrap();
    fs::write(
        temp.path().join("user/mode/legacy.md"),
        "---\ndescription: Legacy primary mode\n---\nAct as a primary agent.",
    )
    .unwrap();

    let snapshot = discover(&provider(&temp, &workspace), workspace, BTreeSet::new());
    let markdown = snapshot
        .definitions
        .iter()
        .find(|item| item.logical_id == "review/security")
        .unwrap();
    assert_eq!(markdown.description, "Security review");
    assert_eq!(markdown.prompt.expose(), "Review security boundaries.");
    assert_eq!(
        markdown.compatibility,
        ExternalSubagentCompatibilityState::Ready
    );

    let legacy = snapshot
        .definitions
        .iter()
        .find(|item| item.logical_id == "legacy")
        .unwrap();
    assert_eq!(
        legacy.compatibility,
        ExternalSubagentCompatibilityState::Blocked
    );
    assert!(legacy
        .diagnostic_codes
        .contains(&"opencode_legacy_primary_mode_not_imported".to_string()));
}

#[test]
fn missing_prompt_and_native_overlays_are_blocked_not_invalid() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user")).unwrap();
    fs::write(
        temp.path().join("user/opencode.json"),
        r#"{
          "agent": {
            "missing": { "description": "Relies on OpenCode defaults" },
            "defaulted": { "prompt": "Use conservative defaults", "mode": "subagent" },
            "Build": { "prompt": "Overlay native build", "mode": "subagent" }
          }
        }"#,
    )
    .unwrap();

    let snapshot = discover(&provider(&temp, &workspace), workspace, BTreeSet::new());
    for id in ["missing", "Build"] {
        assert_eq!(
            snapshot
                .definitions
                .iter()
                .find(|item| item.logical_id == id)
                .unwrap()
                .compatibility,
            ExternalSubagentCompatibilityState::Blocked
        );
    }
    let defaulted = snapshot
        .definitions
        .iter()
        .find(|item| item.logical_id == "defaulted")
        .unwrap();
    assert_eq!(
        defaulted.compatibility,
        ExternalSubagentCompatibilityState::ReadyWithDegradation
    );
    assert_eq!(defaulted.requested_tools.selectors.len(), 4);
}
