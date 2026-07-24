#[cfg(test)]
mod tests {
    use tokio::sync::broadcast::error::TryRecvError;

    use super::{
        action_opens_extension_management, agent_event_stream_failure, apply_agent_mode_feedback,
        apply_model_selection_feedback, builtin_command_reconfirmation,
        builtin_hook_help_requested, command_route, external_agent_attention,
        external_agent_diagnostic_lines, external_agent_pending_notice_key,
        external_agent_result_is_stale, external_agent_review_text, external_command_projections,
        external_control_review_text, external_hook_help_text, external_integration_policy_lines,
        external_operation_error_status, external_tool_mutation_result_label,
        external_tool_pending_notice_key, external_tool_result_is_stale, external_tool_review_text,
        external_tool_run_location_label, mark_active_turn_failed,
        merge_external_agent_mutation_snapshot, mode_change_blocks_typed_submission,
        mode_change_completion_should_exit, native_command_conflict_key, parse_command_token,
        parse_external_agent_review_action, parse_external_control_action,
        parse_external_tool_review_action, previous_session_mode_change_status,
        render_external_hook_catalog, CommandQualifier, CommandRoute, ExternalAgentReviewAction,
        ExternalControlUiAction, ExternalSourceConflictPreferences, ExternalToolReviewAction,
        ModeSelectionApplyOutcome, ModelSelectionApplyOutcome,
    };
    use crate::actions::{action_conflict_behavior_version, ActionState, ResolvedKeymap};
    use crate::chat_state::ChatState;
    use crate::config::ShortcutsConfig;
    use crate::ui::command_menu::{ExternalCommandProjection, NativeCommandCollisionProjection};
    use bitfun_core::external_hooks::ExternalHookCatalogSnapshotV1;
    use bitfun_core::external_sources::{
        ExternalSourceAssetKind, ExternalSourceCatalogSnapshot, ExternalSourceControlSnapshotV1,
        ExternalSourceDiagnostic, ExternalSourceDiagnosticSeverity, ExternalSourceOperationError,
        ExternalSourceOperationErrorCode, ExternalSubagentActivationState,
        ExternalToolActivationState,
    };
    use std::collections::{BTreeMap, BTreeSet};

    fn external_command(
        name: &str,
        selected_candidate_id: Option<&str>,
    ) -> ExternalCommandProjection {
        ExternalCommandProjection {
            action_id: format!("external-command:{name}"),
            command_name: name.to_string(),
            invocation_alias: format!("/{name}"),
            candidate_id: format!("external:{name}"),
            content_version: "v1".to_string(),
            description: "External command".to_string(),
            restricted: false,
            provider_conflict_key: None,
            native_collision: Some(NativeCommandCollisionProjection {
                native_action_id: name.to_string(),
                native_candidate_id: format!("bitfun.cli:{name}"),
                external_candidate_id: format!("external:{name}"),
                conflict_key: "conflict-v1".to_string(),
                selected_candidate_id: selected_candidate_id.map(str::to_string),
            }),
        }
    }

    #[test]
    fn external_control_commands_use_one_small_closed_action_set() {
        assert_eq!(
            parse_external_control_action("").unwrap(),
            ExternalControlUiAction::Show
        );
        assert_eq!(
            parse_external_control_action("status").unwrap(),
            ExternalControlUiAction::Show
        );
        assert_eq!(
            parse_external_control_action("refresh").unwrap(),
            ExternalControlUiAction::Refresh
        );
        assert_eq!(
            parse_external_control_action("safe-mode on").unwrap(),
            ExternalControlUiAction::SetSafeMode(true)
        );
        assert_eq!(
            parse_external_control_action("safe-mode off").unwrap(),
            ExternalControlUiAction::SetSafeMode(false)
        );
        assert_eq!(
            parse_external_control_action("source disable opencode.commands:project").unwrap(),
            ExternalControlUiAction::SetSourceEnabled {
                source_key: "opencode.commands:project".to_string(),
                enabled: false,
            }
        );
        assert_eq!(
            parse_external_control_action("source enable opencode.commands:project").unwrap(),
            ExternalControlUiAction::SetSourceEnabled {
                source_key: "opencode.commands:project".to_string(),
                enabled: true,
            }
        );
        assert!(parse_external_control_action("safe-mode toggle").is_err());
        assert!(parse_external_control_action("enable-everything").is_err());
    }

    #[test]
    fn external_control_status_projects_shared_runtime_facts() {
        let control: ExternalSourceControlSnapshotV1 = serde_json::from_value(serde_json::json!({
            "schemaVersion": 1,
            "executionDomainId": "local-user",
            "refreshGeneration": 9,
            "preferenceRevision": 4,
            "safeMode": true,
            "hostCapabilities": {
                "canRefresh": true,
                "canMutatePolicy": true,
                "canManageSources": true,
                "canApproveRuntime": true,
                "canExecuteExternalAssets": true,
                "canSetSafeMode": true
            },
            "sources": [{
                "stableKey": "opencode.commands:project",
                "ecosystemId": "opencode",
                "displayName": "OpenCode project commands",
                "scope": "project",
                "contentVersion": "v1",
                "discovery": "current",
                "desired": "enabled",
                "review": { "state": "not_required" },
                "runtime": "not_applicable",
                "support": "supported",
                "effectiveStatus": "available"
            }],
            "capabilities": [{
                "kind": "tool",
                "revision": 9,
                "itemCount": 2,
                "pendingReviewCount": 1,
                "unresolvedConflictCount": 0,
                "runtime": "inactive",
                "support": "supported"
            }],
            "diagnostics": [],
            "recoveryActions": [{ "type": "exit_safe_mode" }]
        }))
        .unwrap();

        let text = external_control_review_text(&control);
        assert!(text.contains("Safe Mode: on"));
        assert!(text.contains("Generation: 9"));
        assert!(text.contains("Execution domain: local-user"));
        assert!(text.contains("New external Tool, Agent, and MCP calls are blocked"));
        assert!(text.contains("restarting the Host turns it off"));
        assert!(text.contains("Source opencode.commands:project"));
        assert!(text.contains("source disable <source-key>"));
        assert!(text.contains("Tools: 2 items, 1 review, 0 conflicts, inactive"));
        assert!(text.contains("/builtin:extensions safe-mode off"));
    }

    #[test]
    fn external_control_status_keeps_empty_provider_failures_actionable() {
        let control: ExternalSourceControlSnapshotV1 = serde_json::from_value(serde_json::json!({
            "schemaVersion": 1,
            "executionDomainId": "local-user",
            "refreshGeneration": 10,
            "preferenceRevision": 4,
            "safeMode": false,
            "hostCapabilities": {
                "canRefresh": true,
                "canMutatePolicy": true,
                "canManageSources": true,
                "canApproveRuntime": true,
                "canExecuteExternalAssets": true,
                "canSetSafeMode": true
            },
            "sources": [],
            "capabilities": [{
                "kind": "tool",
                "revision": 10,
                "itemCount": 0,
                "pendingReviewCount": 0,
                "unresolvedConflictCount": 0,
                "runtime": "inactive",
                "support": "partial"
            }],
            "diagnostics": [{
                "severity": "error",
                "assetKind": "tool",
                "code": "external_tool.runtime_unavailable",
                "message": "Node runtime is unavailable"
            }],
            "recoveryActions": [
                { "type": "refresh" },
                { "type": "install_runtime" }
            ]
        }))
        .unwrap();

        let text = external_control_review_text(&control);
        assert!(text.contains("Tools: 0 items, 0 review, 0 conflicts, inactive, support: partial"));
        assert!(text.contains("Issues"));
        assert!(text.contains("[external_tool.runtime_unavailable]"));
        assert!(text.contains("Recovery"));
        assert!(text.contains("/builtin:extensions refresh"));
        assert!(text.contains("install or repair the required runtime"));
    }

