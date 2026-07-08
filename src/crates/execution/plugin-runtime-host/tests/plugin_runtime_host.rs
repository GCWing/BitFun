use async_trait::async_trait;
use bitfun_plugin_runtime_host::{PluginHostAdapter, PluginRuntimeHost};
use bitfun_runtime_ports::{
    PermissionPromptDenyState, PermissionPromptDescriptor, PermissionPromptEffectKind,
    PluginAuditRef, PluginCapabilityRef, PluginDataClassification, PluginDispatchEnvelope,
    PluginEffectCandidate, PluginEffectCandidatePayload, PluginManifestRef, PluginOwnerKind,
    PluginOwnerRef, PluginPermissionGate, PluginQuarantineReason, PluginQuarantineScope,
    PluginQuarantineState, PluginResponseEnvelope, PluginRiskLevel, PluginRollbackMode,
    PluginRollbackPolicy, PluginRuntimeAvailability, PluginRuntimeClient, PluginRuntimeEpochs,
    PluginRuntimeReadRequest, PluginRuntimeReadResponse, PluginRuntimeUnavailableReason,
    PluginSourceKind, PluginSourceRef, PluginStatusKind, PluginStatusSnapshot, PluginTargetRef,
    PluginTrustLevel, PortError, PortErrorKind, PortResult,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Default)]
struct CountingAdapter {
    calls: Mutex<Vec<String>>,
    statuses: Mutex<Vec<ScopedStatus>>,
    fail: bool,
    invalid_project_response: bool,
    invalid_workspace_read_response: bool,
    invalid_permission_prompt: bool,
    invalid_permission_authority: bool,
    final_policy_decision: bool,
    wrong_adapter_id: bool,
    success_quarantine_with_effects: bool,
    status_quarantine_with_effects: bool,
    delay_ms: u64,
}

#[derive(Clone)]
struct ScopedStatus {
    project_domain_id: String,
    workspace_id: String,
    status: PluginStatusSnapshot,
}

struct CrossKeyQuarantineRaceAdapter {
    calls: Mutex<Vec<String>>,
    fail_started: AtomicBool,
}

impl Default for CrossKeyQuarantineRaceAdapter {
    fn default() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            fail_started: AtomicBool::new(false),
        }
    }
}

#[async_trait]
impl PluginHostAdapter for CrossKeyQuarantineRaceAdapter {
    fn adapter_id(&self) -> &str {
        "opencode-compatible"
    }

    async fn read_plugins(
        &self,
        request: PluginRuntimeReadRequest,
    ) -> PortResult<PluginRuntimeReadResponse> {
        Ok(PluginRuntimeReadResponse {
            request_id: request.request_id,
            project_domain_id: request.project_domain_id,
            workspace_id: request.workspace_id,
            sources: Vec::new(),
            plugin_statuses: Vec::new(),
            diagnostics: Vec::new(),
            observed_epochs: request.epochs,
        })
    }

    async fn dispatch(
        &self,
        envelope: PluginDispatchEnvelope,
    ) -> PortResult<PluginResponseEnvelope> {
        self.calls.lock().unwrap().push(envelope.event_id.clone());
        if envelope.event_id == "event-cross-quarantine-fail" {
            self.fail_started.store(true, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(50)).await;
            return Err(PortError::new(
                PortErrorKind::Backend,
                "adapter protocol failure",
            ));
        }

        Ok(PluginResponseEnvelope {
            envelope_version: envelope.envelope_version,
            request_event_id: envelope.event_id.clone(),
            project_domain_id: envelope.project_domain_id.clone(),
            workspace_id: envelope.workspace_id.clone(),
            adapter_id: "opencode-compatible".to_string(),
            plugin_id: Some(envelope.source.plugin_id.clone()),
            completed_at_ms: 1_720_000_100,
            effects: vec![PluginEffectCandidate {
                effect_id: "effect-1".to_string(),
                schema_version: "plugin.effect.v1".to_string(),
                declared_capability: envelope.declared_capability.clone(),
                target_ref: target_ref(),
                data_classification: PluginDataClassification::Workspace,
                risk_level: PluginRiskLevel::Medium,
                permission: PluginPermissionGate::PermissionRequired {
                    prompt: permission_prompt(&envelope, false, false),
                },
                source_ref: envelope.source.clone(),
                payload: PluginEffectCandidatePayload::ProviderCandidate {
                    provider_id: "opencode.example.provider".to_string(),
                    tool_contract_id: "tool-provider.v1".to_string(),
                },
            }],
            diagnostics: Vec::new(),
            quarantine: None,
            plugin_statuses: Vec::new(),
            observed_epochs: envelope.epochs,
        })
    }
}

#[async_trait]
impl PluginHostAdapter for CountingAdapter {
    fn adapter_id(&self) -> &str {
        "opencode-compatible"
    }

