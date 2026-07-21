use bitfun_external_sources::ExternalSourceCoordinator;
use bitfun_product_domains::external_sources::{
    EcosystemId, ExecutionDomainId, ExpandedPromptCommand, ExternalSourceContext,
    ExternalSourceHealth, ExternalSourceLifecycleState, ExternalSourceProviderError,
    ExternalSourceRecord, ExternalSourceScope, ExternalWatchRoot, PromptCommandAvailability,
    PromptCommandDefinition, PromptCommandProviderIdentity, PromptCommandProviderSnapshot,
    PromptCommandSourceProvider, SourceKey, SourceQualifiedCommandId,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn source(provider_id: &str, ecosystem_id: &str, source_id: &str) -> ExternalSourceRecord {
    ExternalSourceRecord {
        key: SourceKey::new(provider_id, source_id).expect("valid source key"),
        ecosystem_id: EcosystemId::new(ecosystem_id).expect("valid ecosystem id"),
        display_name: format!("{provider_id} commands"),
        source_kind: "prompt_commands".to_string(),
        scope: ExternalSourceScope::Project,
        location: format!("/workspace/{provider_id}"),
        execution_domain_id: ExecutionDomainId::new("local-user").expect("valid domain"),
        health: ExternalSourceHealth::Available,
        content_version: format!("{provider_id}-v1"),
        diagnostics: Vec::new(),
    }
}

fn command(provider_id: &str, source_id: &str, precedence: i32) -> PromptCommandDefinition {
    command_named(provider_id, source_id, "review", precedence)
}

fn command_named(
    provider_id: &str,
    source_id: &str,
    name: &str,
    version: i32,
) -> PromptCommandDefinition {
    PromptCommandDefinition {
        id: SourceQualifiedCommandId::new(SourceKey::new(provider_id, source_id).unwrap(), name)
            .unwrap(),
        name: name.to_string(),
        description: format!("Review from {provider_id}"),
        template: format!("{provider_id}: $ARGUMENTS"),
        availability: PromptCommandAvailability::Available,
        content_version: format!("command-v{version}"),
    }
}

fn context() -> ExternalSourceContext {
    ExternalSourceContext {
        workspace_root: Some(PathBuf::from("/workspace")),
        execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
    }
}

#[derive(Clone)]
enum ProviderState {
    Snapshot(PromptCommandProviderSnapshot),
    Failed(&'static str),
}

struct FakeProvider {
    identity: PromptCommandProviderIdentity,
    state: Arc<Mutex<ProviderState>>,
}

impl FakeProvider {
    fn new(provider_id: &str, ecosystem_id: &str, source_id: &str, precedence: i32) -> Self {
        let identity = PromptCommandProviderIdentity::new(
            provider_id,
            ecosystem_id,
            format!("{provider_id} display"),
        )
        .unwrap();
        Self {
            identity: identity.clone(),
            state: Arc::new(Mutex::new(ProviderState::Snapshot(
                PromptCommandProviderSnapshot {
                    provider: identity,
                    sources: vec![source(provider_id, ecosystem_id, source_id)],
                    commands: vec![command(provider_id, source_id, precedence)],
                    unavailable_command_ids: Vec::new(),
                    diagnostics: Vec::new(),
                },
            ))),
        }
    }

    fn state_handle(&self) -> Arc<Mutex<ProviderState>> {
        Arc::clone(&self.state)
    }
}

impl PromptCommandSourceProvider for FakeProvider {
    fn identity(&self) -> PromptCommandProviderIdentity {
        self.identity.clone()
    }

    fn discover(
        &self,
        _context: &ExternalSourceContext,
    ) -> Result<PromptCommandProviderSnapshot, ExternalSourceProviderError> {
        match self.state.lock().unwrap().clone() {
            ProviderState::Snapshot(snapshot) => Ok(snapshot),
            ProviderState::Failed(message) => Err(ExternalSourceProviderError::new(
                "fake.failed",
                message,
                true,
            )),
        }
    }

    fn expand(
        &self,
        command: &PromptCommandDefinition,
        arguments: &str,
    ) -> Result<ExpandedPromptCommand, ExternalSourceProviderError> {
        Ok(ExpandedPromptCommand {
            content: command.template.replace("$ARGUMENTS", arguments),
        })
    }

    fn watch_roots(&self, context: &ExternalSourceContext) -> Vec<ExternalWatchRoot> {
        vec![ExternalWatchRoot {
            path: context.workspace_root.clone().unwrap(),
            recursive: true,
        }]
    }
}

#[test]
fn assembly_can_disable_one_ecosystem_without_clearing_other_provider_generations() {
    let first = FakeProvider::new("first", "ecosystem.first", "project", 10);
    let second = FakeProvider::new("second", "ecosystem.second", "project", 20);
    let mut coordinator =
        ExternalSourceCoordinator::new(context(), vec![Arc::new(first), Arc::new(second)])
            .expect("construct coordinator");

    let results = coordinator
        .discovery_requests()
        .into_iter()
        .map(|request| {
            if request.ecosystem_id().as_str() == "ecosystem.second" {
                request.disabled()
            } else {
                request.execute()
            }
        })
        .collect();
    let snapshot = coordinator.apply_discovery_results(results);

    assert_eq!(snapshot.sources.len(), 1);
    assert_eq!(
        snapshot.sources[0].record.ecosystem_id.as_str(),
        "ecosystem.first"
    );
    assert_eq!(snapshot.commands.len(), 1);
    assert_eq!(snapshot.commands[0].definition.name, "review");
}

#[test]
fn provider_failure_isolated_and_successful_deletion_withdraws_only_its_generation() {
    let first = FakeProvider::new("first", "ecosystem.first", "project", 10);
    let first_state = first.state_handle();
    let second = FakeProvider::new("second", "ecosystem.second", "project", 20);
    let second_state = second.state_handle();
    let second_snapshot = match second_state.lock().unwrap().clone() {
        ProviderState::Snapshot(snapshot) => snapshot,
        ProviderState::Failed(_) => unreachable!(),
    };
    let mut coordinator =
        ExternalSourceCoordinator::new(context(), vec![Arc::new(first), Arc::new(second)])
            .expect("construct coordinator");

    let initial = coordinator.refresh();
    assert!(initial.commands.is_empty());
    assert_eq!(initial.command_conflicts.len(), 1);
    assert_eq!(initial.sources.len(), 2);
    let conflict_key = initial.command_conflicts[0].conflict_key.clone();
    let second_candidate = initial.command_conflicts[0]
        .candidates
        .iter()
        .find(|candidate| candidate.source.provider_id.as_str() == "second")
        .unwrap()
        .candidate_id
        .clone();
    coordinator
        .set_conflict_choice(&conflict_key, &second_candidate)
        .expect("select second provider once");

    *second_state.lock().unwrap() = ProviderState::Failed("temporary parse failure");
    let degraded = coordinator.refresh();
    assert_eq!(
        degraded.commands[0].definition.description,
        "Review from second"
    );
    assert_eq!(
        degraded
            .sources
            .iter()
            .find(|source| source.record.key.provider_id.as_str() == "second")
            .unwrap()
            .lifecycle,
        ExternalSourceLifecycleState::UsingLastValidVersion
    );

    *second_state.lock().unwrap() = ProviderState::Snapshot(PromptCommandProviderSnapshot {
        provider: PromptCommandProviderIdentity::new(
            "second",
            "ecosystem.second",
            "second display",
        )
        .unwrap(),
        sources: Vec::new(),
        commands: Vec::new(),
        unavailable_command_ids: Vec::new(),
        diagnostics: Vec::new(),
    });
    let removed = coordinator.refresh();
    assert_eq!(removed.sources.len(), 2);
    assert_eq!(
        removed
            .sources
            .iter()
            .find(|source| source.record.key.provider_id.as_str() == "second")
            .unwrap()
            .lifecycle,
        ExternalSourceLifecycleState::Removed
    );
    assert!(removed.commands.is_empty());
    assert_eq!(removed.command_conflicts.len(), 1);
    assert_eq!(removed.command_conflicts[0].candidates.len(), 1);
    assert_eq!(
        removed.command_conflicts[0].candidates[0]
            .source
            .provider_id
            .as_str(),
        "first"
    );
    assert_eq!(removed.command_conflicts[0].selected_candidate_id, None);

    let remaining_conflict = &removed.command_conflicts[0];
    coordinator
        .set_conflict_choice(
            &remaining_conflict.conflict_key,
            &remaining_conflict.candidates[0].candidate_id,
        )
        .expect("confirm the remaining provider after the candidate set changed");
    let confirmed = coordinator.snapshot();
    assert_eq!(
        confirmed.commands[0].definition.description,
        "Review from first"
    );

    let mut updated_first = match first_state.lock().unwrap().clone() {
        ProviderState::Snapshot(snapshot) => snapshot,
        ProviderState::Failed(_) => unreachable!(),
    };
    updated_first.commands[0].content_version = "first-command-v2".to_string();
    *first_state.lock().unwrap() = ProviderState::Snapshot(updated_first);
    let changed_singleton = coordinator.refresh();
    assert!(changed_singleton.commands.is_empty());
    assert_eq!(changed_singleton.command_conflicts.len(), 1);
    assert_eq!(
        changed_singleton.command_conflicts[0].selected_candidate_id,
        None
    );
    coordinator
        .set_conflict_choice(
            &changed_singleton.command_conflicts[0].conflict_key,
            &changed_singleton.command_conflicts[0].candidates[0].candidate_id,
        )
        .expect("confirm the changed singleton once");

    *second_state.lock().unwrap() = ProviderState::Snapshot(second_snapshot);
    let returned = coordinator.refresh();
    assert!(returned.commands.is_empty());
    assert_eq!(returned.command_conflicts.len(), 1);
    assert_eq!(returned.command_conflicts[0].selected_candidate_id, None);
}

#[test]
fn failed_command_uses_last_valid_without_reviving_a_deleted_sibling() {
    let provider = FakeProvider::new("first", "ecosystem.first", "project", 1);
    let state = provider.state_handle();
    let mut initial = match state.lock().unwrap().clone() {
        ProviderState::Snapshot(snapshot) => snapshot,
        ProviderState::Failed(_) => unreachable!(),
    };
    initial.commands = vec![
        command_named("first", "project", "deleted", 1),
        command_named("first", "project", "temporarily-broken", 1),
    ];
    *state.lock().unwrap() = ProviderState::Snapshot(initial.clone());
    let mut coordinator = ExternalSourceCoordinator::new(context(), vec![Arc::new(provider)])
        .expect("construct coordinator");

    let first = coordinator.refresh();
    assert_eq!(first.commands.len(), 2);

    let mut degraded = initial;
    degraded.sources[0].health = ExternalSourceHealth::Degraded;
    degraded.sources[0].content_version = "first-v2".to_string();
    degraded.commands.clear();
    degraded.unavailable_command_ids = vec![SourceQualifiedCommandId::new(
        SourceKey::new("first", "project").unwrap(),
        "temporarily-broken",
    )
    .unwrap()];
    *state.lock().unwrap() = ProviderState::Snapshot(degraded);

    let refreshed = coordinator.refresh();
    assert_eq!(refreshed.commands.len(), 1);
    assert_eq!(refreshed.commands[0].definition.name, "temporarily-broken");
    assert!(refreshed
        .commands
        .iter()
        .all(|command| command.definition.name != "deleted"));
    assert_eq!(
        refreshed.sources[0].lifecycle,
        ExternalSourceLifecycleState::UsingLastValidVersion
    );

    let mut deleted = match state.lock().unwrap().clone() {
        ProviderState::Snapshot(snapshot) => snapshot,
        ProviderState::Failed(_) => unreachable!(),
    };
    deleted.sources[0].health = ExternalSourceHealth::Available;
    deleted.sources[0].content_version = "first-v3".to_string();
    deleted.unavailable_command_ids.clear();
    *state.lock().unwrap() = ProviderState::Snapshot(deleted);

    let withdrawn = coordinator.refresh();
    assert!(withdrawn.commands.is_empty());
    assert_eq!(
        withdrawn.sources[0].lifecycle,
        ExternalSourceLifecycleState::Available
    );
}

#[test]
fn suppression_survives_refresh_and_expansion_dispatches_by_provider_identity() {
    let first = FakeProvider::new("first", "ecosystem.first", "project", 10);
    let second = FakeProvider::new("second", "ecosystem.second", "project", 20);
    let mut coordinator =
        ExternalSourceCoordinator::new(context(), vec![Arc::new(first), Arc::new(second)])
            .expect("construct coordinator");

    coordinator.refresh();
    let conflict = coordinator.snapshot().command_conflicts[0].clone();
    let second_candidate = conflict
        .candidates
        .iter()
        .find(|candidate| candidate.source.provider_id.as_str() == "second")
        .unwrap()
        .candidate_id
        .clone();
    coordinator
        .set_conflict_choice(&conflict.conflict_key, &second_candidate)
        .expect("select second provider once");
    let second_key = coordinator
        .snapshot()
        .sources
        .iter()
        .find(|source| source.record.key.provider_id.as_str() == "second")
        .unwrap()
        .stable_key
        .clone();
    coordinator
        .set_source_enabled(&second_key, false)
        .expect("suppress known source");
    let suppressed = coordinator.refresh();
    assert!(suppressed.commands.is_empty());
    let remaining_conflict = suppressed.command_conflicts[0].clone();
    let first_candidate = remaining_conflict.candidates[0].candidate_id.clone();
    coordinator
        .set_conflict_choice(&remaining_conflict.conflict_key, &first_candidate)
        .expect("confirm remaining provider after changing the candidate set");
    let confirmed = coordinator.snapshot();
    assert_eq!(
        confirmed.commands[0].definition.description,
        "Review from first"
    );
    assert_eq!(
        suppressed
            .sources
            .iter()
            .find(|source| source.record.key.provider_id.as_str() == "second")
            .unwrap()
            .lifecycle,
        ExternalSourceLifecycleState::Suppressed
    );

    let expanded = coordinator
        .expand_command("review", "this change")
        .expect("expand active command");
    assert_eq!(expanded.content, "first: this change");

    coordinator
        .set_source_enabled(&second_key, true)
        .expect("restore known source");
    let restored = coordinator.refresh();
    assert!(restored.commands.is_empty());
    assert_eq!(restored.command_conflicts.len(), 1);
    assert_eq!(restored.command_conflicts[0].selected_candidate_id, None);
}

#[test]
fn updated_candidate_content_requires_a_new_conflict_choice() {
    let first = FakeProvider::new("first", "ecosystem.first", "project", 10);
    let second = FakeProvider::new("second", "ecosystem.second", "project", 20);
    let second_state = second.state_handle();
    let mut coordinator =
        ExternalSourceCoordinator::new(context(), vec![Arc::new(first), Arc::new(second)])
            .expect("construct coordinator");

    let initial = coordinator.refresh();
    let initial_conflict = initial.command_conflicts[0].clone();
    let selected = initial_conflict.candidates[1].candidate_id.clone();
    coordinator
        .set_conflict_choice(&initial_conflict.conflict_key, &selected)
        .unwrap();
    assert_eq!(coordinator.snapshot().commands.len(), 1);

    let mut updated = match second_state.lock().unwrap().clone() {
        ProviderState::Snapshot(snapshot) => snapshot,
        ProviderState::Failed(_) => unreachable!(),
    };
    updated.sources[0].content_version = "second-v2".to_string();
    *second_state.lock().unwrap() = ProviderState::Snapshot(updated);

    let refreshed = coordinator.refresh();
    assert_eq!(refreshed.commands.len(), 1);
    assert_eq!(
        refreshed.command_conflicts[0].conflict_key,
        initial_conflict.conflict_key
    );
    assert_eq!(
        refreshed.command_conflicts[0].selected_candidate_id,
        Some(selected.clone())
    );

    let mut updated = match second_state.lock().unwrap().clone() {
        ProviderState::Snapshot(snapshot) => snapshot,
        ProviderState::Failed(_) => unreachable!(),
    };
    updated.commands[0].content_version = "second-command-v2".to_string();
    *second_state.lock().unwrap() = ProviderState::Snapshot(updated);

    let refreshed = coordinator.refresh();
    assert!(refreshed.commands.is_empty());
    assert_ne!(
        refreshed.command_conflicts[0].conflict_key,
        initial_conflict.conflict_key
    );
    assert!(refreshed.command_conflicts[0]
        .selected_candidate_id
        .is_none());

    for version in 3..=10 {
        let conflict = coordinator.snapshot().command_conflicts[0].clone();
        let selected = conflict
            .candidates
            .iter()
            .find(|candidate| candidate.source.provider_id.as_str() == "second")
            .unwrap()
            .candidate_id
            .clone();
        coordinator
            .set_conflict_choice(&conflict.conflict_key, &selected)
            .unwrap();
        assert_eq!(coordinator.conflict_choices().len(), 1);
        assert_eq!(coordinator.conflict_lineage_current_keys().len(), 1);
        assert_eq!(coordinator.conflicted_candidate_ids().len(), 2);

        let mut updated = match second_state.lock().unwrap().clone() {
            ProviderState::Snapshot(snapshot) => snapshot,
            ProviderState::Failed(_) => unreachable!(),
        };
        updated.commands[0].content_version = format!("second-command-v{version}");
        *second_state.lock().unwrap() = ProviderState::Snapshot(updated);

        let refreshed = coordinator.refresh();
        assert!(refreshed.commands.is_empty());
        assert!(refreshed.command_conflicts[0]
            .selected_candidate_id
            .is_none());
        assert!(coordinator.conflict_choices().len() <= 1);
        assert_eq!(coordinator.conflict_lineage_current_keys().len(), 1);
        assert_eq!(coordinator.conflicted_candidate_ids().len(), 2);
    }
}

#[test]
fn duplicate_provider_registration_is_rejected_without_ecosystem_branching() {
    let first = Arc::new(FakeProvider::new("same", "ecosystem.first", "one", 1));
    let duplicate = Arc::new(FakeProvider::new("same", "ecosystem.other", "two", 2));

    let error = ExternalSourceCoordinator::new(context(), vec![first, duplicate])
        .expect_err("provider id collision must be rejected");
    assert!(error.contains("same"));
}

#[test]
fn catalog_stays_pending_until_every_provider_has_an_initial_result() {
    let first = Arc::new(FakeProvider::new("first", "ecosystem.first", "global", 1));
    let second = Arc::new(FakeProvider::new("second", "ecosystem.second", "global", 1));
    let mut coordinator = ExternalSourceCoordinator::new(context(), vec![first, second]).unwrap();

    assert!(coordinator.snapshot().discovery_pending);

    let first = coordinator
        .discovery_requests()
        .into_iter()
        .find(|request| request.provider_id().as_str() == "first")
        .unwrap()
        .execute();
    assert!(coordinator.apply_discovery_result(first).discovery_pending);

    let second = coordinator
        .discovery_requests()
        .into_iter()
        .find(|request| request.provider_id().as_str() == "second")
        .unwrap()
        .execute();
    assert!(!coordinator.apply_discovery_result(second).discovery_pending);
}

#[test]
fn invocation_guard_rejects_a_command_changed_after_projection() {
    let provider = FakeProvider::new("first", "ecosystem.first", "project", 1);
    let state = provider.state_handle();
    let mut coordinator = ExternalSourceCoordinator::new(context(), vec![Arc::new(provider)])
        .expect("construct coordinator");
    let projected = coordinator.refresh().commands[0].definition.clone();

    let mut updated = match state.lock().unwrap().clone() {
        ProviderState::Snapshot(snapshot) => snapshot,
        ProviderState::Failed(_) => unreachable!(),
    };
    updated.commands[0].template = "updated: $ARGUMENTS".to_string();
    updated.commands[0].content_version = "command-v2".to_string();
    *state.lock().unwrap() = ProviderState::Snapshot(updated);
    coordinator.refresh();

    let error = coordinator
        .expand_command_guarded(
            "review",
            "change",
            Some(&projected.id.stable_key()),
            Some(&projected.content_version),
        )
        .expect_err("stale projection must not execute changed content");
    assert_eq!(error.code, "external_source.stale_command_selection");
}