    fn external_tool_review_snapshot() -> ExternalSourceCatalogSnapshot {
        serde_json::from_value(serde_json::json!({
            "generation": 3,
            "discoveryPending": false,
            "sources": [{
                "stableKey": "opencode-tools-project",
                "record": {
                    "key": { "providerId": "opencode.tools", "sourceId": "project" },
                    "ecosystemId": "opencode",
                    "displayName": "OpenCode project tools",
                    "sourceKind": "tools",
                    "scope": "project",
                    "location": "<workspace>/.opencode/tools",
                    "executionDomainId": "local:D:/repo",
                    "health": "available",
                    "contentVersion": "source-v1"
                },
                "lifecycle": "available"
            }],
            "commands": [],
            "tools": [{
                "definition": {
                    "id": {
                        "target": {
                            "source": { "providerId": "opencode.tools", "sourceId": "project" },
                            "localId": "review.js"
                        },
                        "exportId": "default"
                    },
                    "name": "review",
                    "descriptionPreview": "Review a change",
                    "modulePath": "<workspace>/.opencode/tools/review.js",
                    "workingDirectory": "<workspace>/",
                    "runtimeKind": "java_script",
                    "capabilities": ["file_system", "network", "environment", "process"],
                    "contentVersion": "content-v1",
                    "staticStatus": { "state": "ready" }
                },
                "approvalKey": "approval-v1",
                "decisionKey": "decision-v1",
                "activation": { "state": "approval_required" }
            }, {
                "definition": {
                    "id": {
                        "target": {
                            "source": { "providerId": "opencode.tools", "sourceId": "project" },
                            "localId": "weather.js"
                        },
                        "exportId": "default"
                    },
                    "name": "weather",
                    "descriptionPreview": "Read weather",
                    "modulePath": "<workspace>/.opencode/tools/weather.js",
                    "workingDirectory": "<workspace>/",
                    "runtimeKind": "java_script",
                    "capabilities": ["network"],
                    "contentVersion": "content-v1",
                    "staticStatus": { "state": "ready" }
                },
                "approvalKey": "approval-v2",
                "decisionKey": "decision-v2",
                "activation": { "state": "declined" }
            }, {
                "definition": {
                    "id": {
                        "target": {
                            "source": { "providerId": "opencode.tools", "sourceId": "project" },
                            "localId": "deploy.js"
                        },
                        "exportId": "default"
                    },
                    "name": "deploy",
                    "descriptionPreview": "Deploy a build",
                    "modulePath": "<workspace>/.opencode/tools/deploy.js",
                    "workingDirectory": "<workspace>/",
                    "runtimeKind": "java_script",
                    "capabilities": ["process"],
                    "contentVersion": "content-v1",
                    "staticStatus": { "state": "ready" }
                },
                "approvalKey": "approval-v3",
                "decisionKey": "decision-v3",
                "activation": { "state": "active" }
            }, {
                "definition": {
                    "id": {
                        "target": {
                            "source": { "providerId": "opencode.tools", "sourceId": "project" },
                            "localId": "broken.ts"
                        },
                        "exportId": "default"
                    },
                    "name": "broken",
                    "descriptionPreview": "Broken tool",
                    "modulePath": "<workspace>/.opencode/tools/broken.ts",
                    "workingDirectory": "<workspace>/",
                    "runtimeKind": "type_script",
                    "capabilities": ["file_system"],
                    "contentVersion": "content-v1",
                    "staticStatus": { "state": "ready" }
                },
                "approvalKey": "approval-v4",
                "decisionKey": "decision-v4",
                "activation": {
                    "state": "load_failed",
                    "reason": "PR2 worker could not import the module"
                }
            }],
            "toolApprovalRequests": [{
                "approvalKey": "approval-v1",
                "decisionKey": "decision-v1",
                "targetId": {
                    "source": { "providerId": "opencode.tools", "sourceId": "project" },
                    "localId": "review.js"
                },
                "sourceDisplayName": "OpenCode project tools",
                "sourceScope": "project",
                "sourceLocation": "<workspace>/.opencode/tools/review.js",
                "workingDirectory": "<workspace>/",
                "runtimeKind": "java_script",
                "capabilities": ["file_system", "network", "environment", "process"],
                "contentVersion": "content-v1",
                "toolNames": ["review"]
            }],
            "toolConflicts": [{
                "conflictKey": "conflict-v1",
                "toolName": "review",
                "candidates": [{
                    "candidateId": "bitfun:review",
                    "displayName": "BitFun review",
                    "kind": "built_in",
                    "providerId": "bitfun",
                    "contentVersion": "builtin-v1"
                }, {
                    "candidateId": "external:review",
                    "displayName": "OpenCode review",
                    "kind": "external",
                    "providerId": "opencode.tools",
                    "contentVersion": "content-v1",
                    "source": { "providerId": "opencode.tools", "sourceId": "project" },
                    "sourceLocation": "<workspace>/.opencode/tools/review.js"
                }]
            }],
            "integrationPolicy": {
                "schemaMajor": 1,
                "status": "compatible",
                "userDefaults": { "enabled": true },
                "globalEffective": {
                    "enabled": true,
                    "ecosystems": {
                        "opencode": {
                            "ecosystemId": "opencode",
                            "mode": "recommended",
                            "capabilities": {
                                "command": "auto",
                                "tool": "ask_before_use",
                                "subagent": "ask_before_use",
                                "mcp": "ask_before_use"
                            }
                        }
                    }
                },
                "effective": {
                    "enabled": true,
                    "ecosystems": {
                        "opencode": {
                            "ecosystemId": "opencode",
                            "mode": "recommended",
                            "capabilities": {
                                "command": "auto",
                                "tool": "ask_before_use",
                                "subagent": "ask_before_use",
                                "mcp": "ask_before_use"
                            }
                        }
                    }
                },
                "registeredEcosystems": [{
                    "ecosystemId": "opencode",
                    "displayName": "OpenCode",
                    "adapterRevision": "1",
                    "capabilities": [
                        {
                            "capabilityId": "command",
                            "recommendedAccess": "auto",
                            "safetyCeiling": "auto"
                        },
                        {
                            "capabilityId": "tool",
                            "recommendedAccess": "ask_before_use",
                            "safetyCeiling": "ask_before_use"
                        },
                        {
                            "capabilityId": "subagent",
                            "recommendedAccess": "ask_before_use",
                            "safetyCeiling": "ask_before_use"
                        },
                        {
                            "capabilityId": "mcp",
                            "recommendedAccess": "ask_before_use",
                            "safetyCeiling": "ask_before_use"
                        }
                    ]
                }]
            },
            "diagnostics": [{
                "severity": "warning",
                "code": "opencode.tool.directory_read_failed",
                "message": "PR2 worker could not read one tool directory",
                "source": { "providerId": "opencode.tools", "sourceId": "project" }
            }]
        }))
        .unwrap()
    }