    async fn read_plugins(
        &self,
        request: PluginRuntimeReadRequest,
    ) -> PortResult<PluginRuntimeReadResponse> {
        let statuses = self
            .statuses
            .lock()
            .unwrap()
            .iter()
            .filter(|status| {
                status.project_domain_id == request.project_domain_id
                    && status.workspace_id == request.workspace_id
                    && (request.plugin_ids.is_empty()
                        || request.plugin_ids.contains(&status.status.source.plugin_id))
            })
            .map(|status| status.status.clone())
            .collect::<Vec<_>>();
        let sources = statuses
            .iter()
            .map(|status| status.source.clone())
            .collect::<Vec<_>>();

        Ok(PluginRuntimeReadResponse {
            request_id: request.request_id,
            project_domain_id: request.project_domain_id,
            workspace_id: if self.invalid_workspace_read_response {
                "other-workspace".to_string()
            } else {
                request.workspace_id
            },
            sources,
            plugin_statuses: statuses,
            diagnostics: Vec::new(),
            observed_epochs: request.epochs,
        })
    }

    async fn dispatch(
        &self,
        envelope: PluginDispatchEnvelope,
    ) -> PortResult<PluginResponseEnvelope> {
        self.calls
            .lock()
            .unwrap()
            .push(envelope.idempotency_key.clone());
        if self.delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
        }
        if self.fail {
            return Err(PortError::new(
                PortErrorKind::Backend,
                "adapter protocol failure",
            ));
        }

        let mut response = PluginResponseEnvelope {
            envelope_version: envelope.envelope_version,
            request_event_id: envelope.event_id.clone(),
            project_domain_id: envelope.project_domain_id.clone(),
            workspace_id: envelope.workspace_id.clone(),
            adapter_id: if self.wrong_adapter_id {
                "other-adapter".to_string()
            } else {
                "opencode-compatible".to_string()
            },
            plugin_id: Some(envelope.source.plugin_id.clone()),
            completed_at_ms: 1_720_000_100,
            effects: vec![PluginEffectCandidate {
                effect_id: "effect-1".to_string(),
                schema_version: "plugin.effect.v1".to_string(),
                declared_capability: envelope.declared_capability.clone(),
                target_ref: target_ref(),
                data_classification: PluginDataClassification::Workspace,
                risk_level: PluginRiskLevel::Medium,
                permission: PluginPermissionGate::PermissionRequired {
                    prompt: permission_prompt(
                        &envelope,
                        self.invalid_permission_prompt,
                        self.invalid_permission_authority,
                    ),
                },
                source_ref: envelope.source.clone(),
                payload: PluginEffectCandidatePayload::ProviderCandidate {
                    provider_id: "opencode.example.provider".to_string(),
                    tool_contract_id: "tool-provider.v1".to_string(),
                },
            }],
            diagnostics: Vec::new(),
            quarantine: None,
            plugin_statuses: Vec::new(),
            observed_epochs: envelope.epochs,
        };
        if self.final_policy_decision {
            response.effects[0].permission = PluginPermissionGate::PolicyAllowed {
                audit: audit_ref_for_event("corr-1", &response.request_event_id),
            };
        }
        if self.invalid_project_response {
            response.project_domain_id = "other-project".to_string();
        }
        if self.success_quarantine_with_effects {
            response.quarantine = Some(PluginQuarantineState {
                schema_version: 1,
                quarantine_id: "adapter-success-quarantine".to_string(),
                scope: PluginQuarantineScope::ProjectPlugin {
                    project_domain_id: envelope.project_domain_id,
                    workspace_id: envelope.workspace_id,
                    plugin_id: envelope.source.plugin_id.clone(),
                },
                reason: PluginQuarantineReason::AdapterFailure,
                source: envelope.source.clone(),
                audit: audit_ref_for_event("corr-1", &response.request_event_id),
                created_at_ms: 1_720_000_100,
                log_ref: None,
                clears_when: Vec::new(),
                diagnostic_ids: Vec::new(),
            });
        }
        if self.status_quarantine_with_effects {
            response.plugin_statuses.push(PluginStatusSnapshot {
                source: source_ref(),
                status: PluginStatusKind::Quarantined,
                availability: PluginRuntimeAvailability::Available,
                config_validation: None,
                quarantine: Some(PluginQuarantineState {
                    schema_version: 1,
                    quarantine_id: "status-quarantine".to_string(),
                    scope: PluginQuarantineScope::ProjectPlugin {
                        project_domain_id: "project-1".to_string(),
                        workspace_id: "workspace-1".to_string(),
                        plugin_id: "opencode.example".to_string(),
                    },
                    reason: PluginQuarantineReason::AdapterFailure,
                    source: source_ref(),
                    audit: audit_ref_for_event("corr-1", &response.request_event_id),
                    created_at_ms: 1_720_000_100,
                    log_ref: None,
                    clears_when: Vec::new(),
                    diagnostic_ids: Vec::new(),
                }),
                diagnostic_ids: Vec::new(),
                updated_at_ms: 1_720_000_100,
            });
        }
        Ok(response)
    }
}

fn manifest_ref() -> PluginManifestRef {
    PluginManifestRef {
        manifest_id: "manifest-1".to_string(),
        schema_version: "opencode.plugin.v1".to_string(),
        path: Some("opencode.json".to_string()),
    }
}

