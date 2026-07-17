#[cfg(test)]
mod tests {
    use tokio::sync::broadcast::error::TryRecvError;

    use super::{
        agent_event_stream_failure, builtin_command_reconfirmation, command_route,
        external_command_projections, external_tool_mutation_result_label,
        external_tool_pending_notice_key, external_tool_result_is_stale, external_tool_review_text,
        apply_model_selection_feedback, mark_active_turn_failed, parse_command_token,
        parse_external_tool_review_action, CommandQualifier, CommandRoute,
        ExternalSourceConflictPreferences, ExternalToolReviewAction, ModelSelectionApplyOutcome,
    };
    use crate::actions::{ActionState, ResolvedKeymap};
    use crate::chat_state::ChatState;
    use crate::config::ShortcutsConfig;
    use crate::ui::command_menu::{ExternalCommandProjection, NativeCommandCollisionProjection};
    use bitfun_core::external_sources::{
        ExternalSourceCatalogSnapshot, ExternalToolActivationState,
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
                    "location": "D:/repo/.opencode/tools",
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
                    "modulePath": "D:/repo/.opencode/tools/review.js",
                    "workingDirectory": "D:/repo",
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
                    "modulePath": "D:/repo/.opencode/tools/weather.js",
                    "workingDirectory": "D:/repo",
                    "runtimeKind": "java_script",
                    "capabilities": ["network"],
                    "contentVersion": "content-v1",
                    "staticStatus": { "state": "ready" }
                },
                "approvalKey": "approval-v2",
                "decisionKey": "decision-v2",
                "activation": { "state": "disabled" }
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
                    "modulePath": "D:/repo/.opencode/tools/deploy.js",
                    "workingDirectory": "D:/repo",
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
                    "modulePath": "D:/repo/.opencode/tools/broken.ts",
                    "workingDirectory": "D:/repo",
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
                "sourceLocation": "D:/repo/.opencode/tools/review.js",
                "workingDirectory": "D:/repo",
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
                    "sourceLocation": "D:/repo/.opencode/tools/review.js"
                }]
            }],
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
    fn external_tool_review_summary_discloses_execution_boundary_and_commands() {
        let summary = external_tool_review_text(Some(&external_tool_review_snapshot()));

        assert!(summary.contains("No external code ran during discovery"));
        assert!(summary.contains("filesystem, network, process, environment variables"));
        assert!(summary.contains("inherited environment variables"));
        assert!(summary.contains("full descendant-process cleanup"));
        assert!(summary.contains("/external-tools enable 1"));
        assert!(summary.contains("/external-tools choose 1 2"));
        assert!(summary.contains("D:/repo/.opencode/tools/review.js"));
        assert!(summary.contains("Source root: D:/repo/.opencode/tools"));
        assert!(summary.contains("Scope: project"));
        assert!(summary.contains("Execution domain: local:D:/repo"));
        assert!(summary.contains("disabled"));
        assert!(summary.contains("active"));
        assert!(summary.contains("loaded successfully"));
        assert!(summary.contains("load failed"));
        assert!(summary.contains("D:/repo/.opencode/tools/broken.ts"));
        assert!(summary.contains("Diagnostics"));
        assert!(summary.contains("opencode.tool.directory_read_failed"));
        assert!(!summary.contains("PR2"));
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
            "External tool approved, but loading failed"
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
    fn shortcut_registry_contract_help_uses_resolved_keymap() {
        let keymap = ResolvedKeymap::new(&ShortcutsConfig::default());

        let help = keymap.help_text(ActionState::chat(false, false));
        assert!(help.contains("Ctrl+P"));
        assert!(help.contains("Command Palette"));
    }
}