    #[test]
    fn external_review_projects_effective_scope_and_capability_policy() {
        let lines = external_integration_policy_lines(&external_tool_review_snapshot());
        let text = lines.join("\n");

        assert!(text.contains("Access: enabled"));
        assert!(text.contains("this project inherits global settings"));
        assert!(text.contains("OpenCode: recommended"));
        assert!(text.contains("command auto"));
        assert!(text.contains("tool ask"));
        assert!(text.contains("bitfun config external --help"));
    }

    #[test]
    fn external_operation_errors_use_stable_tui_copy_without_raw_details() {
        let stale = ExternalSourceOperationError::new(
            ExternalSourceOperationErrorCode::StaleRevision,
            "raw stale detail",
            true,
        )
        .with_default_recovery_actions();
        let policy = ExternalSourceOperationError::new(
            ExternalSourceOperationErrorCode::PolicyLimited,
            "raw policy detail",
            false,
        )
        .with_default_recovery_actions();
        let internal = ExternalSourceOperationError::new(
            ExternalSourceOperationErrorCode::Internal,
            "database password must not be shown",
            true,
        )
        .with_correlation_id("external-source-ref-9")
        .with_default_recovery_actions();

        let stale_status = external_operation_error_status("tools", &stale);
        assert!(stale_status.contains("settings changed"));
        assert!(stale_status.contains("/builtin:tools refresh"));
        assert!(!stale_status.contains("raw stale detail"));

        let policy_status = external_operation_error_status("agents", &policy);
        assert!(policy_status.contains("safety policy"));
        assert!(policy_status.contains("review the listed external items"));
        assert!(!policy_status.contains("raw policy detail"));

        let internal_status = external_operation_error_status("tools", &internal);
        assert!(internal_status.contains("external-source-ref-9"));
        assert!(!internal_status.contains("database password"));
    }

    #[test]
    fn external_tool_review_summary_discloses_execution_boundary_and_commands() {
        let summary = external_tool_review_text(Some(&external_tool_review_snapshot()));

        assert!(summary.contains("BitFun and MCP"));
        assert!(summary.contains("External AI applications"));
        assert!(summary.contains("Use /mcps to manage MCP servers"));
        assert!(summary.contains("BitFun does not run external code while checking sources"));
        assert!(summary.contains("filesystem, network, process, environment variables"));
        assert!(summary.contains("inherited environment variables"));
        assert!(summary.contains("processes it starts may keep running after cancellation"));
        assert!(summary.contains("/builtin:tools enable 1"));
        assert!(summary.contains("/builtin:tools choose 1 2"));
        assert!(summary.contains("<workspace>/.opencode/tools/review.js"));
        assert!(summary.contains("Source folder: <workspace>/.opencode/tools"));
        assert!(summary.contains("Applies to: current workspace"));
        assert!(summary.contains("Runs in: this computer"));
        assert!(!summary.contains("local:D:/repo"));
        assert!(summary.contains("disabled"));
        assert!(summary.contains("enabled"));
        assert!(summary.contains("loaded and ready to use"));
        assert!(summary.contains("could not load"));
        assert!(summary.contains("<workspace>/.opencode/tools/broken.ts"));
        assert!(!summary.contains("D:/repo"));
        assert!(summary.contains("Issues"));
        assert!(summary.contains("Technical details:"));
        assert!(summary.contains("opencode.tool.directory_read_failed"));
        assert!(!summary.contains("PR2"));
    }

    #[test]
    fn external_tool_runtime_recovery_starts_with_refresh_without_restart_pressure() {
        let mut snapshot = external_tool_review_snapshot();
        snapshot.tools[0].activation = ExternalToolActivationState::RuntimeUnavailable {
            reason: "BitFun could not find Node.js for external tools".to_string(),
        };

        let summary = external_tool_review_text(Some(&snapshot));
        assert!(summary.contains("Install or repair Node.js, then refresh"));
        assert!(summary.contains("continue without external JavaScript tools"));
        assert!(!summary.to_ascii_lowercase().contains("restart"));
    }

    #[test]
    fn external_tool_review_keeps_remembered_conflicts_visible_and_changeable() {
        let mut snapshot = external_tool_review_snapshot();
        let external_candidate_id = snapshot.tools[0].definition.candidate_id();
        snapshot.tool_conflicts[0].candidates[1].candidate_id = external_candidate_id.clone();
        snapshot.tool_conflicts[0].selected_candidate_id = Some(external_candidate_id);

        let summary = external_tool_review_text(Some(&snapshot));
        assert!(summary.contains("Current choices"));
        assert!(summary.contains("OpenCode review [selected, currently unavailable]"));
        assert!(summary.contains("BitFun review [not selected]"));
        assert!(summary.contains("/builtin:tools choose 1 1"));

        snapshot.tools[0].activation = ExternalToolActivationState::Active;
        let active_summary = external_tool_review_text(Some(&snapshot));
        assert!(active_summary.contains("OpenCode review [selected]"));
        assert!(!active_summary.contains("selected, currently unavailable"));

        assert_eq!(
            parse_external_tool_review_action("choose 1 1", Some(&snapshot), None).unwrap(),
            ExternalToolReviewAction::Choose {
                conflict_key: "conflict-v1".to_string(),
                candidate_id: "bitfun:review".to_string(),
            }
        );
        let notice = external_tool_pending_notice_key(&snapshot).unwrap();
        assert!(notice.contains("approval:decision-v1"));
        assert!(notice.contains("opencode.tool.directory_read_failed"));
        assert!(!notice.contains("conflict:conflict-v1"));
    }

    #[test]
    fn external_tool_review_commands_resolve_indices_to_stable_keys() {
        let snapshot = external_tool_review_snapshot();

        assert_eq!(
            parse_external_tool_review_action("enable 2", Some(&snapshot), None).unwrap(),
            ExternalToolReviewAction::Decide {
                approval_key: "approval-v2".to_string(),
                decision_key: "decision-v2".to_string(),
                approved: true,
            }
        );
        assert_eq!(
            parse_external_tool_review_action("disable 3", Some(&snapshot), None).unwrap(),
            ExternalToolReviewAction::Decide {
                approval_key: "approval-v3".to_string(),
                decision_key: "decision-v3".to_string(),
                approved: false,
            }
        );
        assert_eq!(
            parse_external_tool_review_action("disable 4", Some(&snapshot), None).unwrap(),
            ExternalToolReviewAction::Decide {
                approval_key: "approval-v4".to_string(),
                decision_key: "decision-v4".to_string(),
                approved: false,
            }
        );
        assert_eq!(
            parse_external_tool_review_action("choose 1 2", Some(&snapshot), None).unwrap(),
            ExternalToolReviewAction::Choose {
                conflict_key: "conflict-v1".to_string(),
                candidate_id: "external:review".to_string(),
            }
        );
        assert!(parse_external_tool_review_action("enable 3", Some(&snapshot), None).is_err());
    }