fn source_ref() -> PluginSourceRef {
    PluginSourceRef {
        plugin_id: "opencode.example".to_string(),
        source_kind: PluginSourceKind::OpenCodeCompatible,
        source: "file:///plugins/opencode-example".to_string(),
        version: Some("1.2.3".to_string()),
        content_hash: "sha256:abc123".to_string(),
        trust_level: PluginTrustLevel::Trusted,
        manifest: Some(manifest_ref()),
    }
}

fn owner_ref() -> PluginOwnerRef {
    PluginOwnerRef {
        kind: PluginOwnerKind::ExtensionContract,
        id: "extension.tool-provider".to_string(),
    }
}

fn capability_ref() -> PluginCapabilityRef {
    PluginCapabilityRef {
        capability_id: "tools.provider".to_string(),
        owner: owner_ref(),
    }
}

fn target_ref() -> PluginTargetRef {
    PluginTargetRef {
        target_kind: "tool_provider".to_string(),
        target_id: "opencode.example.provider".to_string(),
        display_name: "OpenCode example provider".to_string(),
        artifact: None,
    }
}

fn audit_ref_for(envelope: &PluginDispatchEnvelope) -> PluginAuditRef {
    audit_ref_for_event(&envelope.correlation_id, &envelope.event_id)
}

fn audit_ref_for_event(correlation_id: &str, event_id: &str) -> PluginAuditRef {
    PluginAuditRef {
        correlation_id: correlation_id.to_string(),
        event_id: Some(event_id.to_string()),
    }
}

fn permission_prompt(
    envelope: &PluginDispatchEnvelope,
    invalid_prompt: bool,
    invalid_authority: bool,
) -> PermissionPromptDescriptor {
    PermissionPromptDescriptor {
        descriptor_version: 1,
        prompt_id: "prompt-1".to_string(),
        plugin: envelope.source.clone(),
        requested_capability: envelope.declared_capability.clone(),
        requested_effect: PermissionPromptEffectKind::ProviderCandidate,
        target: if invalid_prompt {
            PluginTargetRef {
                target_kind: "tool_provider".to_string(),
                target_id: "other.provider".to_string(),
                display_name: "Other provider".to_string(),
                artifact: None,
            }
        } else {
            target_ref()
        },
        risk_level: if invalid_prompt {
            PluginRiskLevel::Low
        } else {
            PluginRiskLevel::Medium
        },
        owner: if invalid_authority {
            PluginOwnerRef {
                kind: PluginOwnerKind::ProductFeature,
                id: "spoofed-tools-owner".to_string(),
            }
        } else {
            PluginOwnerRef {
                kind: envelope.declared_capability.owner.kind,
                id: envelope.declared_capability.owner.id.clone(),
            }
        },
        rollback: PluginRollbackPolicy {
            mode: PluginRollbackMode::DisablePlugin,
            reason_ref: Some(if invalid_authority {
                "audit:spoofed-event".to_string()
            } else {
                format!("audit:{}", envelope.event_id)
            }),
        },
        deny_state: if invalid_authority {
            PermissionPromptDenyState::PolicyDenied
        } else {
            PermissionPromptDenyState::CandidateDiscarded
        },
        audit: audit_ref_for(envelope),
    }
}

fn epochs() -> PluginRuntimeEpochs {
    PluginRuntimeEpochs {
        project_epoch: 7,
        trust_epoch: 3,
        policy_epoch: 5,
        tool_registry_epoch: Some(11),
    }
}

fn envelope(id: &str) -> PluginDispatchEnvelope {
    PluginDispatchEnvelope {
        envelope_version: 1,
        event_id: id.to_string(),
        event_type: "agent.turn.completed".to_string(),
        event_version: "2026-07-08".to_string(),
        project_domain_id: "project-1".to_string(),
        workspace_id: "workspace-1".to_string(),
        extension_point_id: "command.palette".to_string(),
        source: source_ref(),
        declared_capability: capability_ref(),
        correlation_id: "corr-1".to_string(),
        causation_id: None,
        idempotency_key: format!("{id}:command.palette"),
        deadline_ms: 30_000,
        epochs: epochs(),
        payload_ref: None,
    }
}

fn scoped_status(
    project_domain_id: &str,
    workspace_id: &str,
    source: PluginSourceRef,
) -> ScopedStatus {
    scoped_status_with_quarantine(project_domain_id, workspace_id, source, None)
}

fn scoped_status_with_quarantine(
    project_domain_id: &str,
    workspace_id: &str,
    source: PluginSourceRef,
    quarantine: Option<PluginQuarantineState>,
) -> ScopedStatus {
    let has_quarantine = quarantine.is_some();
    ScopedStatus {
        project_domain_id: project_domain_id.to_string(),
        workspace_id: workspace_id.to_string(),
        status: PluginStatusSnapshot {
            source,
            status: if has_quarantine {
                PluginStatusKind::Quarantined
            } else {
                PluginStatusKind::Enabled
            },
            availability: if has_quarantine {
                PluginRuntimeAvailability::ProjectionOnly {
                    reason: PluginRuntimeUnavailableReason::HostUnavailable,
                }
            } else {
                PluginRuntimeAvailability::Available
            },
            config_validation: None,
            quarantine,
            diagnostic_ids: Vec::new(),
            updated_at_ms: 1_720_000_123,
        },
    }
}