    #[test]
    fn external_tool_review_commands_keep_the_indices_from_the_displayed_review() {
        let reviewed = external_tool_review_snapshot();
        let mut current = reviewed.clone();
        current.tools.swap(0, 1);

        assert_eq!(
            parse_external_tool_review_action("enable 2", Some(&current), Some(&reviewed)).unwrap(),
            ExternalToolReviewAction::Decide {
                approval_key: "approval-v2".to_string(),
                decision_key: "decision-v2".to_string(),
                approved: true,
            }
        );
    }

    #[test]
    fn external_tool_enable_result_reports_the_returned_activation() {
        let mut snapshot = external_tool_review_snapshot();
        snapshot.tools[0].activation = ExternalToolActivationState::LoadFailed {
            reason: "module import failed".to_string(),
        };
        let action = ExternalToolReviewAction::Decide {
            approval_key: "approval-v1".to_string(),
            decision_key: "decision-v1".to_string(),
            approved: true,
        };

        assert_eq!(
            external_tool_mutation_result_label(&action, &snapshot),
            "External tool enabled, but loading failed"
        );
    }

    #[test]
    fn external_tool_notice_key_changes_for_pending_decisions_or_diagnostics() {
        let snapshot = external_tool_review_snapshot();
        let key = external_tool_pending_notice_key(&snapshot).unwrap();
        let mut generation_only = snapshot.clone();
        generation_only.generation += 1;
        assert_eq!(
            external_tool_pending_notice_key(&generation_only),
            Some(key.clone())
        );

        generation_only.tool_approval_requests[0].decision_key = "decision-v2".to_string();
        assert_ne!(
            external_tool_pending_notice_key(&generation_only),
            Some(key.clone())
        );

        let mut diagnostic_change = snapshot;
        diagnostic_change.diagnostics[0].message = "different failure".to_string();
        assert_ne!(
            external_tool_pending_notice_key(&diagnostic_change),
            Some(key)
        );
    }

    #[test]
    fn external_tool_mutation_result_does_not_overwrite_a_newer_catalog_generation() {
        let incoming = external_tool_review_snapshot();
        let mut current = incoming.clone();
        current.generation += 1;

        assert!(external_tool_result_is_stale(Some(&current), &incoming));
        assert!(!external_tool_result_is_stale(Some(&incoming), &current));
        assert!(!external_tool_result_is_stale(None, &incoming));
    }

    #[test]
    fn explicit_builtin_never_falls_through_to_an_external_command() {
        let external = external_command("review", None);
        assert_eq!(
            command_route(
                CommandQualifier::Builtin,
                false,
                Some(&external),
                false,
                false,
            ),
            CommandRoute::UnknownBuiltin
        );
    }

    #[test]
    fn command_qualifiers_are_ascii_case_insensitive() {
        assert_eq!(
            parse_command_token("/BUILTIN:help"),
            (CommandQualifier::Builtin, "help")
        );
        assert_eq!(
            parse_command_token("/External:review"),
            (CommandQualifier::External, "review")
        );
    }

    #[test]
    fn hooks_uses_the_existing_native_command_collision_flow() {
        let action =
            crate::actions::action_for_alias("/hooks", crate::actions::ActionContext::Chat)
                .expect("/hooks must be registered");
        assert_eq!(action.id, "hooks");
        let collision = external_command("hooks", None);
        assert_eq!(
            command_route(
                CommandQualifier::Unqualified,
                true,
                Some(&collision),
                false,
                false,
            ),
            CommandRoute::AskForCollisionChoice
        );
        assert_eq!(
            command_route(
                CommandQualifier::External,
                true,
                Some(&collision),
                false,
                false,
            ),
            CommandRoute::External
        );
        let selected_external = external_command("hooks", Some("external:hooks"));
        assert_eq!(
            command_route(
                CommandQualifier::Unqualified,
                true,
                Some(&selected_external),
                false,
                false,
            ),
            CommandRoute::External
        );
        assert!(builtin_hook_help_requested("hooks", "unexpected"));
        assert!(builtin_hook_help_requested("help", "hooks"));
        assert!(!builtin_hook_help_requested("help", "other"));
    }

    #[test]
    fn selected_external_help_keeps_its_hooks_argument() {
        let selected_external = external_command("help", Some("external:help"));
        assert_eq!(
            command_route(
                CommandQualifier::Unqualified,
                true,
                Some(&selected_external),
                false,
                false,
            ),
            CommandRoute::External
        );
    }

    #[test]
    fn hook_catalog_text_is_read_only_redacted_and_explains_native_only_events() {
        let snapshot: ExternalHookCatalogSnapshotV1 = serde_json::from_value(serde_json::json!({
            "schemaVersion": 1,
            "discoveryPending": false,
            "providers": [{
                "providerId": "claude-code.hooks",
                "ecosystemId": "claude-code",
                "displayName": "Claude Code Hooks"
            }],
            "sources": [{
                "key": {"providerId": "claude-code.hooks", "sourceId": "project-settings"},
                "ecosystemId": "claude-code",
                "displayName": "Claude Code project settings",
                "sourceKind": "settings",
                "scope": "project",
                "locationHint": ".claude/settings.json",
                "health": "available",
                "contentVersion": "sha256:source"
            }],
            "entries": [
                {
                    "stableKey": "claude-pre",
                    "source": {"providerId": "claude-code.hooks", "sourceId": "project-settings"},
                    "nativeEvent": "PreToolUse",
                    "matcher": {"kind": "pattern", "display": "Bash|Edit"},
                    "handlerKind": "command",
                    "projectionStatus": "mapped",
                    "nativeActivation": "unknown",
                    "mapping": {"hookPoint": "tool_before"},
                    "contentVersion": "sha256:pre"
                },
                {
                    "stableKey": "claude-session",
                    "source": {"providerId": "claude-code.hooks", "sourceId": "project-settings"},
                    "nativeEvent": "SessionStart",
                    "matcher": {"kind": "any"},
                    "handlerKind": "http",
                    "projectionStatus": "native_only",
                    "nativeActivation": "unknown",
                    "contentVersion": "sha256:session"
                },
                {
                    "stableKey": "claude-opaque",
                    "source": {"providerId": "claude-code.hooks", "sourceId": "project-settings"},
                    "nativeEvent": "<dynamic>",
                    "matcher": {"kind": "dynamic"},
                    "handlerKind": "function",
                    "projectionStatus": "opaque",
                    "nativeActivation": "unknown",
                    "contentVersion": "sha256:opaque"
                }
            ],
            "staleProviderIds": [],
            "diagnostics": []
        }))
        .unwrap();

        let text = render_external_hook_catalog(&snapshot);
        assert!(text.contains("Hooks (read-only)"));
        assert!(text.contains("Claude Code"));
        assert!(text.contains("PreToolUse"));
        assert!(text.contains("coverage mapped: BitFun tool before"));
        assert!(text.contains("SessionStart"));
        assert!(text.contains("native only"));
        assert!(text.contains("opaque static registration"));
        assert!(text.contains("Bash|Edit"));
        assert!(!text.contains("command body"));
    }

    #[test]
    fn hooks_help_uses_the_established_slash_help_pattern() {
        let help = external_hook_help_text();
        assert!(help.contains("Usage: /hooks"));
        assert!(help.contains("/help hooks"));
        assert!(help.contains("/hooks -h"));
        assert!(help.contains("/hooks --help"));
        assert!(!help.contains("/builtin:hooks"));
    }

    #[test]
    fn hook_catalog_text_distinguishes_failed_empty_providers() {
        let snapshot: ExternalHookCatalogSnapshotV1 = serde_json::from_value(serde_json::json!({
            "schemaVersion": 1,
            "discoveryPending": false,
            "providers": [{
                "providerId": "failed.hooks",
                "ecosystemId": "failed",
                "displayName": "Failed Hooks"
            }],
            "sources": [],
            "entries": [],
            "failedProviderIds": ["failed.hooks"],
            "diagnostics": [{
                "severity": "error",
                "assetKind": "hook",
                "code": "failed.hook.read_failed",
                "message": "read failed"
            }]
        }))
        .unwrap();

        let text = render_external_hook_catalog(&snapshot);

        assert!(text.contains("Failed Hooks: 0 Hooks, 0 sources (discovery failed)"));
        assert!(text.contains("No valid catalog is available"));
        assert!(!text.contains("No supported Hook configuration was found"));
    }

    #[test]
    fn hook_catalog_text_does_not_call_a_stale_empty_catalog_successful() {
        let snapshot: ExternalHookCatalogSnapshotV1 = serde_json::from_value(serde_json::json!({
            "schemaVersion": 1,
            "discoveryPending": false,
            "providers": [{
                "providerId": "stale.hooks",
                "ecosystemId": "stale",
                "displayName": "Stale Hooks"
            }],
            "sources": [],
            "entries": [],
            "staleProviderIds": ["stale.hooks"],
            "diagnostics": []
        }))
        .unwrap();

        let text = render_external_hook_catalog(&snapshot);

        assert!(text.contains("The last valid catalog is empty"));
        assert!(!text.contains("No supported Hook configuration was found"));
    }

    #[test]
    fn hook_catalog_text_bounds_large_provider_output() {
        let mut snapshot: ExternalHookCatalogSnapshotV1 =
            serde_json::from_value(serde_json::json!({
                "schemaVersion": 1,
                "discoveryPending": false,
                "providers": [{
                    "providerId": "test.hooks",
                    "ecosystemId": "test",
                    "displayName": "Test Hooks"
                }],
                "sources": [{
                    "key": {"providerId": "test.hooks", "sourceId": "project"},
                    "ecosystemId": "test",
                    "displayName": "Project Hooks",
                    "sourceKind": "settings",
                    "scope": "project",
                    "locationHint": ".test/settings.json",
                    "health": "available",
                    "contentVersion": "source-v1"
                }],
                "entries": [],
                "staleProviderIds": [],
                "diagnostics": []
            }))
            .unwrap();
        snapshot.entries = (0..105)
            .map(
                |index| bitfun_core::external_hooks::ExternalHookCatalogEntry {
                    stable_key: format!("test-{index}"),
                    source: snapshot.sources[0].key.clone(),
                    native_event: format!("Event{index}"),
                    matcher: bitfun_core::external_hooks::ExternalHookMatcherSummary::Any,
                    handler_kind: bitfun_core::external_hooks::ExternalHookHandlerKind::Command,
                    projection_status:
                        bitfun_core::external_hooks::ExternalHookProjectionStatus::NativeOnly,
                    native_activation:
                        bitfun_core::external_hooks::ExternalHookNativeActivation::Unknown,
                    mapping: None,
                    content_version: format!("entry-v{index}"),
                },
            )
            .collect();

        let text = render_external_hook_catalog(&snapshot);

        assert_eq!(text.matches("    - Event").count(), 100);
        assert!(text.contains("omitted 0 source(s), 5 Hook(s)"));
    }

    #[test]
    fn unresolved_provider_conflicts_expose_explicit_cli_choices() {
        let snapshot: ExternalSourceCatalogSnapshot = serde_json::from_value(serde_json::json!({
            "generation": 1,
            "discoveryPending": false,
            "sources": [
                {
                    "stableKey": "first",
                    "record": {
                        "key": { "providerId": "first.commands", "sourceId": "global" },
                        "ecosystemId": "first",
                        "displayName": "First commands",
                        "sourceKind": "prompt_commands",
                        "scope": "user_global",
                        "location": "/first",
                        "executionDomainId": "local-user",
                        "health": "available",
                        "contentVersion": "source-v1"
                    },
                    "lifecycle": "available"
                },
                {
                    "stableKey": "second",
                    "record": {
                        "key": { "providerId": "second.commands", "sourceId": "global" },
                        "ecosystemId": "second",
                        "displayName": "Second commands",
                        "sourceKind": "prompt_commands",
                        "scope": "user_global",
                        "location": "/second",
                        "executionDomainId": "local-user",
                        "health": "available",
                        "contentVersion": "source-v1"
                    },
                    "lifecycle": "available"
                }
            ],
            "commands": [],
            "commandConflicts": [{
                "conflictKey": "provider-conflict-v1",
                "commandName": "review",
                "candidates": [
                    {
                        "candidateId": "first-candidate",
                        "source": { "providerId": "first.commands", "sourceId": "global" },
                        "sourceDisplayName": "First commands",
                        "ecosystemId": "first",
                        "contentVersion": "command-v1",
                        "commandDescription": "First review",
                        "sourceScope": "user_global",
                        "sourceLocation": "/first",
                        "availability": { "state": "available" }
                    },
                    {
                        "candidateId": "second-candidate",
                        "source": { "providerId": "second.commands", "sourceId": "global" },
                        "sourceDisplayName": "Second commands",
                        "ecosystemId": "second",
                        "contentVersion": "command-v1",
                        "commandDescription": "Second review",
                        "sourceScope": "user_global",
                        "sourceLocation": "/second",
                        "availability": { "state": "available" }
                    }
                ]
            }]
        }))
        .unwrap();

        let projections = external_command_projections(&snapshot, &BTreeMap::new());

        assert_eq!(projections.len(), 2);
        assert!(projections.iter().all(|projection| {
            projection.provider_conflict_key.as_deref() == Some("provider-conflict-v1")
        }));
        assert!(projections
            .iter()
            .any(|projection| projection.invocation_alias == "/external:first.commands:review"));
        assert!(projections
            .iter()
            .any(|projection| projection.invocation_alias == "/external:second.commands:review"));
    }