#[tokio::test]
async fn host_dispatches_candidates() {
    let adapter = Arc::new(CountingAdapter::default());
    let host = PluginRuntimeHost::new(adapter.clone());

    let response = host
        .dispatch(envelope("event-1"))
        .await
        .expect("host dispatch should return adapter candidates");

    assert_eq!(response.effects.len(), 1);
    assert_eq!(response.adapter_id, "opencode-compatible");
    assert_eq!(adapter.calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn host_replays_idempotent_dispatch_without_recalling_adapter() {
    let adapter = Arc::new(CountingAdapter::default());
    let host = PluginRuntimeHost::new(adapter.clone());
    let request = envelope("event-2");

    let first = host
        .dispatch(request.clone())
        .await
        .expect("first dispatch");
    let second = host.dispatch(request).await.expect("idempotent replay");

    assert_eq!(first.request_event_id, second.request_event_id);
    assert_eq!(first.effects[0].effect_id, second.effects[0].effect_id);
    assert_eq!(
        adapter.calls.lock().unwrap().len(),
        1,
        "idempotent replay must not call the adapter twice"
    );
}

#[tokio::test]
async fn idempotent_dispatch_cache_evicts_old_entries() {
    let adapter = Arc::new(CountingAdapter::default());
    let host = PluginRuntimeHost::new(adapter.clone());

    let first = envelope("event-cache-evict-0");
    host.dispatch(first.clone()).await.expect("first dispatch");
    for index in 1..=300 {
        host.dispatch(envelope(&format!("event-cache-evict-{index}")))
            .await
            .expect("bounded cache fill dispatch");
    }

    host.dispatch(first)
        .await
        .expect("evicted dispatch should call adapter again");

    assert_eq!(
        adapter.calls.lock().unwrap().len(),
        302,
        "old idempotency entries must be evicted instead of growing without bound"
    );
}

#[tokio::test]
async fn concurrent_idempotent_dispatch_reuses_in_flight_response() {
    let adapter = Arc::new(CountingAdapter {
        delay_ms: 50,
        ..Default::default()
    });
    let host = PluginRuntimeHost::new(adapter.clone());
    let request = envelope("event-concurrent-idempotent");

    let (first, second) = tokio::join!(host.dispatch(request.clone()), host.dispatch(request));
    let first = first.expect("first dispatch");
    let second = second.expect("second dispatch");

    assert_eq!(first.request_event_id, second.request_event_id);
    assert_eq!(first.effects[0].effect_id, second.effects[0].effect_id);
    assert_eq!(
        adapter.calls.lock().unwrap().len(),
        1,
        "concurrent idempotent dispatch must not call the adapter twice"
    );
}

#[tokio::test]
async fn concurrent_cross_key_dispatch_observes_active_quarantine_before_success() {
    let adapter = Arc::new(CrossKeyQuarantineRaceAdapter::default());
    let host = PluginRuntimeHost::new(adapter.clone());
    let failing_host = host.clone();

    let failing_dispatch = tokio::spawn(async move {
        failing_host
            .dispatch(envelope("event-cross-quarantine-fail"))
            .await
    });

    while !adapter.fail_started.load(Ordering::SeqCst) {
        tokio::time::sleep(Duration::from_millis(1)).await;
    }

    let blocked = host
        .dispatch(envelope("event-cross-quarantine-success"))
        .await
        .expect_err("active quarantine must block concurrent cross-key success");

    let failing_response = failing_dispatch
        .await
        .expect("failing dispatch task should finish")
        .expect("adapter failure should become typed quarantine response");

    assert_eq!(
        failing_response
            .quarantine
            .as_ref()
            .expect("quarantine")
            .reason,
        PluginQuarantineReason::AdapterFailure
    );
    assert_eq!(blocked.kind, PortErrorKind::NotAvailable);
    assert!(blocked.message.contains("active quarantine"));
    let calls = adapter.calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        1,
        "same-plugin concurrent dispatches with different keys must serialize behind quarantine"
    );
    assert_eq!(calls[0], "event-cross-quarantine-fail");
}

#[tokio::test]
async fn idempotent_dispatch_cache_does_not_replay_across_events() {
    let adapter = Arc::new(CountingAdapter::default());
    let host = PluginRuntimeHost::new(adapter.clone());
    let mut first = envelope("event-cache-1");
    first.idempotency_key = "shared-event-idempotency".to_string();
    let mut second = first.clone();
    second.event_id = "event-cache-2".to_string();
    second.event_type = "agent.turn.failed".to_string();
    second.event_version = "2026-07-08".to_string();
    second.correlation_id = "corr-2".to_string();
    second.causation_id = Some("event-cache-1".to_string());

    let first_response = host.dispatch(first).await.expect("first dispatch");
    let second_response = host.dispatch(second).await.expect("second dispatch");

    assert_eq!(first_response.request_event_id, "event-cache-1");
    assert_eq!(second_response.request_event_id, "event-cache-2");
    assert_eq!(
        adapter.calls.lock().unwrap().len(),
        2,
        "same idempotency key on a different canonical event must not replay stale response"
    );
    let PluginPermissionGate::PermissionRequired { prompt } =
        &second_response.effects[0].permission
    else {
        panic!("second response should still be permission gated");
    };
    assert_eq!(prompt.audit.correlation_id, "corr-2");
    assert_eq!(prompt.audit.event_id.as_deref(), Some("event-cache-2"));
}

#[tokio::test]
async fn zero_deadline_quarantines_without_adapter_dispatch() {
    let adapter = Arc::new(CountingAdapter::default());
    let host = PluginRuntimeHost::new(adapter.clone());
    let mut request = envelope("event-3");
    request.deadline_ms = 0;

    let response = host
        .dispatch(request)
        .await
        .expect("deadline failure should be a typed host response");

    assert!(response.effects.is_empty());
    assert_eq!(
        response.diagnostics[0].code,
        "plugin_host.deadline_exceeded"
    );
    assert_eq!(
        response.quarantine.as_ref().expect("quarantine").reason,
        PluginQuarantineReason::DeadlineExceeded
    );
    let quarantine_id = &response
        .quarantine
        .as_ref()
        .expect("quarantine")
        .quarantine_id;
    assert!(quarantine_id.starts_with("quarantine:"));
    assert!(
        !quarantine_id.contains("file:///"),
        "public quarantine id must not leak source paths"
    );
    assert!(adapter.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn malformed_dispatch_envelope_quarantines_without_adapter_dispatch() {
    let adapter = Arc::new(CountingAdapter::default());
    let host = PluginRuntimeHost::new(adapter.clone());
    let mut request = envelope("event-empty-idempotency");
    request.idempotency_key.clear();

    let response = host
        .dispatch(request)
        .await
        .expect("invalid dispatch envelope should become a typed host response");

    assert!(response.effects.is_empty());
    assert_eq!(
        response.diagnostics[0].code,
        "plugin_host.invalid_dispatch_envelope"
    );
    assert!(response.diagnostics[0]
        .message
        .contains("idempotency_key is empty"));
    assert_eq!(
        response.quarantine.as_ref().expect("quarantine").reason,
        PluginQuarantineReason::HostFailure
    );
    assert!(adapter.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn malformed_dispatch_with_missing_identity_observes_active_quarantine() {
    let adapter = Arc::new(CountingAdapter::default());
    let host = PluginRuntimeHost::new(adapter.clone());
    let mut first = envelope("event-missing-plugin-1");
    first.source.plugin_id.clear();

    let response = host
        .dispatch(first)
        .await
        .expect("missing plugin identity should become a typed host response");
    let quarantine_id = response
        .quarantine
        .as_ref()
        .expect("quarantine")
        .quarantine_id
        .clone();

    let mut second = envelope("event-missing-plugin-2");
    second.source.plugin_id.clear();
    let error = host
        .dispatch(second)
        .await
        .expect_err("missing identity must not bypass active host quarantine");

    assert_eq!(error.kind, PortErrorKind::NotAvailable);
    assert!(error.message.contains(&quarantine_id));
    assert!(
        adapter.calls.lock().unwrap().is_empty(),
        "malformed dispatches with missing identity must not reach adapter"
    );
}

#[tokio::test]
async fn adapter_failure_quarantines_without_writing_success() {
    let adapter = Arc::new(CountingAdapter {
        fail: true,
        ..Default::default()
    });
    let host = PluginRuntimeHost::new(adapter);

    let response = host
        .dispatch(envelope("event-4"))
        .await
        .expect("adapter failures should become typed diagnostics");

    assert!(response.effects.is_empty());
    assert_eq!(response.diagnostics[0].code, "plugin_host.adapter_failure");
    assert_eq!(
        response.quarantine.as_ref().expect("quarantine").reason,
        PluginQuarantineReason::AdapterFailure
    );
}

#[tokio::test]
async fn active_quarantine_blocks_new_dispatches_until_host_restart() {
    let adapter = Arc::new(CountingAdapter {
        fail: true,
        ..Default::default()
    });
    let host = PluginRuntimeHost::new(adapter.clone());

    let first = host
        .dispatch(envelope("event-active-quarantine-1"))
        .await
        .expect("adapter failure should quarantine the plugin");
    assert_eq!(
        first.quarantine.as_ref().expect("quarantine").reason,
        PluginQuarantineReason::AdapterFailure
    );
    assert_eq!(adapter.calls.lock().unwrap().len(), 1);

    let replay = host
        .dispatch(envelope("event-active-quarantine-1"))
        .await
        .expect("same idempotency key should replay the original quarantine response");
    assert_eq!(
        replay.quarantine.as_ref().expect("quarantine").reason,
        PluginQuarantineReason::AdapterFailure
    );
    assert_eq!(
        adapter.calls.lock().unwrap().len(),
        1,
        "quarantine replay must not call the adapter again"
    );

    let error = host
        .dispatch(envelope("event-active-quarantine-2"))
        .await
        .expect_err("active quarantine must block follow-up dispatch");

    assert_eq!(error.kind, PortErrorKind::NotAvailable);
    assert!(error.message.contains("active quarantine"));
    assert_eq!(
        adapter.calls.lock().unwrap().len(),
        1,
        "quarantined plugins must not keep producing candidate effects"
    );
}

#[tokio::test]
async fn active_quarantine_blocks_malformed_follow_up_without_new_quarantine() {
    let adapter = Arc::new(CountingAdapter {
        fail: true,
        ..Default::default()
    });
    let host = PluginRuntimeHost::new(adapter.clone());

    let first = host
        .dispatch(envelope("event-active-quarantine-malformed-1"))
        .await
        .expect("adapter failure should quarantine the plugin");
    let quarantine_id = first
        .quarantine
        .as_ref()
        .expect("quarantine")
        .quarantine_id
        .clone();

    let mut malformed = envelope("event-active-quarantine-malformed-2");
    malformed.idempotency_key.clear();
    let error = host
        .dispatch(malformed)
        .await
        .expect_err("active quarantine must block malformed follow-up dispatch");

    assert_eq!(error.kind, PortErrorKind::NotAvailable);
    assert!(error.message.contains(&quarantine_id));
    assert_eq!(
        adapter.calls.lock().unwrap().len(),
        1,
        "malformed follow-up must not call the adapter or create a new adapter failure"
    );

    let read = host
        .read_plugins(PluginRuntimeReadRequest {
            request_id: "read-active-quarantine-malformed".to_string(),
            project_domain_id: "project-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            plugin_ids: vec!["opencode.example".to_string()],
            include_config_validation: true,
            epochs: epochs(),
        })
        .await
        .expect("active quarantine read model");
    assert_eq!(
        read.plugin_statuses[0]
            .quarantine
            .as_ref()
            .expect("quarantine")
            .quarantine_id,
        quarantine_id
    );
    assert_eq!(read.diagnostics.len(), 1);
}

#[tokio::test]
async fn host_owned_quarantine_is_visible_in_read_model_with_diagnostics() {
    let adapter = Arc::new(CountingAdapter::default());
    let host = PluginRuntimeHost::new(adapter);
    let mut request = envelope("event-8");
    request.deadline_ms = 0;
    let response = host.dispatch(request).await.expect("deadline quarantine");
    let quarantine = response.quarantine.expect("quarantine");

    let read = host
        .read_plugins(PluginRuntimeReadRequest {
            request_id: "read-host-quarantine".to_string(),
            project_domain_id: "project-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            plugin_ids: vec!["opencode.example".to_string()],
            include_config_validation: true,
            epochs: epochs(),
        })
        .await
        .expect("host-owned quarantine should be projected");
    assert_eq!(read.sources[0].plugin_id, "opencode.example");
    assert_eq!(
        read.plugin_statuses[0].status,
        PluginStatusKind::Quarantined
    );
    assert_eq!(
        read.plugin_statuses[0].availability,
        PluginRuntimeAvailability::ProjectionOnly {
            reason: PluginRuntimeUnavailableReason::HostUnavailable
        }
    );
    assert!(!read.plugin_statuses[0].availability.is_executable());
    assert_eq!(
        read.plugin_statuses[0]
            .quarantine
            .as_ref()
            .expect("read quarantine")
            .quarantine_id,
        quarantine.quarantine_id
    );
    assert_eq!(read.diagnostics.len(), 1);
    assert_eq!(
        read.diagnostics[0].diagnostic_id,
        read.plugin_statuses[0].diagnostic_ids[0]
    );
    assert_eq!(read.diagnostics[0].code, "plugin_host.deadline_exceeded");
}

#[tokio::test]
async fn host_restart_clears_domain_quarantine_and_cached_dispatch() {
    let adapter = Arc::new(CountingAdapter::default());
    let host = PluginRuntimeHost::new(adapter.clone());
    let mut request = envelope("event-restart-clear");
    request.deadline_ms = 0;
    let quarantine_response = host
        .dispatch(request)
        .await
        .expect("deadline failure should quarantine");
    assert!(quarantine_response.quarantine.is_some());

    host.restart("project-1", "workspace-1");

    let read = host
        .read_plugins(PluginRuntimeReadRequest {
            request_id: "read-restarted-host".to_string(),
            project_domain_id: "project-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            plugin_ids: vec!["opencode.example".to_string()],
            include_config_validation: true,
            epochs: epochs(),
        })
        .await
        .expect("restart should leave read model available");
    assert!(
        read.plugin_statuses.is_empty(),
        "HostRestarted must clear host-owned quarantine projection"
    );

    let retry = host
        .dispatch(envelope("event-restart-clear"))
        .await
        .expect("restart must clear cached quarantine response");
    assert_eq!(retry.effects.len(), 1);
    assert_eq!(
        adapter.calls.lock().unwrap().len(),
        1,
        "retry after restart should reach adapter instead of replaying cached quarantine"
    );
}

#[tokio::test]
async fn disposed_project_rejects_dispatch_and_read_model_reports_statuses() {
    let adapter = Arc::new(CountingAdapter::default());
    adapter
        .statuses
        .lock()
        .unwrap()
        .push(scoped_status("project-1", "workspace-1", source_ref()));
    let host = PluginRuntimeHost::new(adapter.clone());
    let read = host
        .read_plugins(PluginRuntimeReadRequest {
            request_id: "read-1".to_string(),
            project_domain_id: "project-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            plugin_ids: vec!["opencode.example".to_string()],
            include_config_validation: true,
            epochs: epochs(),
        })
        .await
        .expect("host read model should return status projection");

    assert_eq!(read.sources[0].plugin_id, "opencode.example");
    assert_eq!(read.plugin_statuses[0].source.plugin_id, "opencode.example");

    host.dispose_project("project-1", "workspace-1");
    let error = host
        .dispatch(envelope("event-5"))
        .await
        .expect_err("disposed projects must not dispatch to adapter");

    assert_eq!(error.kind, PortErrorKind::NotAvailable);
    assert!(adapter.calls.lock().unwrap().is_empty());

    let mut other_workspace = envelope("event-5-other-workspace");
    other_workspace.workspace_id = "workspace-2".to_string();
    let other_response = host
        .dispatch(other_workspace)
        .await
        .expect("dispose should stay scoped to the project workspace");
    assert_eq!(other_response.workspace_id, "workspace-2");
    assert_eq!(adapter.calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn idempotent_dispatch_cache_is_scoped_by_project_workspace_and_source() {
    let adapter = Arc::new(CountingAdapter::default());
    let host = PluginRuntimeHost::new(adapter.clone());
    let mut first = envelope("event-6");
    first.idempotency_key = "shared-idempotency".to_string();
    let mut second = first.clone();
    second.event_id = "event-7".to_string();
    second.workspace_id = "workspace-2".to_string();
    let mut third = first.clone();
    third.event_id = "event-8".to_string();
    third.project_domain_id = "project-2".to_string();
    let mut fourth = first.clone();
    fourth.event_id = "event-9".to_string();
    fourth.source.source = "file:///plugins/other-opencode-example".to_string();
    fourth.source.content_hash = "sha256:def456".to_string();

    let first_response = host.dispatch(first).await.expect("first dispatch");
    let second_response = host.dispatch(second).await.expect("second dispatch");
    let third_response = host.dispatch(third).await.expect("third dispatch");
    let fourth_response = host.dispatch(fourth).await.expect("fourth dispatch");

    assert_eq!(first_response.project_domain_id, "project-1");
    assert_eq!(first_response.workspace_id, "workspace-1");
    assert_eq!(second_response.project_domain_id, "project-1");
    assert_eq!(second_response.workspace_id, "workspace-2");
    assert_eq!(third_response.project_domain_id, "project-2");
    assert_eq!(
        fourth_response.plugin_id.as_deref(),
        Some("opencode.example")
    );
    assert_eq!(
        adapter.calls.lock().unwrap().len(),
        4,
        "same idempotency key in another execution domain or source must not replay cached response"
    );
}

#[tokio::test]
async fn idempotent_dispatch_cache_is_scoped_by_epoch_changes() {
    let adapter = Arc::new(CountingAdapter::default());
    let host = PluginRuntimeHost::new(adapter.clone());
    let mut first = envelope("event-10");
    first.idempotency_key = "shared-epoch-idempotency".to_string();
    let mut second = first.clone();
    second.epochs.policy_epoch += 1;

    let first_response = host.dispatch(first).await.expect("first dispatch");
    let second_response = host.dispatch(second).await.expect("second dispatch");

    assert_ne!(
        first_response.observed_epochs.policy_epoch,
        second_response.observed_epochs.policy_epoch
    );
    assert_eq!(
        adapter.calls.lock().unwrap().len(),
        2,
        "epoch changes must not replay stale idempotent dispatch responses"
    );
}

#[tokio::test]
async fn read_model_is_scoped_by_project_and_workspace() {
    let adapter = Arc::new(CountingAdapter::default());
    let mut project_2_source = source_ref();
    project_2_source.source = "file:///project-2/plugins/opencode-example".to_string();
    adapter.statuses.lock().unwrap().extend([
        scoped_status("project-1", "workspace-1", source_ref()),
        scoped_status("project-2", "workspace-2", project_2_source),
    ]);
    let host = PluginRuntimeHost::new(adapter);

    let read = host
        .read_plugins(PluginRuntimeReadRequest {
            request_id: "read-project-1".to_string(),
            project_domain_id: "project-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            plugin_ids: vec!["opencode.example".to_string()],
            include_config_validation: true,
            epochs: epochs(),
        })
        .await
        .expect("read should stay within requested execution domain");

    assert_eq!(read.sources.len(), 1);
    assert_eq!(read.sources[0].source, "file:///plugins/opencode-example");
}

#[tokio::test]
async fn read_model_rejects_wrong_workspace_response() {
    let adapter = Arc::new(CountingAdapter {
        invalid_workspace_read_response: true,
        ..Default::default()
    });
    let host = PluginRuntimeHost::new(adapter);

    let error = host
        .read_plugins(PluginRuntimeReadRequest {
            request_id: "read-wrong-workspace".to_string(),
            project_domain_id: "project-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            plugin_ids: vec!["opencode.example".to_string()],
            include_config_validation: true,
            epochs: epochs(),
        })
        .await
        .expect_err("wrong workspace read model response must fail closed");

    assert_eq!(error.kind, PortErrorKind::Backend);
    assert!(error.message.contains("workspace_id mismatch"));
}

#[tokio::test]
async fn malformed_adapter_success_quarantines_without_effects() {
    let adapter = Arc::new(CountingAdapter {
        invalid_project_response: true,
        ..Default::default()
    });
    let host = PluginRuntimeHost::new(adapter);

    let response = host
        .dispatch(envelope("event-8"))
        .await
        .expect("malformed adapter success should become typed quarantine response");

    assert!(response.effects.is_empty());
    assert_eq!(response.diagnostics[0].code, "plugin_host.invalid_response");
    assert_eq!(
        response.quarantine.as_ref().expect("quarantine").reason,
        PluginQuarantineReason::AdapterFailure
    );
}

#[tokio::test]
async fn permission_prompt_target_mismatch_quarantines_without_effects() {
    let adapter = Arc::new(CountingAdapter {
        invalid_permission_prompt: true,
        ..Default::default()
    });
    let host = PluginRuntimeHost::new(adapter);

    let response = host
        .dispatch(envelope("event-11"))
        .await
        .expect("prompt mismatch should become typed quarantine response");

    assert!(response.effects.is_empty());
    assert_eq!(response.diagnostics[0].code, "plugin_host.invalid_response");
    assert!(response.diagnostics[0]
        .message
        .contains("permission prompt target mismatch"));
}

#[tokio::test]
async fn permission_prompt_authority_mismatch_quarantines_without_effects() {
    let adapter = Arc::new(CountingAdapter {
        invalid_permission_authority: true,
        ..Default::default()
    });
    let host = PluginRuntimeHost::new(adapter);

    let response = host
        .dispatch(envelope("event-18"))
        .await
        .expect("prompt authority mismatch should become typed quarantine response");

    assert!(response.effects.is_empty());
    assert_eq!(response.diagnostics[0].code, "plugin_host.invalid_response");
    assert!(response.diagnostics[0]
        .message
        .contains("permission prompt owner mismatch"));
}

#[tokio::test]
async fn final_policy_decision_from_adapter_fails_closed() {
    let adapter = Arc::new(CountingAdapter {
        final_policy_decision: true,
        ..Default::default()
    });
    let host = PluginRuntimeHost::new(adapter);

    let response = host
        .dispatch(envelope("event-14"))
        .await
        .expect("adapter policy outcome should become typed quarantine response");

    assert!(response.effects.is_empty());
    assert!(response.diagnostics[0]
        .message
        .contains("final policy_allowed decisions"));
}

#[tokio::test]
async fn adapter_id_or_quarantine_with_effects_mismatch_fails_closed() {
    let wrong_adapter = Arc::new(CountingAdapter {
        wrong_adapter_id: true,
        ..Default::default()
    });
    let wrong_adapter_response = PluginRuntimeHost::new(wrong_adapter)
        .dispatch(envelope("event-12"))
        .await
        .expect("wrong adapter id should become typed quarantine response");
    assert!(wrong_adapter_response.effects.is_empty());
    assert!(wrong_adapter_response.diagnostics[0]
        .message
        .contains("adapter_id mismatch"));

    let quarantine_with_effects = Arc::new(CountingAdapter {
        success_quarantine_with_effects: true,
        ..Default::default()
    });
    let mixed_response = PluginRuntimeHost::new(quarantine_with_effects)
        .dispatch(envelope("event-13"))
        .await
        .expect("success effects mixed with quarantine should fail closed");
    assert!(mixed_response.effects.is_empty());
    assert!(mixed_response.diagnostics[0]
        .message
        .contains("quarantine response must not carry success effects"));
}

#[tokio::test]
async fn status_quarantine_with_success_effects_fails_closed() {
    let adapter = Arc::new(CountingAdapter {
        status_quarantine_with_effects: true,
        ..Default::default()
    });
    let host = PluginRuntimeHost::new(adapter);

    let response = host
        .dispatch(envelope("event-16"))
        .await
        .expect("status quarantine mixed with effects should fail closed");

    assert!(response.effects.is_empty());
    assert!(response.diagnostics[0]
        .message
        .contains("quarantine response must not carry success effects"));
}

#[tokio::test]
async fn nonzero_deadline_timeout_quarantines_without_success_effects() {
    let adapter = Arc::new(CountingAdapter {
        delay_ms: 50,
        ..Default::default()
    });
    let host = PluginRuntimeHost::new(adapter.clone());
    let mut request = envelope("event-9");
    request.deadline_ms = 1;

    let response = host
        .dispatch(request)
        .await
        .expect("timeout should return typed host response");

    assert!(response.effects.is_empty());
    assert_eq!(
        response.diagnostics[0].code,
        "plugin_host.deadline_exceeded"
    );
    assert_eq!(adapter.calls.lock().unwrap().len(), 1);
}