    #[test]
    fn native_collision_requires_one_choice_and_then_reuses_it() {
        let unresolved = external_command("help", None);
        assert_eq!(
            command_route(
                CommandQualifier::Unqualified,
                true,
                Some(&unresolved),
                false,
                false,
            ),
            CommandRoute::AskForCollisionChoice
        );
        let selected = external_command("help", Some("external:help"));
        assert_eq!(
            command_route(
                CommandQualifier::Unqualified,
                true,
                Some(&selected),
                false,
                false,
            ),
            CommandRoute::External
        );
    }

    #[test]
    fn discovery_pending_requires_an_explicit_command_qualifier() {
        assert_eq!(
            command_route(CommandQualifier::Unqualified, true, None, true, false,),
            CommandRoute::WaitForDiscovery
        );
        assert_eq!(
            command_route(CommandQualifier::Builtin, true, None, true, false),
            CommandRoute::Builtin
        );
    }

    #[test]
    fn removed_external_candidate_requires_builtin_reconfirmation() {
        assert_eq!(
            command_route(CommandQualifier::Unqualified, true, None, false, true,),
            CommandRoute::AskForCollisionChoice
        );
        assert_eq!(
            command_route(CommandQualifier::Builtin, true, None, false, true),
            CommandRoute::Builtin
        );
    }

    #[test]
    fn persisted_collision_history_detects_a_removed_external_candidate() {
        let action =
            crate::actions::action_for_alias("/help", crate::actions::ActionContext::Chat).unwrap();
        let mut preferences = ExternalSourceConflictPreferences {
            choices: BTreeMap::new(),
            lineage_current_keys: BTreeMap::new(),
            conflicted_candidate_ids: BTreeSet::from([
                "bitfun.cli:help".to_string(),
                "external:help".to_string(),
            ]),
        };

        let pending = builtin_command_reconfirmation(action.id, action.name, &preferences).unwrap();
        assert!(!pending.confirmed);

        preferences
            .choices
            .insert(pending.conflict_key.clone(), pending.candidate_id.clone());
        let confirmed =
            builtin_command_reconfirmation(action.id, action.name, &preferences).unwrap();
        assert!(confirmed.confirmed);
    }

    #[test]
    fn agent_event_stream_failure_ignores_empty_queue() {
        assert_eq!(agent_event_stream_failure(TryRecvError::Empty), None);
    }

    #[test]
    fn agent_event_stream_failure_treats_lagged_and_closed_as_fatal() {
        let lagged = agent_event_stream_failure(TryRecvError::Lagged(7))
            .expect("lagged stream must be fatal");
        assert!(lagged.contains("lagged by 7 events"));
        assert!(lagged.contains("can no longer be trusted"));

        let closed =
            agent_event_stream_failure(TryRecvError::Closed).expect("closed stream must be fatal");
        assert!(closed.contains("closed"));
        assert!(closed.contains("can no longer be trusted"));
    }

    #[test]
    fn agent_event_stream_failure_marks_active_turn_failed() {
        let mut state = ChatState::new(
            "session".to_string(),
            "Session".to_string(),
            "agentic".to_string(),
            Some("D:/workspace/current".to_string()),
        );
        state.handle_turn_started("turn", "hello");

        assert!(mark_active_turn_failed(
            &mut state,
            "Agent event stream closed; chat state can no longer be trusted"
        ));
        assert_eq!(state.current_turn_id(), None);
        assert!(!state.is_processing);
    }

    #[test]
    fn model_selection_keeps_the_applied_session_model_when_default_persistence_fails() {
        let mut state = ChatState::new(
            "session".to_string(),
            "Session".to_string(),
            "agentic".to_string(),
            Some("D:/workspace/current".to_string()),
        );
        state.current_model_name = "Old model".to_string();

        apply_model_selection_feedback(
            &mut state,
            "New model / Provider",
            "new-model-id",
            ModelSelectionApplyOutcome::Applied {
                default_persist_error: Some("config storage unavailable".to_string()),
            },
        );

        assert_eq!(state.current_model_name, "New model / Provider");
        let notice = state.messages.last().expect("partial-success notice");
        let crate::chat_state::FlowItem::Text { content, .. } = &notice.flow_items[0] else {
            panic!("partial-success notice must be text");
        };
        assert!(content.contains("current session"));
        assert!(content.contains("future sessions"));
    }

    #[test]
    fn model_selection_reports_when_the_current_session_update_fails() {
        let mut state = ChatState::new(
            "session".to_string(),
            "Session".to_string(),
            "agentic".to_string(),
            Some("D:/workspace/current".to_string()),
        );
        state.current_model_name = "Old model".to_string();

        apply_model_selection_feedback(
            &mut state,
            "New model / Provider",
            "new-model-id",
            ModelSelectionApplyOutcome::SessionUpdateFailed("session unavailable".to_string()),
        );

        assert_eq!(state.current_model_name, "Old model");
        let notice = state.messages.last().expect("failure notice");
        let crate::chat_state::FlowItem::Text { content, .. } = &notice.flow_items[0] else {
            panic!("failure notice must be text");
        };
        assert!(content.contains("was not changed"));
        assert!(content.contains("retry"));
    }

    #[test]
    fn mode_selection_commits_visible_state_only_after_runtime_success() {
        let mut current_mode = "agentic".to_string();
        let mut state = ChatState::new(
            "session".to_string(),
            "Session".to_string(),
            "agentic".to_string(),
            Some("D:/workspace/current".to_string()),
        );

        let applied = apply_agent_mode_feedback(
            &mut current_mode,
            &mut state,
            "plan",
            ModeSelectionApplyOutcome::Applied,
        );

        assert!(applied);
        assert_eq!(current_mode, "plan");
        assert_eq!(state.agent_type, "plan");
    }

    #[test]
    fn mode_selection_failure_preserves_visible_state_and_explains_retry() {
        let mut current_mode = "agentic".to_string();
        let mut state = ChatState::new(
            "session".to_string(),
            "Session".to_string(),
            "agentic".to_string(),
            Some("D:/workspace/current".to_string()),
        );

        let applied = apply_agent_mode_feedback(
            &mut current_mode,
            &mut state,
            "plan",
            ModeSelectionApplyOutcome::SessionUpdateFailed(
                "session storage unavailable".to_string(),
            ),
        );

        assert!(!applied);
        assert_eq!(current_mode, "agentic");
        assert_eq!(state.agent_type, "agentic");
        let notice = state.messages.last().expect("failure notice");
        let crate::chat_state::FlowItem::Text { content, .. } = &notice.flow_items[0] else {
            panic!("failure notice must be text");
        };
        assert!(content.contains("was not changed"));
        assert!(content.contains("retry"));
    }

    #[test]
    fn previous_session_mode_failure_is_not_reported_as_a_success() {
        let status = previous_session_mode_change_status(
            "Plan",
            &ModeSelectionApplyOutcome::SessionUpdateFailed("storage unavailable".to_string()),
        );

        assert!(status.contains("failed"));
        assert!(status.contains("storage unavailable"));
        assert!(status.contains("retry"));
    }

    #[test]
    fn pending_mode_change_allows_host_commands_but_blocks_agent_submission() {
        assert!(mode_change_blocks_typed_submission(true, "continue"));
        assert!(!mode_change_blocks_typed_submission(true, "/new"));
        assert!(!mode_change_blocks_typed_submission(true, "/sessions"));
        assert!(!mode_change_blocks_typed_submission(true, "/exit"));
        assert!(!mode_change_blocks_typed_submission(false, "continue"));
    }

    #[test]
    fn failed_mode_save_cancels_automatic_exit() {
        assert!(mode_change_completion_should_exit(true, true));
        assert!(!mode_change_completion_should_exit(true, false));
        assert!(!mode_change_completion_should_exit(false, true));
    }

    #[test]
    fn shortcut_registry_contract_help_uses_resolved_keymap() {
        let keymap = ResolvedKeymap::new(&ShortcutsConfig::default());

        let help = keymap.help_text(ActionState::chat(false, false));
        assert!(help.contains("Ctrl+P"));
        assert!(help.contains("Command Palette"));
    }
    fn external_agent_review_snapshot() -> ExternalSourceCatalogSnapshot {
        serde_json::from_value(serde_json::json!({
            "generation": 9,
            "discoveryPending": false,
            "sources": [],
            "commands": [],
            "subagentGeneration": 4,
            "preferenceRevision": 7,
            "subagents": [{
                "candidateId": "external_subagent:opencode:review:v1",
                "logicalId": "review",
                "displayName": "Review agent",
                "description": "Review a change",
                "providerLabel": "OpenCode",
                "scope": "project",
                "sourceKeys": [{
                    "providerId": "opencode.agents",
                    "sourceId": "project-review"
                }],
                "sourceLocationLabels": ["<workspace>/.opencode/agents/review.md"],
                "sourceCount": 1,
                "effectiveModelLabel": "fast",
                "effectiveToolLabels": ["read", "search"],
                "supportsFollowUp": false,
                "compatibilityState": "ready",
                "diagnostics": [],
                "activationState": { "state": "approval_required" },
                "decisionKey": "decision-v1"
            }],
            "subagentConflicts": [{
                "conflictKey": "conflict-v1",
                "logicalId": "review",
                "candidates": [{
                    "candidateId": "bitfun:review",
                    "displayName": "BitFun review",
                    "sourceLabel": "BitFun",
                    "external": false
                }, {
                    "candidateId": "external_subagent:opencode:review:v1",
                    "displayName": "Review agent",
                    "sourceLabel": "OpenCode",
                    "external": true
                }]
            }],
            "pendingSubagentApprovals": ["external_subagent:opencode:review:v1"]
        }))
        .unwrap()
    }

    #[test]
    fn external_agent_review_is_explicit_single_run_and_does_not_expose_prompt() {
        let summary = external_agent_review_text(Some(&external_agent_review_snapshot()));

        assert!(summary.contains("one run only; no follow-up"));
        assert!(summary.contains("Model: fast"));
        assert!(summary.contains("Tools: read, search"));
        assert!(summary.contains("/builtin:agents enable 1"));
        assert!(summary.contains("/builtin:agents choose 1 2"));
        assert!(summary.contains("/builtin:agents choose 1 0"));
        assert!(summary.contains("Runs on: this computer in the current workspace"));
        assert!(summary.contains("instructions guide the selected model"));
        assert!(summary.contains("may call the tools listed below"));
        assert!(summary.contains("asks again if the instructions, model, tools"));
        assert!(summary.contains("<workspace>/.opencode/agents/review.md"));
        assert!(summary.contains("This choice also confirms"));
        assert!(!summary.contains("D:/repo"));
        assert!(!summary.to_ascii_lowercase().contains("system prompt"));

        let mut unavailable = external_agent_review_snapshot();
        unavailable.subagents[0].effective_model_label = None;
        assert!(external_agent_review_text(Some(&unavailable)).contains("Model: unavailable"));
    }

    #[test]
    fn opening_unified_management_does_not_imply_a_native_command_choice() {
        let tools = crate::actions::action_for_alias("/tools", crate::actions::ActionContext::Chat)
            .unwrap();
        let agents =
            crate::actions::action_for_alias("/agents", crate::actions::ActionContext::Chat)
                .unwrap();
        let help =
            crate::actions::action_for_alias("/help", crate::actions::ActionContext::Chat).unwrap();

        assert!(action_opens_extension_management(tools));
        assert!(action_opens_extension_management(agents));
        assert!(!action_opens_extension_management(help));
    }

    #[test]
    fn agents_management_behavior_change_invalidates_an_old_native_choice() {
        let candidate_id = "bitfun.cli:switch_agent";
        let old_key =
            native_command_conflict_key("local-user", "agents", [(candidate_id, "switch-mode-v1")]);
        let current_key = native_command_conflict_key(
            "local-user",
            "agents",
            [(
                candidate_id,
                action_conflict_behavior_version("switch_agent"),
            )],
        );

        assert_ne!(old_key, current_key);
    }

    #[test]
    fn external_agent_review_keeps_remembered_conflicts_visible_and_changeable() {
        let mut snapshot = external_agent_review_snapshot();
        snapshot.subagent_conflicts[0].selected_candidate_id =
            Some("external_subagent:opencode:review:v1".to_string());
        snapshot.pending_subagent_approvals.clear();

        let summary = external_agent_review_text(Some(&snapshot));
        assert!(summary.contains("Current choices"));
        assert!(
            summary.contains("Review agent (OpenCode, external) [selected, currently unavailable]")
        );
        assert!(summary.contains("BitFun review (BitFun, BitFun/local) [not selected]"));
        assert!(summary.contains("/builtin:agents choose 1 1"));

        snapshot.subagents[0].activation_state = ExternalSubagentActivationState::Active;
        let active_summary = external_agent_review_text(Some(&snapshot));
        assert!(active_summary.contains("Review agent (OpenCode, external) [selected]"));
        assert!(!active_summary.contains("selected, currently unavailable"));

        assert_eq!(
            parse_external_agent_review_action("choose 1 1", Some(&snapshot), None).unwrap(),
            ExternalAgentReviewAction::Choose {
                conflict_key: "conflict-v1".to_string(),
                candidate_id: "bitfun:review".to_string(),
                approve_external: false,
                expected_subagent_generation: 4,
                expected_preference_revision: 7,
            }
        );
        assert_eq!(external_agent_attention(None, &snapshot).conflicts, 0);
    }

    #[test]
    fn external_agent_model_settings_recovery_does_not_require_restart() {
        let lines = external_agent_diagnostic_lines(
            "external_subagent.configuration_unavailable",
            true,
            "",
        );
        let text = lines.join("\n");
        assert!(text.contains("check that BitFun can read and save its settings, then refresh"));
        assert!(!text.to_ascii_lowercase().contains("restart"));
    }

    #[test]
    fn external_agent_review_shows_agent_storage_issues_only_on_the_agent_surface() {
        let mut snapshot = external_agent_review_snapshot();
        snapshot.diagnostics.push(ExternalSourceDiagnostic {
            severity: ExternalSourceDiagnosticSeverity::Warning,
            asset_kind: ExternalSourceAssetKind::Subagent,
            code: "external_subagent.conflict_history_write_failed".to_string(),
            message: "routes remain unavailable".to_string(),
            source: None,
        });
        snapshot.diagnostics.push(ExternalSourceDiagnostic {
            severity: ExternalSourceDiagnosticSeverity::Error,
            asset_kind: ExternalSourceAssetKind::Subagent,
            code: "future_host.agent_map_invalid".to_string(),
            message: "agent map is invalid".to_string(),
            source: None,
        });

        let agents = external_agent_review_text(Some(&snapshot));
        assert!(agents.contains("BitFun could not save conflict information"));
        assert!(agents.contains("check BitFun settings storage, then refresh"));
        assert!(agents.contains("external_subagent.conflict_history_write_failed"));
        assert!(agents.contains("future_host.agent_map_invalid"));

        let tools = external_tool_review_text(Some(&snapshot));
        assert!(!tools.contains("external_subagent.conflict_history_write_failed"));
        assert!(!tools.contains("future_host.agent_map_invalid"));
    }

    #[test]
    fn external_agent_review_actions_bind_generation_revision_and_stable_keys() {
        let snapshot = external_agent_review_snapshot();

        assert_eq!(
            parse_external_agent_review_action("enable 1", Some(&snapshot), None).unwrap(),
            ExternalAgentReviewAction::Decide {
                candidate_id: "external_subagent:opencode:review:v1".to_string(),
                decision_key: "decision-v1".to_string(),
                approved: true,
                expected_subagent_generation: 4,
                expected_preference_revision: 7,
            }
        );
        assert_eq!(
            parse_external_agent_review_action("choose 1 2", Some(&snapshot), None).unwrap(),
            ExternalAgentReviewAction::Choose {
                conflict_key: "conflict-v1".to_string(),
                candidate_id: "external_subagent:opencode:review:v1".to_string(),
                approve_external: true,
                expected_subagent_generation: 4,
                expected_preference_revision: 7,
            }
        );
        assert_eq!(
            parse_external_agent_review_action("choose 1 0", Some(&snapshot), None).unwrap(),
            ExternalAgentReviewAction::Choose {
                conflict_key: "conflict-v1".to_string(),
                candidate_id: "__bitfun_disabled__".to_string(),
                approve_external: false,
                expected_subagent_generation: 4,
                expected_preference_revision: 7,
            }
        );
    }

    #[test]
    fn external_agent_freshness_ignores_unrelated_catalog_generation() {
        let current = external_agent_review_snapshot();
        let mut unrelated_update = current.clone();
        unrelated_update.generation += 1;

        assert!(!external_agent_result_is_stale(
            Some(&unrelated_update),
            &current
        ));

        let mut notice_only_update = unrelated_update.clone();
        notice_only_update.subagent_generation += 1;
        notice_only_update.preference_revision += 1;
        let notice_key = external_agent_pending_notice_key(None, &current);
        assert!(notice_key.is_some());
        assert_eq!(
            external_agent_pending_notice_key(None, &notice_only_update),
            notice_key
        );

        notice_only_update.subagents[0].decision_key = "agent-decision-v2".to_string();
        assert_ne!(
            external_agent_pending_notice_key(None, &notice_only_update),
            notice_key
        );
    }

    #[test]
    fn external_agent_attention_reports_active_agents_that_become_unavailable_or_disappear() {
        let mut previous = external_agent_review_snapshot();
        previous.pending_subagent_approvals.clear();
        previous.subagent_conflicts.clear();
        previous.subagents[0].activation_state = ExternalSubagentActivationState::Active;

        let mut blocked = previous.clone();
        blocked.subagent_generation += 1;
        blocked.subagents[0].activation_state = ExternalSubagentActivationState::Blocked;
        let blocked_attention = external_agent_attention(Some(&previous), &blocked);
        assert_eq!(blocked_attention.unavailable, 1);
        assert!(external_agent_pending_notice_key(Some(&previous), &blocked).is_some());

        let mut removed = previous.clone();
        removed.subagent_generation += 1;
        removed.subagents.clear();
        let removed_attention = external_agent_attention(Some(&previous), &removed);
        assert_eq!(removed_attention.unavailable, 1);
        assert!(external_agent_pending_notice_key(Some(&previous), &removed).is_some());
    }

    #[test]
    fn external_agent_attention_includes_only_agent_warning_and_error_diagnostics() {
        let mut snapshot = external_agent_review_snapshot();
        snapshot.pending_subagent_approvals.clear();
        snapshot.subagent_conflicts.clear();
        snapshot.diagnostics = vec![
            ExternalSourceDiagnostic {
                severity: ExternalSourceDiagnosticSeverity::Warning,
                asset_kind: ExternalSourceAssetKind::Subagent,
                code: "future_host.agent_map_invalid".to_string(),
                message: "agent map is invalid".to_string(),
                source: None,
            },
            ExternalSourceDiagnostic {
                severity: ExternalSourceDiagnosticSeverity::Warning,
                asset_kind: ExternalSourceAssetKind::Tool,
                code: "opencode.tool.directory_read_failed".to_string(),
                message: "tool directory is unavailable".to_string(),
                source: None,
            },
        ];

        let attention = external_agent_attention(None, &snapshot);
        assert_eq!(attention.diagnostics, 1);
        assert!(external_agent_pending_notice_key(None, &snapshot).is_some());
    }

    #[test]
    fn external_agent_result_preserves_newer_unrelated_catalog_partitions() {
        let result = external_agent_review_snapshot();
        let mut current = result.clone();
        current.generation += 1;
        current.commands.clear();
        current.tools.clear();

        let merged = merge_external_agent_mutation_snapshot(Some(&current), result.clone());

        assert_eq!(merged.generation, current.generation);
        assert!(merged.commands.is_empty());
        assert!(merged.tools.is_empty());
        assert_eq!(merged.subagents, result.subagents);
        assert_eq!(merged.subagent_conflicts, result.subagent_conflicts);
        assert_eq!(
            merged.pending_subagent_approvals,
            result.pending_subagent_approvals
        );
    }

    #[test]
    fn external_review_copy_classifies_unknown_locations_and_agent_diagnostics_safely() {
        assert_eq!(external_tool_run_location_label("custom-domain"), "unknown");

        let prompt =
            external_agent_diagnostic_lines("opencode_agent_prompt_not_imported", true, "")
                .join(" ");
        assert!(prompt.contains("does not support"));
        assert!(!prompt.contains("invalid or missing required value"));

        let default_permissions = external_agent_diagnostic_lines(
            "opencode_default_permission_semantics_not_imported",
            false,
            "",
        )
        .join(" ");
        assert!(default_permissions.contains("does not use this setting"));
        assert!(!default_permissions.contains("cannot be enabled"));

        let invalid =
            external_agent_diagnostic_lines("opencode_agent_definition_type_invalid", true, "")
                .join(" ");
        assert!(invalid.contains("invalid or missing required value"));

        let config = external_agent_diagnostic_lines(
            "external_subagent.configuration_unavailable",
            true,
            "",
        )
        .join(" ");
        assert!(config.contains("could not read its model settings"));
        assert!(config.contains("can read and save its settings"));
        assert!(!config.contains("requested model is not available"));
    }
}
