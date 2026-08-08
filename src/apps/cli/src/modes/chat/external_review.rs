// Pure projections and review text derived from the external-source catalog.
use bitfun_product_domains::external_source_control::{
    ExternalApplicationControlActionV2, ExternalApplicationControlRequestV2,
    ExternalApplicationControlResultV2, ExternalApplicationEffectiveStatusV2,
    ExternalApplicationHealthV2, ExternalApplicationOperationOutcomeV2,
    ExternalApplicationPrimaryActionV2, ExternalApplicationRecoveryActionV2,
    ExternalApplicationReviewItemRefV2, ExternalApplicationReviewPageRequestV2,
    ExternalApplicationReviewPageV2, ExternalApplicationReviewSelectionBaselineV2,
    ExternalApplicationReviewSelectionOverrideV2, ExternalApplicationSnapshotV2,
    ExternalApplicationTargetScopeV2, ExternalSourceDesiredState, ExternalSourceEffectiveStatus,
    ExternalSourceRecoveryActionV1, ExternalSourceSupportState,
    EXTERNAL_APPLICATION_REVIEW_PAGE_MAX_ITEMS, EXTERNAL_APPLICATION_SCHEMA_V2,
};

fn external_command_projections(
    snapshot: &ExternalSourceCatalogSnapshot,
    conflict_choices: &BTreeMap<String, String>,
) -> Vec<ExternalCommandProjection> {
    let built_in_actions = slash_actions(ActionState::chat(false, false));
    let mut projections = snapshot
        .commands
        .iter()
        .filter_map(|entry| {
            let ecosystem = snapshot
                .sources
                .iter()
                .find(|source| source.record.key == entry.definition.id.source)
                .map(|source| source.record.ecosystem_id.as_str())
                .unwrap_or("external");
            let restricted = !matches!(
                entry.definition.availability,
                PromptCommandAvailability::Available
            );
            let native_collision = built_in_actions.iter().find_map(|action| {
                if !action
                    .name
                    .trim_start_matches('/')
                    .eq_ignore_ascii_case(&entry.definition.name)
                {
                    return None;
                }
                let source = snapshot
                    .sources
                    .iter()
                    .find(|source| source.record.key == entry.definition.id.source)?;
                let native_candidate_id = format!("bitfun.cli:{}", action.id);
                let external_candidate_id = entry.candidate_id.clone();
                let conflict_key = native_prompt_command_conflict_key(
                    source.record.execution_domain_id.as_str(),
                    &entry.definition.name,
                    [
                        (
                            native_candidate_id.as_str(),
                            action_conflict_behavior_version(action.id),
                        ),
                        (
                            external_candidate_id.as_str(),
                            entry.definition.content_version.as_str(),
                        ),
                    ],
                );
                Some(NativeCommandCollisionProjection {
                    native_action_id: action.id.to_string(),
                    native_candidate_id,
                    external_candidate_id,
                    selected_candidate_id: conflict_choices.get(&conflict_key).cloned(),
                    conflict_key,
                })
            });
            Some(ExternalCommandProjection {
                action_id: format!("external-command:{}", entry.definition.name),
                command_name: entry.definition.name.clone(),
                invocation_alias: format!("/{}", entry.definition.name),
                candidate_id: entry.candidate_id.clone(),
                content_version: entry.definition.content_version.clone(),
                description: format!("{} · {}", entry.definition.description, ecosystem),
                restricted,
                provider_conflict_key: None,
                native_collision,
            })
        })
        .collect::<Vec<_>>();

    for conflict in snapshot
        .command_conflicts
        .iter()
        .filter(|conflict| conflict.selected_candidate_id.is_none())
    {
        let built_in = built_in_actions.iter().find(|action| {
            action
                .name
                .trim_start_matches('/')
                .eq_ignore_ascii_case(&conflict.command_name)
        });
        let native_group = built_in.and_then(|action| {
            let execution_domain = conflict.candidates.iter().find_map(|candidate| {
                snapshot
                    .sources
                    .iter()
                    .find(|source| source.record.key == candidate.source)
                    .map(|source| source.record.execution_domain_id.as_str())
            })?;
            let native_candidate_id = format!("bitfun.cli:{}", action.id);
            let mut candidates = conflict
                .candidates
                .iter()
                .map(|candidate| {
                    (
                        candidate.candidate_id.as_str(),
                        candidate.content_version.as_str(),
                    )
                })
                .collect::<Vec<_>>();
            candidates.push((
                native_candidate_id.as_str(),
                action_conflict_behavior_version(action.id),
            ));
            let conflict_key = native_prompt_command_conflict_key(
                execution_domain,
                &conflict.command_name,
                candidates,
            );
            Some((action.id.to_string(), native_candidate_id, conflict_key))
        });
        projections.extend(conflict.candidates.iter().map(|candidate| {
            let native_collision = native_group.as_ref().map(
                |(native_action_id, native_candidate_id, conflict_key)| {
                    NativeCommandCollisionProjection {
                        native_action_id: native_action_id.clone(),
                        native_candidate_id: native_candidate_id.clone(),
                        external_candidate_id: candidate.candidate_id.clone(),
                        selected_candidate_id: conflict_choices.get(conflict_key).cloned(),
                        conflict_key: conflict_key.clone(),
                    }
                },
            );
            ExternalCommandProjection {
                action_id: format!("external-command-candidate:{}", candidate.candidate_id),
                command_name: conflict.command_name.clone(),
                invocation_alias: format!("/{}", conflict.command_name),
                candidate_id: candidate.candidate_id.clone(),
                content_version: candidate.content_version.clone(),
                description: format!(
                    "{} · {} · {}",
                    candidate.command_description,
                    candidate.source_display_name,
                    candidate.ecosystem_id
                ),
                restricted: !matches!(candidate.availability, PromptCommandAvailability::Available),
                provider_conflict_key: Some(conflict.conflict_key.clone()),
                native_collision,
            }
        }));
    }
    projections
}

fn external_command_counts(snapshot: &ExternalSourceCatalogSnapshot) -> (usize, usize) {
    snapshot
        .commands
        .iter()
        .fold((0, 0), |(available, restricted), entry| {
            if matches!(
                entry.definition.availability,
                PromptCommandAvailability::Available
            ) {
                (available + 1, restricted)
            } else {
                (available, restricted + 1)
            }
        })
}

fn external_integration_policy_lines(snapshot: &ExternalSourceCatalogSnapshot) -> Vec<String> {
    let policy = &snapshot.integration_policy;
    if policy.status
        == bitfun_product_domains::external_integration_policy::ExternalIntegrationPolicyStatus::IncompatibleSchema
    {
        return vec![
            format!(
                "Access: safely off; unsupported policy schema {}",
                policy.schema_major
            ),
            "Recover: bitfun config external reset-incompatible".to_string(),
        ];
    }
    if !policy.status.is_compatible() {
        return vec![
            format!(
                "Access: safely off; unsupported policy status '{}'",
                policy.status.as_str()
            ),
            "Recover: upgrade BitFun or connect through a compatible workspace host".to_string(),
        ];
    }
    let scope = if policy.workspace_override.is_some() {
        "this project overrides global settings"
    } else {
        "this project inherits global settings"
    };
    if policy.registered_ecosystems.is_empty() {
        return vec![format!("Access: unavailable; {scope}")];
    }
    let mut lines = vec![format!(
        "Access: {}; {scope}",
        if policy.effective.enabled {
            "enabled"
        } else {
            "disabled"
        }
    )];
    for descriptor in &policy.registered_ecosystems {
        let Some(ecosystem) = policy.effective.ecosystems.get(&descriptor.ecosystem_id) else {
            lines.push(format!("{}: unavailable", descriptor.display_name));
            continue;
        };
        let mode = match ecosystem.mode.as_str() {
            "recommended" => "recommended",
            "discover_only" => "discover only",
            "disabled" => "off",
            "custom" => "custom",
            _ => "unsupported, safely off",
        };
        let capability_summary = descriptor
            .capabilities
            .iter()
            .filter_map(|capability| {
                ecosystem
                    .capabilities
                    .get(&capability.capability_id)
                    .map(|access| {
                        let access = match access.as_str() {
                            "disabled" => "off",
                            "discover_only" => "discover",
                            "ask_before_use" => "ask",
                            "auto" => "auto",
                            _ => "unsupported, safely off",
                        };
                        format!("{} {access}", capability.capability_id.as_str())
                    })
            })
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!(
            "{}: {mode}; {capability_summary}",
            descriptor.display_name
        ));
    }
    lines.push("Manage: bitfun config external --help".to_string());
    lines
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExternalToolReviewAction {
    Show,
    Refresh,
    Decide {
        approval_key: String,
        decision_key: String,
        approved: bool,
    },
    Choose {
        conflict_key: String,
        candidate_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExternalControlUiAction {
    Show,
    Refresh,
    SetSafeMode(bool),
    SetSourceEnabled { source_key: String, enabled: bool },
}

fn parse_external_control_action(arguments: &str) -> Result<ExternalControlUiAction, String> {
    match arguments.split_whitespace().collect::<Vec<_>>().as_slice() {
        [] | ["status"] => Ok(ExternalControlUiAction::Show),
        ["refresh"] => Ok(ExternalControlUiAction::Refresh),
        ["safe-mode", "on"] => Ok(ExternalControlUiAction::SetSafeMode(true)),
        ["safe-mode", "off"] => Ok(ExternalControlUiAction::SetSafeMode(false)),
        ["source", "enable", source_key] => Ok(ExternalControlUiAction::SetSourceEnabled {
            source_key: (*source_key).to_string(),
            enabled: true,
        }),
        ["source", "disable", source_key] => Ok(ExternalControlUiAction::SetSourceEnabled {
            source_key: (*source_key).to_string(),
            enabled: false,
        }),
        _ => Err("usage: /extensions [status | refresh | safe-mode on | safe-mode off | source enable <source-key> | source disable <source-key>]".to_string()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExternalReviewDirection {
    Next,
    Previous,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExternalReviewNavigation {
    Open,
    Move {
        expected_cursor: Option<String>,
        previous_cursors: Vec<Option<String>>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExternalApplicationUiAction {
    Show,
    Refresh,
    ConnectApplication {
        application_id: String,
    },
    DisconnectApplication {
        application_id: String,
    },
    DeferApplication {
        application_id: String,
    },
    OpenReview,
    ReviewNext,
    ReviewPrevious,
    SetReviewItem {
        item_ref: ExternalApplicationReviewItemRefV2,
        selected: bool,
    },
    ApplyReview,
    DeferReview,
}

struct ExternalApplicationReviewUiState {
    page: ExternalApplicationReviewPageV2,
    previous_cursors: Vec<Option<String>>,
    selection_overrides: Vec<(ExternalApplicationReviewItemRefV2, bool)>,
}

enum ExternalApplicationAsyncResult {
    Snapshot(ExternalApplicationSnapshotV2),
    LegacySnapshot(bitfun_app_server_protocol::external_source::ExternalSourceSnapshotResponse),
    ReviewPage {
        page: ExternalApplicationReviewPageV2,
        navigation: ExternalReviewNavigation,
    },
    Mutation {
        result: ExternalApplicationControlResultV2,
        snapshot: ExternalApplicationSnapshotV2,
    },
}

fn should_fallback_to_legacy_external_status(
    shared: bool,
    error: &ExternalSourceOperationError,
) -> bool {
    !shared
        && matches!(
            error.code,
            ExternalSourceOperationErrorCode::HostCapabilityUnavailable
                | ExternalSourceOperationErrorCode::Unsupported
        )
}

enum ExternalApplicationPendingRequest {
    Snapshot {
        force_refresh: bool,
    },
    ReviewPage {
        request: ExternalApplicationReviewPageRequestV2,
        navigation: ExternalReviewNavigation,
    },
    Mutation(ExternalApplicationControlRequestV2),
}

struct ExternalApplicationMutationResult {
    action: ExternalApplicationUiAction,
    result: std::result::Result<ExternalApplicationAsyncResult, ExternalSourceOperationError>,
}

#[derive(Default)]
struct ExternalApplicationUiState {
    snapshot: Option<ExternalApplicationSnapshotV2>,
    review: Option<ExternalApplicationReviewUiState>,
    pending_rx: Option<Receiver<ExternalApplicationMutationResult>>,
}

impl ExternalApplicationUiState {
    fn replace_snapshot(&mut self, snapshot: ExternalApplicationSnapshotV2) -> Result<(), String> {
        snapshot.validate().map_err(str::to_string)?;
        let keep_review = self.review.as_ref().is_some_and(|review| {
            snapshot.review_summary.as_ref().is_some_and(|summary| {
                summary.review_id == review.page.review_id
                    && snapshot.preference_revision == review.page.preference_revision
                    && snapshot.execution_domain_id == review.page.execution_domain_id
                    && snapshot.workspace_scope_id == review.page.workspace_scope_id
            })
        });
        if !keep_review {
            self.review = None;
        }
        self.snapshot = Some(snapshot);
        Ok(())
    }

    fn snapshot(&self) -> Result<&ExternalApplicationSnapshotV2, String> {
        self.snapshot.as_ref().ok_or_else(|| {
            "External application V2 status is unavailable; run /extensions status".to_string()
        })
    }

    fn can_mutate(&self) -> Result<(), String> {
        let snapshot = self.snapshot()?;
        let scope_allowed = if snapshot.workspace_scope_id.is_some() {
            snapshot.host_capabilities.can_manage_workspace_override
        } else {
            snapshot.host_capabilities.can_manage_user_default
        };
        if snapshot.host_capabilities.can_mutate && scope_allowed {
            Ok(())
        } else {
            Err("This host is read-only for external application changes.".to_string())
        }
    }

    fn target_scope(
        snapshot: &ExternalApplicationSnapshotV2,
    ) -> (ExternalApplicationTargetScopeV2, Option<String>) {
        match snapshot.workspace_scope_id.clone() {
            Some(workspace_scope_id) => (
                ExternalApplicationTargetScopeV2::WorkspaceOverride,
                Some(workspace_scope_id),
            ),
            None => (ExternalApplicationTargetScopeV2::UserDefault, None),
        }
    }

    fn control_request(
        &self,
        operation_id: &str,
        action: ExternalApplicationControlActionV2,
    ) -> Result<ExternalApplicationControlRequestV2, String> {
        self.can_mutate()?;
        let snapshot = self.snapshot()?;
        let (target_scope, workspace_scope_id) = Self::target_scope(snapshot);
        let request = ExternalApplicationControlRequestV2 {
            schema_version: EXTERNAL_APPLICATION_SCHEMA_V2,
            execution_domain_id: snapshot.execution_domain_id.clone(),
            workspace_scope_id,
            target_scope,
            operation_id: operation_id.to_string(),
            expected_preference_revision: snapshot.preference_revision,
            action,
        };
        request.validate().map_err(str::to_string)?;
        Ok(request)
    }

    fn open_review_page_request(&self) -> Result<ExternalApplicationReviewPageRequestV2, String> {
        let snapshot = self.snapshot()?;
        if !snapshot.host_capabilities.can_read_review {
            return Err("This host cannot read the external application review.".to_string());
        }
        let summary = snapshot
            .review_summary
            .as_ref()
            .ok_or_else(|| "No external application review is pending.".to_string())?;
        let (target_scope, workspace_scope_id) = Self::target_scope(snapshot);
        Ok(ExternalApplicationReviewPageRequestV2 {
            schema_version: EXTERNAL_APPLICATION_SCHEMA_V2,
            execution_domain_id: snapshot.execution_domain_id.clone(),
            workspace_scope_id,
            target_scope,
            review_id: summary.review_id.clone(),
            preference_revision: snapshot.preference_revision,
            expected_generations: Vec::new(),
            cursor: None,
            page_size: EXTERNAL_APPLICATION_REVIEW_PAGE_MAX_ITEMS,
        })
    }

    fn review_page_request(
        &self,
        direction: ExternalReviewDirection,
    ) -> Result<
        (
            ExternalApplicationReviewPageRequestV2,
            ExternalReviewNavigation,
        ),
        String,
    > {
        let review = self
            .review
            .as_ref()
            .ok_or_else(|| "Open /extensions review before changing review pages.".to_string())?;
        let (cursor, previous_cursors) = match direction {
            ExternalReviewDirection::Next => {
                let cursor = review.page.next_cursor.clone().ok_or_else(|| {
                    "The external application review has no next page.".to_string()
                })?;
                let mut history = review.previous_cursors.clone();
                history.push(review.page.cursor.clone());
                (Some(cursor), history)
            }
            ExternalReviewDirection::Previous => {
                let mut history = review.previous_cursors.clone();
                let cursor = history.pop().ok_or_else(|| {
                    "The external application review has no previous page.".to_string()
                })?;
                (cursor, history)
            }
        };
        let request = ExternalApplicationReviewPageRequestV2 {
            schema_version: review.page.schema_version,
            execution_domain_id: review.page.execution_domain_id.clone(),
            workspace_scope_id: review.page.workspace_scope_id.clone(),
            target_scope: review.page.target_scope,
            review_id: review.page.review_id.clone(),
            preference_revision: review.page.preference_revision,
            expected_generations: review.page.expected_generations.clone(),
            cursor: cursor.clone(),
            page_size: EXTERNAL_APPLICATION_REVIEW_PAGE_MAX_ITEMS,
        };
        Ok((
            request,
            ExternalReviewNavigation::Move {
                expected_cursor: cursor,
                previous_cursors,
            },
        ))
    }

    fn replace_review_page(
        &mut self,
        page: ExternalApplicationReviewPageV2,
        navigation: ExternalReviewNavigation,
    ) -> Result<(), String> {
        page.validate().map_err(str::to_string)?;
        let snapshot = self.snapshot()?;
        let summary = snapshot.review_summary.as_ref().ok_or_else(|| {
            "The external application review is stale; refresh /extensions.".to_string()
        })?;
        let (expected_scope, expected_workspace_scope_id) = Self::target_scope(snapshot);
        if page.execution_domain_id != snapshot.execution_domain_id
            || page.workspace_scope_id != expected_workspace_scope_id
            || page.target_scope != expected_scope
            || page.review_id != summary.review_id
            || page.preference_revision != snapshot.preference_revision
        {
            return Err(
                "The external application review is stale; refresh /extensions.".to_string(),
            );
        }
        if matches!(&navigation, ExternalReviewNavigation::Move { .. })
            && self
                .review
                .as_ref()
                .is_none_or(|review| review.page.expected_generations != page.expected_generations)
        {
            return Err(
                "The external application review generation is stale; reopen /extensions review."
                    .to_string(),
            );
        }
        let (expected_cursor, previous_cursors, keep_overrides) = match navigation {
            ExternalReviewNavigation::Open => (None, Vec::new(), false),
            ExternalReviewNavigation::Move {
                expected_cursor,
                previous_cursors,
            } => (expected_cursor, previous_cursors, true),
        };
        if page.cursor != expected_cursor {
            return Err(
                "The external application review page is stale; reopen /extensions review."
                    .to_string(),
            );
        }
        let selection_overrides = if keep_overrides {
            self.review
                .take()
                .map(|review| review.selection_overrides)
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        self.review = Some(ExternalApplicationReviewUiState {
            page,
            previous_cursors,
            selection_overrides,
        });
        Ok(())
    }

    fn review_item_selected(&self, index: usize) -> Result<bool, String> {
        let review = self
            .review
            .as_ref()
            .ok_or_else(|| "Open /extensions review before selecting items.".to_string())?;
        let item = review.page.items.get(index).ok_or_else(|| {
            "That review item is not on the current page; reopen /extensions review.".to_string()
        })?;
        Ok(review
            .selection_overrides
            .iter()
            .find_map(|(item_ref, selected)| (item_ref == &item.item_ref).then_some(*selected))
            .unwrap_or(item.recommended))
    }

    fn set_review_item_selected(&mut self, index: usize, selected: bool) -> Result<(), String> {
        self.can_mutate()?;
        let review = self
            .review
            .as_mut()
            .ok_or_else(|| "Open /extensions review before selecting items.".to_string())?;
        let item = review.page.items.get(index).ok_or_else(|| {
            "That review item is not on the current page; reopen /extensions review.".to_string()
        })?;
        if selected == item.recommended {
            review
                .selection_overrides
                .retain(|(item_ref, _)| item_ref != &item.item_ref);
        } else if let Some((_, current)) = review
            .selection_overrides
            .iter_mut()
            .find(|(item_ref, _)| item_ref == &item.item_ref)
        {
            *current = selected;
        } else {
            review
                .selection_overrides
                .push((item.item_ref.clone(), selected));
        }
        Ok(())
    }

    fn review_apply_request(
        &self,
        operation_id: &str,
    ) -> Result<ExternalApplicationControlRequestV2, String> {
        let review = self
            .review
            .as_ref()
            .ok_or_else(|| "Open /extensions review before applying it.".to_string())?;
        self.control_request(
            operation_id,
            ExternalApplicationControlActionV2::SubmitApplicationReview {
                review_id: review.page.review_id.clone(),
                expected_generations: review.page.expected_generations.clone(),
                selection_baseline: ExternalApplicationReviewSelectionBaselineV2::Recommended,
                selection_overrides: review
                    .selection_overrides
                    .iter()
                    .map(
                        |(item_ref, selected)| ExternalApplicationReviewSelectionOverrideV2 {
                            item_ref: item_ref.clone(),
                            selected: *selected,
                        },
                    )
                    .collect(),
            },
        )
    }
}

fn external_application_for_number(
    state: &ExternalApplicationUiState,
    value: Option<&str>,
) -> Result<bitfun_product_domains::external_source_control::ExternalApplicationSummaryV2, String> {
    let index = parse_positive_index(value, "application number")?;
    state
        .snapshot()?
        .applications
        .get(index)
        .cloned()
        .ok_or_else(|| {
            "That application is not in the displayed V2 snapshot; run /extensions status."
                .to_string()
        })
}

fn external_review_item_for_number(
    state: &ExternalApplicationUiState,
    value: Option<&str>,
) -> Result<ExternalApplicationReviewItemRefV2, String> {
    let index = parse_positive_index(value, "review item number")?;
    state
        .review
        .as_ref()
        .and_then(|review| review.page.items.get(index))
        .map(|item| item.item_ref.clone())
        .ok_or_else(|| {
            "That item is not in the displayed review page; reopen /extensions review.".to_string()
        })
}

fn parse_external_application_action(
    arguments: &str,
    state: &ExternalApplicationUiState,
) -> Result<ExternalApplicationUiAction, String> {
    let mut parts = arguments.split_whitespace();
    let Some(command) = parts.next() else {
        return Ok(ExternalApplicationUiAction::Show);
    };
    if command.eq_ignore_ascii_case("status") {
        if parts.next().is_none() {
            return Ok(ExternalApplicationUiAction::Show);
        }
    } else if command.eq_ignore_ascii_case("refresh") {
        if parts.next().is_none() {
            return Ok(ExternalApplicationUiAction::Refresh);
        }
    } else if command.eq_ignore_ascii_case("connect")
        || command.eq_ignore_ascii_case("disconnect")
        || command.eq_ignore_ascii_case("defer")
    {
        state.can_mutate()?;
        let application = external_application_for_number(state, parts.next())?;
        if parts.next().is_some() {
            return Err(format!("usage: /extensions {command} <application-number>"));
        }
        let allowed = if command.eq_ignore_ascii_case("connect") {
            application.primary_action == ExternalApplicationPrimaryActionV2::Connect
        } else if command.eq_ignore_ascii_case("disconnect") {
            application.effective_status == ExternalApplicationEffectiveStatusV2::Connected
        } else {
            application.effective_status == ExternalApplicationEffectiveStatusV2::NeedsAttention
        };
        if !allowed {
            return Err(format!(
                "Application {} no longer offers that next action; run /extensions status.",
                application.application_id
            ));
        }
        return if command.eq_ignore_ascii_case("connect") {
            Ok(ExternalApplicationUiAction::ConnectApplication {
                application_id: application.application_id.clone(),
            })
        } else if command.eq_ignore_ascii_case("disconnect") {
            Ok(ExternalApplicationUiAction::DisconnectApplication {
                application_id: application.application_id.clone(),
            })
        } else {
            Ok(ExternalApplicationUiAction::DeferApplication {
                application_id: application.application_id.clone(),
            })
        };
    } else if command.eq_ignore_ascii_case("review") {
        let Some(review_command) = parts.next() else {
            return Ok(ExternalApplicationUiAction::OpenReview);
        };
        if review_command.eq_ignore_ascii_case("next") && parts.next().is_none() {
            state.review_page_request(ExternalReviewDirection::Next)?;
            return Ok(ExternalApplicationUiAction::ReviewNext);
        }
        if review_command.eq_ignore_ascii_case("previous") && parts.next().is_none() {
            state.review_page_request(ExternalReviewDirection::Previous)?;
            return Ok(ExternalApplicationUiAction::ReviewPrevious);
        }
        if review_command.eq_ignore_ascii_case("include")
            || review_command.eq_ignore_ascii_case("exclude")
        {
            state.can_mutate()?;
            let item_ref = external_review_item_for_number(state, parts.next())?;
            if parts.next().is_some() {
                return Err(format!(
                    "usage: /extensions review {review_command} <item-number>"
                ));
            }
            return Ok(ExternalApplicationUiAction::SetReviewItem {
                item_ref,
                selected: review_command.eq_ignore_ascii_case("include"),
            });
        }
        if review_command.eq_ignore_ascii_case("apply") && parts.next().is_none() {
            state.can_mutate()?;
            return Ok(ExternalApplicationUiAction::ApplyReview);
        }
        if review_command.eq_ignore_ascii_case("defer") && parts.next().is_none() {
            return Ok(ExternalApplicationUiAction::DeferReview);
        }
    }
    Err("usage: /extensions [status | refresh | connect <number> | disconnect <number> | defer <number> | review [next | previous | include <number> | exclude <number> | apply | defer]]".to_string())
}

fn external_application_status_label(status: ExternalApplicationEffectiveStatusV2) -> &'static str {
    match status {
        ExternalApplicationEffectiveStatusV2::Connected => "Connected",
        ExternalApplicationEffectiveStatusV2::ConfigurationAvailable => "Configuration available",
        ExternalApplicationEffectiveStatusV2::NoConfiguration => "No configuration",
        ExternalApplicationEffectiveStatusV2::NeedsAttention => "Needs attention",
        ExternalApplicationEffectiveStatusV2::TemporarilyUnavailable => "Temporarily unavailable",
    }
}

fn external_application_health_label(health: ExternalApplicationHealthV2) -> &'static str {
    match health {
        ExternalApplicationHealthV2::Healthy => "healthy",
        ExternalApplicationHealthV2::Degraded => "degraded",
        ExternalApplicationHealthV2::Unavailable => "unavailable",
    }
}

fn external_application_recovery_label(
    action: &ExternalApplicationRecoveryActionV2,
) -> &'static str {
    match action {
        ExternalApplicationRecoveryActionV2::Refresh => "refresh",
        ExternalApplicationRecoveryActionV2::Retry => "retry",
        ExternalApplicationRecoveryActionV2::ReconnectHost => "reconnect host",
        ExternalApplicationRecoveryActionV2::Review => "review",
        ExternalApplicationRecoveryActionV2::UpgradeHost => "upgrade host",
        ExternalApplicationRecoveryActionV2::ViewReason => "view reason",
        ExternalApplicationRecoveryActionV2::ExitSafeMode => "exit safe mode",
        ExternalApplicationRecoveryActionV2::ResolveConflict => "resolve conflict",
        ExternalApplicationRecoveryActionV2::InstallRuntime => "install runtime",
    }
}

fn external_application_overview_text(snapshot: &ExternalApplicationSnapshotV2) -> String {
    let mut lines = vec![
        "External applications".to_string(),
        String::new(),
        format!(
            "Safe Mode: {}",
            if snapshot.safe_mode { "on" } else { "off" }
        ),
    ];
    let scope_can_mutate = snapshot.host_capabilities.can_mutate
        && if snapshot.workspace_scope_id.is_some() {
            snapshot.host_capabilities.can_manage_workspace_override
        } else {
            snapshot.host_capabilities.can_manage_user_default
        };
    for (index, application) in snapshot.applications.iter().enumerate() {
        let number = index + 1;
        lines.push(format!(
            "{number}. {} - {}",
            application.display_name,
            external_application_status_label(application.effective_status)
        ));
        lines.push(format!(
            "   {} enabled, {} pending review, {} blocked, {} conflicts",
            application.enabled_count,
            application.pending_review_count,
            application.blocked_count,
            application.conflict_count
        ));
        lines.push(format!(
            "   Health: {}",
            external_application_health_label(application.health)
        ));
        if !application.recovery_actions.is_empty() {
            lines.push(format!(
                "   Recovery: {}",
                application
                    .recovery_actions
                    .iter()
                    .map(external_application_recovery_label)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if scope_can_mutate {
            match application.primary_action {
                ExternalApplicationPrimaryActionV2::Connect => {
                    lines.push(format!("   Next: /extensions connect {number}"))
                }
                ExternalApplicationPrimaryActionV2::Review => {
                    lines.push("   Next: /extensions review".to_string())
                }
                ExternalApplicationPrimaryActionV2::Retry => {
                    lines.push("   Next: /extensions refresh".to_string())
                }
                ExternalApplicationPrimaryActionV2::None
                | ExternalApplicationPrimaryActionV2::View
                | ExternalApplicationPrimaryActionV2::ViewReason => {}
            }
            if application.effective_status == ExternalApplicationEffectiveStatusV2::Connected {
                lines.push(format!("   Disconnect: /extensions disconnect {number}"));
            }
            if application.effective_status == ExternalApplicationEffectiveStatusV2::NeedsAttention
            {
                lines.push(format!("   Later: /extensions defer {number}"));
            }
        }
    }
    if snapshot.review_summary.is_some() && snapshot.host_capabilities.can_read_review {
        lines.push(String::new());
        lines.push("Review: /extensions review".to_string());
    }
    if snapshot.host_capabilities.can_refresh {
        lines.push("Refresh: /extensions refresh".to_string());
    }
    lines.join("\n")
}

fn external_application_review_text(state: &ExternalApplicationUiState) -> Result<String, String> {
    let review = state
        .review
        .as_ref()
        .ok_or_else(|| "Open /extensions review before displaying it.".to_string())?;
    let mut lines = vec![
        "External application review".to_string(),
        String::new(),
        format!("{} items total", review.page.total_count),
    ];
    for (index, item) in review.page.items.iter().enumerate() {
        let selected = state.review_item_selected(index)?;
        lines.push(format!(
            "{}. [{}] {} - {}",
            index + 1,
            if selected { "x" } else { " " },
            item.display_name,
            item.display_summary
        ));
        lines.push(format!(
            "   Baseline: {}",
            if item.recommended {
                "recommended"
            } else {
                "optional"
            }
        ));
    }
    if !review.previous_cursors.is_empty() {
        lines.push("Previous: /extensions review previous".to_string());
    }
    if review.page.next_cursor.is_some() {
        lines.push("Next: /extensions review next".to_string());
    }
    let can_mutate = state.can_mutate().is_ok();
    if can_mutate {
        lines.push("Include: /extensions review include <number>".to_string());
        lines.push("Exclude: /extensions review exclude <number>".to_string());
        lines.push("Apply: /extensions review apply".to_string());
    }
    lines.push("Later (no changes): /extensions review defer".to_string());
    Ok(lines.join("\n"))
}

fn external_control_review_text(
    control: &bitfun_product_domains::external_source_control::ExternalSourceControlSnapshotV1,
) -> String {
    external_control_review_text_impl(control, true)
}

fn external_control_read_only_review_text(
    control: &bitfun_product_domains::external_source_control::ExternalSourceControlSnapshotV1,
) -> String {
    external_control_review_text_impl(control, false)
}

fn external_control_review_text_impl(
    control: &bitfun_product_domains::external_source_control::ExternalSourceControlSnapshotV1,
    include_mutations: bool,
) -> String {
    use bitfun_product_domains::external_source_control::{
        ExternalCapabilityKindV1, ExternalSourceRuntimeState,
    };

    let mut lines = vec![
        "External integrations".to_string(),
        String::new(),
        format!(
            "Safe Mode: {}",
            if control.safe_mode { "on" } else { "off" }
        ),
        format!("Execution domain: {}", control.execution_domain_id),
        format!("Generation: {}", control.refresh_generation),
        format!("Sources: {}", control.sources.len()),
    ];
    if control.safe_mode {
        lines.push(
            "New external Tool, Agent, and MCP calls are blocked; calls already in progress are not cancelled."
                .to_string(),
        );
        lines.push(
            "Safe Mode applies only to this Host process and execution domain; restarting the Host turns it off."
                .to_string(),
        );
    }
    for source in &control.sources {
        let desired = match source.desired {
            ExternalSourceDesiredState::Enabled => "enabled",
            ExternalSourceDesiredState::Disabled => "disabled",
        };
        let effective = match source.effective_status {
            ExternalSourceEffectiveStatus::Discovering => "discovering",
            ExternalSourceEffectiveStatus::Disabled => "disabled",
            ExternalSourceEffectiveStatus::ReviewRequired => "review required",
            ExternalSourceEffectiveStatus::Conflict => "conflict",
            ExternalSourceEffectiveStatus::Active => "active",
            ExternalSourceEffectiveStatus::Degraded => "degraded",
            ExternalSourceEffectiveStatus::Unsupported => "unsupported",
            ExternalSourceEffectiveStatus::Available => "available",
            ExternalSourceEffectiveStatus::Removed => "removed",
        };
        lines.push(format!(
            "Source {}: {} ({desired}, {effective})",
            source.stable_key, source.display_name
        ));
    }
    for capability in &control.capabilities {
        let label = match capability.kind {
            ExternalCapabilityKindV1::Command => "Commands",
            ExternalCapabilityKindV1::Tool => "Tools",
            ExternalCapabilityKindV1::Subagent => "Agents",
            ExternalCapabilityKindV1::Mcp => "MCP servers",
        };
        let runtime = match capability.runtime {
            ExternalSourceRuntimeState::NotApplicable => "not applicable",
            ExternalSourceRuntimeState::Inactive => "inactive",
            ExternalSourceRuntimeState::Starting => "starting",
            ExternalSourceRuntimeState::Active => "active",
            ExternalSourceRuntimeState::Degraded => "degraded",
            ExternalSourceRuntimeState::Quarantined => "quarantined",
            ExternalSourceRuntimeState::Unsupported => "unsupported",
        };
        let support = match capability.support {
            ExternalSourceSupportState::Supported => "",
            ExternalSourceSupportState::Partial => ", support: partial",
            ExternalSourceSupportState::Unsupported => ", support: unsupported",
            ExternalSourceSupportState::Unavailable => ", support: unavailable",
        };
        lines.push(format!(
            "{label}: {} items, {} review, {} conflicts, {runtime}{support}",
            capability.item_count,
            capability.pending_review_count,
            capability.unresolved_conflict_count,
        ));
    }
    const MAX_STATUS_DETAILS: usize = 4;
    if !control.diagnostics.is_empty() {
        lines.push(String::new());
        lines.push("Issues".to_string());
        for diagnostic in control.diagnostics.iter().take(MAX_STATUS_DETAILS) {
            let severity = match diagnostic.severity {
                ExternalSourceDiagnosticSeverity::Info => "info",
                ExternalSourceDiagnosticSeverity::Warning => "warning",
                ExternalSourceDiagnosticSeverity::Error => "error",
                _ => "notice",
            };
            lines.push(format!(
                "  - {severity}: [{}] {}",
                diagnostic.code,
                external_source_diagnostic_summary(&diagnostic.code)
            ));
        }
        let hidden = control.diagnostics.len().saturating_sub(MAX_STATUS_DETAILS);
        if hidden > 0 {
            lines.push(format!(
                "  - {hidden} more; refresh after fixing the listed issue(s)."
            ));
        }
    }
    if include_mutations && !control.recovery_actions.is_empty() {
        lines.push(String::new());
        lines.push("Recovery".to_string());
        for action in control.recovery_actions.iter().take(MAX_STATUS_DETAILS) {
            lines.push(format!(
                "  - {}",
                external_recovery_action_label(action, "extensions")
            ));
        }
    }
    lines.push(String::new());
    lines.push("Refresh: /extensions refresh".to_string());
    if include_mutations {
        lines.push(if control.safe_mode {
            "Exit Safe Mode: /extensions safe-mode off".to_string()
        } else {
            "Enter Safe Mode: /extensions safe-mode on".to_string()
        });
        lines.push("Enable source: /extensions source enable <source-key>".to_string());
        lines.push("Disable source: /extensions source disable <source-key>".to_string());
    } else {
        lines.push(
            "Read-only compatibility status: upgrade or reconnect the Host to manage applications."
                .to_string(),
        );
    }
    lines.join("\n")
}

struct ExternalControlMutationResult {
    action: ExternalControlUiAction,
    result: std::result::Result<
        (
            bitfun_product_domains::external_source_control::ExternalSourceControlSnapshotV1,
            Option<ExternalSourceCatalogSnapshot>,
            Option<ExternalSourceConflictPreferences>,
        ),
        ExternalSourceOperationError,
    >,
}

struct ExternalToolMutationResult {
    action: ExternalToolReviewAction,
    result: std::result::Result<ExternalSourceCatalogSnapshot, ExternalSourceOperationError>,
}

fn external_operation_error_status(surface: &str, error: &ExternalSourceOperationError) -> String {
    let reason = match error.code {
        ExternalSourceOperationErrorCode::InvalidRequest => {
            "The requested change is no longer valid."
        }
        ExternalSourceOperationErrorCode::HostUnavailable => "The workspace host is not available.",
        ExternalSourceOperationErrorCode::HostCapabilityUnavailable => {
            "This workspace host is read-only for external integrations."
        }
        ExternalSourceOperationErrorCode::TrustRequired => {
            "This external integration requires review before it can run."
        }
        ExternalSourceOperationErrorCode::PolicyIncompatible => {
            "Compatibility settings were written by a newer BitFun version."
        }
        ExternalSourceOperationErrorCode::PolicyLimited => {
            "The current safety policy does not allow this change."
        }
        ExternalSourceOperationErrorCode::StaleRevision => {
            "Compatibility settings changed before the update completed."
        }
        ExternalSourceOperationErrorCode::Conflict => {
            "The available choices changed before the update completed."
        }
        ExternalSourceOperationErrorCode::NotFound => "That external item is no longer available.",
        ExternalSourceOperationErrorCode::Unavailable => {
            "The external integration is temporarily unavailable."
        }
        ExternalSourceOperationErrorCode::RuntimeUnavailable
        | ExternalSourceOperationErrorCode::DependencyFailed
        | ExternalSourceOperationErrorCode::ProcessLost => {
            "The external integration runtime is unavailable."
        }
        ExternalSourceOperationErrorCode::Unsupported
        | ExternalSourceOperationErrorCode::IncompatibleVersion => {
            "This external integration is not supported by the current BitFun version."
        }
        ExternalSourceOperationErrorCode::Timeout
        | ExternalSourceOperationErrorCode::Overloaded
        | ExternalSourceOperationErrorCode::TemporarilyUnavailable => {
            "The external integration is temporarily unavailable."
        }
        ExternalSourceOperationErrorCode::Cancelled => {
            "The external integration operation was cancelled."
        }
        ExternalSourceOperationErrorCode::InvalidResponse
        | ExternalSourceOperationErrorCode::Internal => {
            "BitFun could not complete the external integration update."
        }
    };
    let next_steps = error
        .recovery_actions
        .iter()
        .map(|action| external_recovery_action_label(action, surface))
        .collect::<Vec<_>>();
    let next_step = if next_steps.is_empty() {
        format!(" Run /{surface} refresh to review the current state.")
    } else {
        format!(" Next: {}.", next_steps.join("; "))
    };
    let reference = error
        .correlation_id
        .as_deref()
        .map(|id| format!(" Reference: {id}."))
        .unwrap_or_default();
    format!("{reason}{next_step}{reference}")
}

fn external_recovery_action_label(
    action: &ExternalSourceRecoveryActionV1,
    surface: &str,
) -> String {
    match action {
        ExternalSourceRecoveryActionV1::Refresh => format!("/{surface} refresh"),
        ExternalSourceRecoveryActionV1::Retry => "retry the operation".to_string(),
        ExternalSourceRecoveryActionV1::Review => "review the listed external items".to_string(),
        ExternalSourceRecoveryActionV1::ResolveConflict => {
            "resolve the listed conflict".to_string()
        }
        ExternalSourceRecoveryActionV1::InstallRuntime => {
            "install or repair the required runtime".to_string()
        }
        ExternalSourceRecoveryActionV1::ReconnectHost => {
            "reconnect or upgrade the execution Host".to_string()
        }
        ExternalSourceRecoveryActionV1::ExitSafeMode => "/extensions safe-mode off".to_string(),
    }
}

struct ExternalToolTargetSummary<'a> {
    tools: Vec<&'a ExternalToolCatalogEntry>,
}

impl<'a> ExternalToolTargetSummary<'a> {
    fn first(&self) -> &'a ExternalToolCatalogEntry {
        self.tools[0]
    }

    fn activation(&self) -> &'a ExternalToolActivationState {
        &self.first().activation
    }

    fn names(&self) -> String {
        let mut names = self
            .tools
            .iter()
            .map(|tool| tool.definition.name.as_str())
            .collect::<Vec<_>>();
        names.sort_unstable();
        names.dedup();
        names.join(", ")
    }
}

fn external_tool_target_summaries(
    snapshot: &ExternalSourceCatalogSnapshot,
) -> Vec<ExternalToolTargetSummary<'_>> {
    let mut summaries: Vec<ExternalToolTargetSummary<'_>> = Vec::new();
    for tool in &snapshot.tools {
        if let Some(summary) = summaries
            .iter_mut()
            .find(|summary| summary.first().definition.id.target == tool.definition.id.target)
        {
            summary.tools.push(tool);
        } else {
            summaries.push(ExternalToolTargetSummary { tools: vec![tool] });
        }
    }
    summaries
}

fn external_tool_activation_label(activation: &ExternalToolActivationState) -> &'static str {
    match activation {
        ExternalToolActivationState::ApprovalRequired => "confirmation required",
        ExternalToolActivationState::Declined => "kept disabled",
        ExternalToolActivationState::Disabled => "disabled",
        ExternalToolActivationState::Active => "enabled",
        ExternalToolActivationState::Conflict => "choose between same-name tools",
        ExternalToolActivationState::Unsupported { .. } => "not supported",
        ExternalToolActivationState::RuntimeUnavailable { .. } => "run environment unavailable",
        ExternalToolActivationState::LoadFailed { .. } => "could not load",
        _ => "unknown",
    }
}

fn external_tool_scope_label(scope: impl std::fmt::Debug) -> &'static str {
    match format!("{scope:?}").as_str() {
        "UserGlobal" => "all workspaces",
        "Project" | "WorkspaceLocal" => "current workspace",
        "RemoteUser" => "all remote workspaces",
        "RemoteProject" => "current remote workspace",
        _ => "unknown",
    }
}

fn external_tool_user_facing_reason(reason: &str) -> String {
    reason
        .replace("PR2 worker", "Tool process")
        .replace("PR2", "This version")
}

fn external_tool_reason(summary: &ExternalToolTargetSummary<'_>) -> Option<String> {
    match summary.activation() {
        ExternalToolActivationState::Unsupported { reason }
        | ExternalToolActivationState::RuntimeUnavailable { reason }
        | ExternalToolActivationState::LoadFailed { reason } => {
            Some(external_tool_user_facing_reason(reason))
        }
        _ => None,
    }
}

fn external_tool_next_step(activation: &ExternalToolActivationState) -> &'static str {
    match activation {
        ExternalToolActivationState::ApprovalRequired => {
            "Review the code source and access, then enable it or keep it disabled."
        }
        ExternalToolActivationState::Declined => {
            "Enable these tools after reviewing their code source and access."
        }
        ExternalToolActivationState::Disabled => {
            "Enable this source or its tool capability before using these tools."
        }
        ExternalToolActivationState::Active => {
            "No action is needed. Disable these tools to stop using this source's tools."
        }
        ExternalToolActivationState::Conflict => {
            "Choose which tool to use below, or leave this name disabled."
        }
        ExternalToolActivationState::Unsupported { .. } => {
            "Change the code to a single JavaScript file supported by BitFun, then refresh."
        }
        ExternalToolActivationState::RuntimeUnavailable { .. } => {
            "Install or repair Node.js, then refresh. You can continue without external JavaScript tools while the run environment is unavailable."
        }
        ExternalToolActivationState::LoadFailed { .. } => {
            "Refresh to retry. If it still fails, fix the source code or keep these tools disabled."
        }
        _ => "Refresh to check the current state.",
    }
}

fn external_tool_default_reason(activation: &ExternalToolActivationState) -> &'static str {
    match activation {
        ExternalToolActivationState::ApprovalRequired => {
            "Review this tool file's access before enabling it."
        }
        ExternalToolActivationState::Declined => "You chose to keep these tools disabled.",
        ExternalToolActivationState::Disabled => {
            "The source or its tool capability is disabled by policy."
        }
        ExternalToolActivationState::Active => "The tool code is loaded and ready to use.",
        ExternalToolActivationState::Conflict => "Another tool uses the same name.",
        ExternalToolActivationState::Unsupported { .. } => {
            "This tool file contains code or operations that BitFun does not support."
        }
        ExternalToolActivationState::RuntimeUnavailable { .. } => {
            "The required JavaScript run environment is unavailable."
        }
        ExternalToolActivationState::LoadFailed { .. } => "BitFun could not load this tool file.",
        _ => "The current state is unavailable.",
    }
}

fn external_tool_can_enable(activation: &ExternalToolActivationState) -> bool {
    matches!(
        activation,
        ExternalToolActivationState::ApprovalRequired | ExternalToolActivationState::Declined
    )
}

fn external_tool_can_disable(activation: &ExternalToolActivationState) -> bool {
    matches!(
        activation,
        ExternalToolActivationState::ApprovalRequired
            | ExternalToolActivationState::Active
            | ExternalToolActivationState::Conflict
            | ExternalToolActivationState::LoadFailed { .. }
    )
}

fn external_tool_result_is_stale(
    current: Option<&ExternalSourceCatalogSnapshot>,
    incoming: &ExternalSourceCatalogSnapshot,
) -> bool {
    current.is_some_and(|current| current.generation > incoming.generation)
}

fn external_tool_pending_notice_key(snapshot: &ExternalSourceCatalogSnapshot) -> Option<String> {
    let mut decisions = snapshot
        .tool_approval_requests
        .iter()
        .map(|request| format!("approval:{}", request.decision_key))
        .chain(
            snapshot
                .tool_conflicts
                .iter()
                .filter(|conflict| conflict.selected_candidate_id.is_none())
                .map(|conflict| format!("conflict:{}", conflict.conflict_key)),
        )
        .collect::<Vec<_>>();
    decisions.extend(
        snapshot
            .diagnostics
            .iter()
            .filter(|&diagnostic| {
                matches!(
                    diagnostic.severity,
                    ExternalSourceDiagnosticSeverity::Warning
                        | ExternalSourceDiagnosticSeverity::Error
                )
            })
            .map(|diagnostic| {
                format!(
                    "diagnostic:{:?}:{}:{}:{}",
                    diagnostic.severity,
                    diagnostic.code,
                    diagnostic.message,
                    diagnostic
                        .source
                        .as_ref()
                        .map(|source| source.stable_key())
                        .unwrap_or_default()
                )
            }),
    );
    if decisions.is_empty() {
        return None;
    }
    decisions.sort_unstable();
    Some(decisions.join("\n"))
}

fn external_tool_capability_label(capability: ExternalToolCapability) -> &'static str {
    match capability {
        ExternalToolCapability::FileSystem => "filesystem",
        ExternalToolCapability::Network => "network",
        ExternalToolCapability::Process => "process",
        ExternalToolCapability::Environment => "environment variables",
        _ => "other",
    }
}

fn external_tool_runtime_label(runtime: ExternalToolRuntimeKind) -> &'static str {
    match runtime {
        ExternalToolRuntimeKind::JavaScript => "JavaScript",
        ExternalToolRuntimeKind::TypeScript => "TypeScript",
        _ => "unknown runtime",
    }
}

fn external_tool_review_text(snapshot: Option<&ExternalSourceCatalogSnapshot>) -> String {
    let Some(snapshot) = snapshot else {
        return "Tools\n\nBitFun and MCP\nBuilt-in tools are provided by BitFun. Use /mcp to manage MCP servers.\n\nExternal AI applications\nBitFun has not finished checking imported tools. Run /tools refresh and try again."
            .to_string();
    };
    let mut lines = vec![
        "Tools".to_string(),
        String::new(),
        "BitFun and MCP".to_string(),
        "Built-in tools are provided by BitFun. Use /mcp to manage MCP servers.".to_string(),
        String::new(),
        "External AI applications".to_string(),
        "BitFun does not run external code while checking sources. Enabling tools runs their code with your user permissions and inherited environment variables. The code is not isolated by an OS sandbox, and processes it starts may keep running after cancellation."
            .to_string(),
    ];
    lines.push(String::new());
    lines.extend(external_integration_policy_lines(snapshot));

    if snapshot.discovery_pending {
        lines.push(String::new());
        lines.push(
            "BitFun is still checking for changes. Existing tools remain usable.".to_string(),
        );
    }

    lines.push(String::new());
    lines.push("Tool sources".to_string());
    let targets = external_tool_target_summaries(snapshot);
    if targets.is_empty() {
        lines.push("  None".to_string());
    } else {
        for (index, target) in targets.iter().enumerate() {
            let tool = target.first();
            let source = snapshot
                .sources
                .iter()
                .find(|source| source.record.key == tool.definition.id.target.source);
            let capabilities = target
                .tools
                .iter()
                .flat_map(|tool| tool.definition.capabilities.iter().copied())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .map(external_tool_capability_label)
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!(
                "  {}. {} - {}",
                index + 1,
                target.names(),
                external_tool_activation_label(target.activation())
            ));
            lines.push(format!(
                "     Source folder: {}",
                source
                    .map(|source| source.record.location.as_str())
                    .unwrap_or("unknown")
            ));
            lines.push("     Code files:".to_string());
            let module_paths = target
                .tools
                .iter()
                .map(|tool| tool.definition.module_path.as_str())
                .collect::<BTreeSet<_>>();
            for module_path in module_paths {
                lines.push(format!("       - {module_path}"));
            }
            lines.push(format!(
                "     Applies to: {}",
                source
                    .map(|source| external_tool_scope_label(source.record.scope))
                    .unwrap_or("unknown")
            ));
            lines.push(format!(
                "     Runs in: {}",
                source
                    .map(|source| external_tool_run_location_label(
                        source.record.execution_domain_id.as_str(),
                    ))
                    .unwrap_or("unknown")
            ));
            lines.push(format!(
                "     Starts in folder: {}",
                tool.definition.working_directory
            ));
            lines.push(format!(
                "     Runs with: {}",
                external_tool_runtime_label(tool.definition.runtime_kind)
            ));
            lines.push(format!("     Access: {capabilities}"));
            if let Some(reason) = external_tool_reason(target) {
                lines.push(format!("     Reason: {reason}"));
            } else {
                lines.push(format!(
                    "     Reason: {}",
                    external_tool_default_reason(target.activation())
                ));
            }
            lines.push(format!(
                "     Next step: {}",
                external_tool_next_step(target.activation())
            ));
            let mut commands = Vec::new();
            if external_tool_can_enable(target.activation()) {
                commands.push(format!("/tools enable {}", index + 1));
            }
            if external_tool_can_disable(target.activation()) {
                commands.push(format!("/tools disable {}", index + 1));
            }
            if !commands.is_empty() {
                lines.push(format!("     Commands: {}", commands.join("  or  ")));
            }
        }
    }

    lines.push(String::new());
    lines.push("Name conflicts - needs a choice".to_string());
    let conflicts = snapshot
        .tool_conflicts
        .iter()
        .filter(|conflict| conflict.selected_candidate_id.is_none())
        .chain(
            snapshot
                .tool_conflicts
                .iter()
                .filter(|conflict| conflict.selected_candidate_id.is_some()),
        )
        .collect::<Vec<_>>();
    let pending_count = conflicts
        .iter()
        .take_while(|conflict| conflict.selected_candidate_id.is_none())
        .count();
    let pending_conflicts = &conflicts[..pending_count];
    if pending_conflicts.is_empty() {
        lines.push("  None".to_string());
    } else {
        for (conflict_index, conflict) in pending_conflicts.iter().enumerate() {
            lines.push(format!(
                "  {}. Multiple tools are named '{}':",
                conflict_index + 1,
                conflict.tool_name
            ));
            for (candidate_index, candidate) in conflict.candidates.iter().enumerate() {
                lines.push(format!(
                    "     {}. {} - /tools choose {} {}",
                    candidate_index + 1,
                    candidate.display_name,
                    conflict_index + 1,
                    candidate_index + 1
                ));
            }
            lines.push(
                "     Choose which tool BitFun should use for this name. The choice is remembered until one of these tools changes."
                    .to_string(),
            );
        }
    }

    lines.push(String::new());
    lines.push("Current choices".to_string());
    let resolved_conflicts = &conflicts[pending_count..];
    if resolved_conflicts.is_empty() {
        lines.push("  None".to_string());
    } else {
        for (resolved_index, conflict) in resolved_conflicts.iter().enumerate() {
            let conflict_index = pending_count + resolved_index;
            lines.push(format!(
                "  {}. Tools named '{}':",
                conflict_index + 1,
                conflict.tool_name
            ));
            for (candidate_index, candidate) in conflict.candidates.iter().enumerate() {
                let status = if conflict.selected_candidate_id.as_deref()
                    == Some(candidate.candidate_id.as_str())
                {
                    let selected_external_unavailable = candidate.source.is_some()
                        && !snapshot.tools.iter().any(|tool| {
                            tool.definition.candidate_id() == candidate.candidate_id
                                && tool.activation == ExternalToolActivationState::Active
                        });
                    if selected_external_unavailable {
                        "selected, currently unavailable"
                    } else {
                        "selected"
                    }
                } else {
                    "not selected"
                };
                lines.push(format!(
                    "     {}. {} [{}] - /tools choose {} {}",
                    candidate_index + 1,
                    candidate.display_name,
                    status,
                    conflict_index + 1,
                    candidate_index + 1
                ));
            }
            lines.push(
                "     This choice is remembered until one of these tools changes. Choose another entry above to change it."
                    .to_string(),
            );
        }
    }

    append_external_source_issues(&mut lines, snapshot, ExternalIssueSurface::Tools);

    lines.push(String::new());
    lines.push(
        "Use /tools refresh after editing, upgrading, or removing tools from an external AI application."
            .to_string(),
    );
    lines.join("\n")
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExternalAgentReviewAction {
    Show,
    Refresh,
    Decide {
        candidate_id: String,
        decision_key: String,
        approved: bool,
        expected_subagent_generation: u64,
        expected_preference_revision: u64,
    },
    Bind {
        binding_key: String,
        target: Option<ExternalSubagentModelBindingTarget>,
        expected_subagent_generation: u64,
        expected_preference_revision: u64,
    },
    Choose {
        conflict_key: String,
        candidate_id: String,
        approve_external: bool,
        expected_subagent_generation: u64,
        expected_preference_revision: u64,
    },
}

struct ExternalAgentMutationResult {
    action: ExternalAgentReviewAction,
    result: std::result::Result<ExternalSourceCatalogSnapshot, ExternalSourceOperationError>,
}

fn external_tool_run_location_label(execution_domain_id: &str) -> &'static str {
    if execution_domain_id.starts_with("local") {
        "this computer"
    } else if execution_domain_id.starts_with("remote") {
        "current remote environment"
    } else {
        "unknown"
    }
}

fn external_source_diagnostic_summary(code: &str) -> &'static str {
    if code.contains("preference_read_failed") {
        "BitFun could not verify saved tool confirmations. Affected tools remain disabled; check BitFun settings storage, then refresh."
    } else if code.contains("conflict_history_write_failed") {
        "BitFun could not save conflict information. Affected names remain unavailable; check BitFun settings storage, then refresh."
    } else if code.contains("discovery_in_progress") {
        "One source is still being checked. Existing content remains available."
    } else if code.contains("timeout") {
        "Checking one source took too long. Other content remains available; refresh to try again."
    } else if code.contains("trust_required") {
        "A source needs your confirmation before BitFun can use it."
    } else if code.contains("too_large")
        || code.contains("file_limit")
        || code.contains("bytes_limit")
    {
        "Some files were skipped because the source is too large. Reduce its size, then refresh."
    } else if code.contains("invalid")
        || code.contains("parse")
        || code.contains("definition")
        || code.contains("export_missing")
        || code.contains("name_unsupported")
    {
        "Some settings could not be read and were skipped. Fix the source, then refresh."
    } else if code.contains("unreadable")
        || code.contains("read_failed")
        || code.contains("metadata_failed")
        || code.contains("directory_")
    {
        "BitFun could not read part of a source. Check file access, then refresh."
    } else if code.contains("projection_only")
        || code.contains("unsupported")
        || code.contains("restricted")
    {
        "This type of external content is not supported yet, so BitFun did not load or run it."
    } else if code.contains("failed") {
        "BitFun could not check one source. Other sources remain available; refresh to retry."
    } else {
        "BitFun found an issue in one source. The affected content was not enabled."
    }
}

#[derive(Clone, Copy)]
enum ExternalIssueSurface {
    Tools,
    Agents,
}

fn is_external_agent_diagnostic(
    diagnostic: &bitfun_product_domains::external_sources::ExternalSourceDiagnostic,
) -> bool {
    matches!(diagnostic.asset_kind, ExternalSourceAssetKind::Subagent)
}

fn append_external_source_issues(
    lines: &mut Vec<String>,
    snapshot: &ExternalSourceCatalogSnapshot,
    surface: ExternalIssueSurface,
) {
    let diagnostics = snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| match surface {
            ExternalIssueSurface::Tools => !is_external_agent_diagnostic(diagnostic),
            ExternalIssueSurface::Agents => is_external_agent_diagnostic(diagnostic),
        })
        .collect::<Vec<_>>();
    lines.push(String::new());
    lines.push("Issues".to_string());
    if diagnostics.is_empty() {
        lines.push("  None".to_string());
        return;
    }
    for diagnostic in diagnostics {
        let severity = match diagnostic.severity {
            ExternalSourceDiagnosticSeverity::Info => "info",
            ExternalSourceDiagnosticSeverity::Warning => "warning",
            ExternalSourceDiagnosticSeverity::Error => "error",
            _ => "notice",
        };
        let source = diagnostic
            .source
            .as_ref()
            .and_then(|key| {
                snapshot
                    .sources
                    .iter()
                    .find(|source| source.record.key == *key)
            })
            .map(|source| source.record.display_name.as_str());
        lines.push(format!(
            "  - {severity}: {}",
            external_source_diagnostic_summary(&diagnostic.code)
        ));
        if let Some(source) = source {
            lines.push(format!("    Affected source: {source}"));
        }
        lines.push(format!(
            "    Technical details: [{}] {}",
            diagnostic.code,
            external_tool_user_facing_reason(&diagnostic.message)
        ));
    }
}

const DISABLED_EXTERNAL_AGENT_CONFLICT_CHOICE: &str = "__bitfun_disabled__";

fn external_agent_activation_label(state: &ExternalSubagentActivationState) -> &'static str {
    match state {
        ExternalSubagentActivationState::ApprovalRequired => "confirmation required",
        ExternalSubagentActivationState::Declined => "kept disabled",
        ExternalSubagentActivationState::Disabled => "disabled by source",
        ExternalSubagentActivationState::Active => "enabled",
        ExternalSubagentActivationState::Conflict => "choose between same-name agents",
        ExternalSubagentActivationState::Blocked => "not supported",
        ExternalSubagentActivationState::Unavailable => "temporarily unavailable",
    }
}

fn external_agent_compatibility_label(state: ExternalSubagentCompatibilityState) -> &'static str {
    match state {
        ExternalSubagentCompatibilityState::Ready => "supported",
        ExternalSubagentCompatibilityState::ReadyWithDegradation => {
            "supported, but some settings will not apply"
        }
        ExternalSubagentCompatibilityState::Blocked => "not supported",
        ExternalSubagentCompatibilityState::Invalid => "configuration error",
    }
}

fn external_agent_model_label(
    model: Option<&str>,
    method: ExternalSubagentModelBindingMethod,
) -> &str {
    model.unwrap_or(match method {
        ExternalSubagentModelBindingMethod::Inherit => {
            "resolved from the parent session when the task starts"
        }
        _ => "unavailable",
    })
}

fn external_agent_model_request_label(request: &ExternalSubagentModelRequest) -> String {
    match request {
        ExternalSubagentModelRequest::Default => "BitFun default".to_string(),
        ExternalSubagentModelRequest::Inherit => "parent session model".to_string(),
        ExternalSubagentModelRequest::Reference {
            provider_hint,
            model_name,
        } => provider_hint
            .as_ref()
            .map(|provider| format!("{provider}/{model_name}"))
            .unwrap_or_else(|| model_name.clone()),
    }
}

fn external_agent_model_profile_label(request: &ExternalSubagentModelProfileRequest) -> String {
    match request {
        ExternalSubagentModelProfileRequest::NamedVariant { name } => {
            format!("named variant {name}")
        }
        ExternalSubagentModelProfileRequest::ReasoningEffort { value } => {
            format!("reasoning effort {value}")
        }
    }
}

fn external_agent_model_binding_method_label(
    method: ExternalSubagentModelBindingMethod,
) -> &'static str {
    match method {
        ExternalSubagentModelBindingMethod::Default => "BitFun default",
        ExternalSubagentModelBindingMethod::Inherit => "inherited from the parent session",
        ExternalSubagentModelBindingMethod::Exact => "exact configured model",
        ExternalSubagentModelBindingMethod::Explicit => "user binding",
        ExternalSubagentModelBindingMethod::BindingRequired => "choose a BitFun model",
        ExternalSubagentModelBindingMethod::BindingUnavailable => {
            "saved BitFun model is unavailable"
        }
    }
}

fn external_agent_model_binding_target_label(
    target: &ExternalSubagentModelBindingTarget,
) -> &'static str {
    match target {
        ExternalSubagentModelBindingTarget::Primary => "primary model",
        ExternalSubagentModelBindingTarget::Fast => "fast model",
        ExternalSubagentModelBindingTarget::Model { .. } => "configured model",
    }
}

fn external_agent_review_text(snapshot: Option<&ExternalSourceCatalogSnapshot>) -> String {
    let Some(snapshot) = snapshot else {
        return "Agents\n\nExternal AI applications\nBitFun has not finished checking imported agents. Run /agent refresh and try again."
            .to_string();
    };
    let mut lines = vec![
        "Agents".to_string(),
        String::new(),
        "External AI applications".to_string(),
        "BitFun only reads supported settings while checking sources. Agent instructions stay hidden and are not added to the current agent. Once enabled, those instructions guide the selected model and may call the tools listed below. Before enabling, review the model, tools, and where the agent runs. BitFun asks again if the instructions, model, tools, or configuration sources change. Each use starts a new task; follow-up is not supported in this version."
            .to_string(),
    ];
    lines.push(String::new());
    lines.extend(external_integration_policy_lines(snapshot));
    if snapshot.discovery_pending {
        lines.push(String::new());
        lines.push(
            "BitFun is still checking for changes. Previously enabled agents remain usable."
                .to_string(),
        );
    }

    append_external_source_issues(&mut lines, snapshot, ExternalIssueSurface::Agents);

    lines.push(String::new());
    lines.push("Model bindings".to_string());
    if snapshot.subagent_model_binding_groups.is_empty() {
        lines.push("  None".to_string());
    } else {
        for (binding_index, binding) in snapshot.subagent_model_binding_groups.iter().enumerate() {
            lines.push(format!(
                "  {}. {} - {}",
                binding_index + 1,
                external_agent_model_request_label(&binding.request),
                external_agent_model_binding_method_label(binding.method)
            ));
            if let Some(profile) = &binding.profile_request {
                lines.push(format!(
                    "     Requested profile: {}",
                    external_agent_model_profile_label(profile)
                ));
            }
            lines.push(format!(
                "     Affects {} agents; effective model: {}",
                binding.affected_candidate_ids.len(),
                external_agent_model_label(
                    binding.effective_model_label.as_deref(),
                    binding.method,
                )
            ));
            if !matches!(
                binding.method,
                ExternalSubagentModelBindingMethod::BindingRequired
                    | ExternalSubagentModelBindingMethod::Explicit
                    | ExternalSubagentModelBindingMethod::BindingUnavailable
            ) {
                lines.push("     Matched automatically; no binding is needed.".to_string());
                continue;
            }
            lines.push(format!(
                "     0. Automatic source matching{} - /agent bind {} 0",
                if binding.selected_target.is_none() {
                    " [selected]"
                } else {
                    ""
                },
                binding_index + 1
            ));
            for (choice_index, option) in snapshot.subagent_model_binding_options.iter().enumerate()
            {
                lines.push(format!(
                    "     {}. {} ({}){}{} - /agent bind {} {}",
                    choice_index + 1,
                    option.effective_model_label,
                    external_agent_model_binding_target_label(&option.target),
                    option
                        .configured_reasoning_effort
                        .as_deref()
                        .map(|value| format!(", configured effort: {value}"))
                        .unwrap_or_default(),
                    if binding.selected_target.as_ref() == Some(&option.target) {
                        " [selected]"
                    } else {
                        ""
                    },
                    binding_index + 1,
                    choice_index + 1
                ));
            }
            if let Some(target) = binding.selected_target.as_ref().filter(|target| {
                !snapshot
                    .subagent_model_binding_options
                    .iter()
                    .any(|option| &option.target == *target)
            }) {
                lines.push(format!(
                    "     Saved {} is unavailable; choose another entry or clear the binding.",
                    external_agent_model_binding_target_label(target)
                ));
            }
        }
    }

    lines.push(String::new());
    lines.push("Agents".to_string());
    if snapshot.subagents.is_empty() {
        lines.push("  None".to_string());
    } else {
        for (index, agent) in snapshot.subagents.iter().enumerate() {
            lines.push(format!(
                "  {}. {} ({}) - {}",
                index + 1,
                agent.display_name,
                agent.logical_id,
                external_agent_activation_label(&agent.activation_state)
            ));
            lines.push(format!("     Source application: {}", agent.provider_label));
            lines.push(format!(
                "     Applies to: {}",
                external_tool_scope_label(agent.scope)
            ));
            if !agent.source_location_labels.is_empty() {
                lines.push(format!(
                    "     Configuration sources: {}",
                    agent.source_location_labels.join(", ")
                ));
            }
            lines.push(format!(
                "     Requested model: {}",
                external_agent_model_request_label(&agent.requested_model)
            ));
            if let Some(profile) = &agent.requested_model_profile {
                lines.push(format!(
                    "     Requested profile: {}",
                    external_agent_model_profile_label(profile)
                ));
            }
            lines.push(format!(
                "     Resolution: {}",
                external_agent_model_binding_method_label(agent.model_binding_method)
            ));
            lines.push(format!(
                "     Model: {}",
                external_agent_model_label(
                    agent.effective_model_label.as_deref(),
                    agent.model_binding_method,
                )
            ));
            lines.push(format!(
                "     Tools: {}",
                if agent.effective_tool_labels.is_empty() {
                    "none".to_string()
                } else {
                    agent.effective_tool_labels.join(", ")
                }
            ));
            lines.push(format!(
                "     Support: {}",
                external_agent_compatibility_label(agent.compatibility_state)
            ));
            lines.push("     Run behavior: one run only; no follow-up".to_string());
            lines.push("     Runs on: this computer in the current workspace".to_string());
            if !agent.diagnostics.is_empty() {
                lines.push("     Compatibility notes:".to_string());
                for diagnostic in &agent.diagnostics {
                    lines.extend(external_agent_diagnostic_lines(
                        &diagnostic.code,
                        diagnostic.blocks_activation,
                        "       ",
                    ));
                }
            }
            match agent.activation_state {
                ExternalSubagentActivationState::ApprovalRequired
                | ExternalSubagentActivationState::Declined => {
                    lines.push(format!("     Command: /agent enable {}", index + 1))
                }
                ExternalSubagentActivationState::Active => {
                    lines.push(format!("     Command: /agent disable {}", index + 1))
                }
                _ => {}
            }
        }
    }

    lines.push(String::new());
    lines.push("Name conflicts - needs a choice".to_string());
    let conflicts = snapshot
        .subagent_conflicts
        .iter()
        .filter(|conflict| conflict.selected_candidate_id.is_none())
        .chain(
            snapshot
                .subagent_conflicts
                .iter()
                .filter(|conflict| conflict.selected_candidate_id.is_some()),
        )
        .collect::<Vec<_>>();
    let pending_count = conflicts
        .iter()
        .take_while(|conflict| conflict.selected_candidate_id.is_none())
        .count();
    let pending_conflicts = &conflicts[..pending_count];
    if pending_conflicts.is_empty() {
        lines.push("  None".to_string());
    } else {
        for (conflict_index, conflict) in pending_conflicts.iter().enumerate() {
            lines.push(format!(
                "  {}. Multiple agents are named '{}'. Choose one:",
                conflict_index + 1,
                conflict.logical_id
            ));
            for (candidate_index, candidate) in conflict.candidates.iter().enumerate() {
                let kind = if candidate.external {
                    "external"
                } else {
                    "BitFun/local"
                };
                lines.push(format!(
                    "     {}. {} ({}, {}) - /agent choose {} {}",
                    candidate_index + 1,
                    candidate.display_name,
                    candidate.source_label,
                    kind,
                    conflict_index + 1,
                    candidate_index + 1
                ));
                if candidate.external {
                    if let Some(agent) = snapshot
                        .subagents
                        .iter()
                        .find(|agent| agent.candidate_id == candidate.candidate_id)
                    {
                        lines.push(format!(
                            "        Model: {}",
                            external_agent_model_label(
                                agent.effective_model_label.as_deref(),
                                agent.model_binding_method,
                            )
                        ));
                        lines.push(format!(
                            "        Tools: {}",
                            if agent.effective_tool_labels.is_empty() {
                                "none".to_string()
                            } else {
                                agent.effective_tool_labels.join(", ")
                            }
                        ));
                        lines.push(
                            "        Runs on: this computer in the current workspace".to_string(),
                        );
                        lines.push(format!(
                            "        Support: {}",
                            external_agent_compatibility_label(agent.compatibility_state)
                        ));
                        for location in &agent.source_location_labels {
                            lines.push(format!("        Source: {location}"));
                        }
                        for diagnostic in &agent.diagnostics {
                            lines.extend(external_agent_diagnostic_lines(
                                &diagnostic.code,
                                diagnostic.blocks_activation,
                                "        ",
                            ));
                        }
                        lines.push(
                            "        This choice also confirms the model, tools, run location, and configuration sources shown above."
                                .to_string(),
                        );
                    }
                }
            }
            lines.push(format!(
                "     Keep unavailable: /agent choose {} 0",
                conflict_index + 1
            ));
            lines.push(
                "     The choice is remembered until one of these agents changes.".to_string(),
            );
        }
    }

    lines.push(String::new());
    lines.push("Current choices".to_string());
    let resolved_conflicts = &conflicts[pending_count..];
    if resolved_conflicts.is_empty() {
        lines.push("  None".to_string());
    } else {
        for (resolved_index, conflict) in resolved_conflicts.iter().enumerate() {
            let conflict_index = pending_count + resolved_index;
            lines.push(format!(
                "  {}. Agents named '{}':",
                conflict_index + 1,
                conflict.logical_id
            ));
            for (candidate_index, candidate) in conflict.candidates.iter().enumerate() {
                let kind = if candidate.external {
                    "external"
                } else {
                    "BitFun/local"
                };
                let status = if conflict.selected_candidate_id.as_deref()
                    == Some(candidate.candidate_id.as_str())
                {
                    if candidate.external
                        && snapshot.subagents.iter().any(|agent| {
                            agent.candidate_id == candidate.candidate_id
                                && agent.activation_state != ExternalSubagentActivationState::Active
                        })
                    {
                        "selected, currently unavailable"
                    } else {
                        "selected"
                    }
                } else {
                    "not selected"
                };
                lines.push(format!(
                    "     {}. {} ({}, {}) [{}] - /agent choose {} {}",
                    candidate_index + 1,
                    candidate.display_name,
                    candidate.source_label,
                    kind,
                    status,
                    conflict_index + 1,
                    candidate_index + 1
                ));
            }
            let disabled = conflict.selected_candidate_id.as_deref()
                == Some(DISABLED_EXTERNAL_AGENT_CONFLICT_CHOICE);
            lines.push(format!(
                "     Keep unavailable{}: /agent choose {} 0",
                if disabled {
                    " [selected]"
                } else {
                    " [not selected]"
                },
                conflict_index + 1
            ));
            lines.push(
                "     This choice is remembered until one of these agents changes. Choose another entry above to change it."
                    .to_string(),
            );
        }
    }

    lines.push(String::new());
    lines.push(
        "Run /agent refresh after editing, upgrading, or removing agent configuration in an external AI application."
            .to_string(),
    );
    lines.join("\n")
}

fn external_agent_diagnostic_lines(
    code: &str,
    blocks_activation: bool,
    indent: &str,
) -> Vec<String> {
    let (reason, next_step) = if code.contains("configuration_unavailable") {
        (
            "BitFun could not read its model settings.",
            "Open BitFun model settings, check that BitFun can read and save its settings, then refresh.",
        )
    } else if code.contains("model_unavailable") {
        (
            "The requested model is not available in BitFun.",
            "Choose an available model in the source application, or set a fixed Sub-Agent model in BitFun, then refresh.",
        )
    } else if code.contains("tool_unavailable") {
        (
            "One or more requested tools are not available in BitFun.",
            "Remove or replace the unsupported tools in the source application, then refresh.",
        )
    } else if code.contains("type_invalid")
        || code.contains("definition_invalid")
        || code.ends_with("_invalid")
    {
        (
            "The agent settings have an invalid or missing required value.",
            "Correct the agent settings in the source application, then refresh.",
        )
    } else if blocks_activation {
        (
            "This agent requires behavior or settings that BitFun does not support.",
            "Update the agent in the source application to use supported settings and include all required content, then refresh.",
        )
    } else {
        (
            "BitFun does not use this setting.",
            "Before enabling, review the model and tools that will actually be used, and confirm that this setting will not apply.",
        )
    };
    let impact = if blocks_activation {
        "This agent cannot be enabled."
    } else {
        "Some settings will not apply. Review the resulting behavior before enabling."
    };
    vec![
        format!("{indent}Reason: {reason}"),
        format!("{indent}Impact: {impact}"),
        format!("{indent}Next step: {next_step}"),
        format!("{indent}Technical code: {code}"),
    ]
}

fn external_agent_result_is_stale(
    current: Option<&ExternalSourceCatalogSnapshot>,
    result: &ExternalSourceCatalogSnapshot,
) -> bool {
    current.is_some_and(|current| {
        current.subagent_generation > result.subagent_generation
            || current.preference_revision > result.preference_revision
    })
}

fn merge_external_agent_mutation_snapshot(
    current: Option<&ExternalSourceCatalogSnapshot>,
    mut result: ExternalSourceCatalogSnapshot,
) -> ExternalSourceCatalogSnapshot {
    let Some(current) = current else {
        return result;
    };
    if current.generation <= result.generation {
        return result;
    }

    // Agent decisions have an independent generation/revision. Preserve a
    // newer unrelated command/tool catalog while applying only the returned
    // agent partition, so a successful review action cannot roll the TUI back.
    let mut merged = current.clone();
    merged.subagent_generation = result.subagent_generation;
    merged.preference_revision = result.preference_revision;
    merged.subagents = std::mem::take(&mut result.subagents);
    merged.subagent_model_binding_groups =
        std::mem::take(&mut result.subagent_model_binding_groups);
    merged.subagent_model_binding_options =
        std::mem::take(&mut result.subagent_model_binding_options);
    merged.subagent_conflicts = std::mem::take(&mut result.subagent_conflicts);
    merged.pending_subagent_approvals = std::mem::take(&mut result.pending_subagent_approvals);
    merged
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExternalAgentAttention {
    bindings: usize,
    confirmations: usize,
    conflicts: usize,
    unavailable: usize,
    diagnostics: usize,
    key: Option<String>,
}

fn external_agent_attention(
    previous: Option<&ExternalSourceCatalogSnapshot>,
    snapshot: &ExternalSourceCatalogSnapshot,
) -> ExternalAgentAttention {
    let unresolved = snapshot
        .subagent_conflicts
        .iter()
        .filter(|conflict| conflict.selected_candidate_id.is_none())
        .map(|conflict| conflict.conflict_key.clone())
        .collect::<Vec<_>>();
    let pending_decisions = snapshot
        .pending_subagent_approvals
        .iter()
        .map(|candidate_id| {
            snapshot
                .subagents
                .iter()
                .find(|agent| agent.candidate_id == *candidate_id)
                .map(|agent| format!("{}:{}", agent.candidate_id, agent.decision_key))
                .unwrap_or_else(|| candidate_id.clone())
        })
        .collect::<Vec<_>>();
    let unavailable = previous
        .into_iter()
        .flat_map(|previous| previous.subagents.iter())
        .filter(|agent| agent.activation_state == ExternalSubagentActivationState::Active)
        .filter_map(|previous_agent| {
            match snapshot
                .subagents
                .iter()
                .find(|agent| agent.candidate_id == previous_agent.candidate_id)
                .map(|agent| &agent.activation_state)
            {
                None => Some(format!("removed:{}", previous_agent.candidate_id)),
                Some(ExternalSubagentActivationState::Active) => None,
                Some(state) => Some(format!("{state:?}:{}", previous_agent.candidate_id)),
            }
        })
        .collect::<BTreeSet<_>>();
    let diagnostics = snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            matches!(
                diagnostic.severity,
                ExternalSourceDiagnosticSeverity::Warning | ExternalSourceDiagnosticSeverity::Error
            ) && is_external_agent_diagnostic(diagnostic)
        })
        .map(|diagnostic| {
            format!(
                "{:?}:{}:{}",
                diagnostic.severity, diagnostic.code, diagnostic.message
            )
        })
        .collect::<Vec<_>>();
    let confirmations = snapshot.pending_subagent_approvals.len();
    let binding_keys = snapshot
        .subagent_model_binding_groups
        .iter()
        .filter(|binding| {
            matches!(
                binding.method,
                ExternalSubagentModelBindingMethod::BindingRequired
                    | ExternalSubagentModelBindingMethod::BindingUnavailable
            )
        })
        .map(|binding| binding.binding_key.as_str())
        .collect::<Vec<_>>();
    let bindings = binding_keys.len();
    let conflicts = unresolved.len();
    let unavailable_count = unavailable.len();
    let diagnostic_count = diagnostics.len();
    let key = if bindings + confirmations + conflicts + unavailable_count + diagnostic_count == 0 {
        None
    } else {
        Some(format!(
            "bindings={};approvals={};conflicts={};unavailable={};diagnostics={}",
            binding_keys.join(","),
            pending_decisions.join(","),
            unresolved.join(","),
            unavailable.into_iter().collect::<Vec<_>>().join(","),
            diagnostics.join(",")
        ))
    };
    ExternalAgentAttention {
        bindings,
        confirmations,
        conflicts,
        unavailable: unavailable_count,
        diagnostics: diagnostic_count,
        key,
    }
}

fn external_agent_pending_notice_key(
    previous: Option<&ExternalSourceCatalogSnapshot>,
    snapshot: &ExternalSourceCatalogSnapshot,
) -> Option<String> {
    external_agent_attention(previous, snapshot).key
}

#[cfg(test)]
mod external_application_v2_tests {
    use super::*;
    use bitfun_product_domains::external_source_control::{
        ExternalApplicationConnectionStateV2, ExternalApplicationDefaultConnectionPolicyV2,
        ExternalApplicationDesiredConnectionV2, ExternalApplicationDiscoveryStateV2,
        ExternalApplicationEffectiveStatusV2, ExternalApplicationHealthV2,
        ExternalApplicationHostCapabilitiesV2, ExternalApplicationOwnerGenerationV2,
        ExternalApplicationPrimaryActionV2, ExternalApplicationReviewCategoryCountV2,
        ExternalApplicationReviewItemKindV2, ExternalApplicationReviewItemRefV2,
        ExternalApplicationReviewItemV2, ExternalApplicationReviewPageV2,
        ExternalApplicationReviewRecommendationSummaryV2, ExternalApplicationReviewSummaryV2,
        ExternalApplicationRiskLevelV2, ExternalApplicationRiskSummaryV2,
        ExternalApplicationSafetyCeilingV2, ExternalApplicationSnapshotV2,
        ExternalApplicationSummaryV2, ExternalApplicationTargetScopeV2,
        ExternalApplicationUserDecisionV2, EXTERNAL_APPLICATION_SCHEMA_V2,
    };
    use bitfun_product_domains::external_sources::ExecutionDomainId;

    fn risk() -> ExternalApplicationRiskSummaryV2 {
        ExternalApplicationRiskSummaryV2 {
            highest_level: Some(ExternalApplicationRiskLevelV2::High),
            reason_codes: vec!["process_execution".to_string()],
        }
    }

    fn application(
        id: &str,
        status: ExternalApplicationEffectiveStatusV2,
        action: ExternalApplicationPrimaryActionV2,
    ) -> ExternalApplicationSummaryV2 {
        ExternalApplicationSummaryV2 {
            application_id: id.to_string(),
            ecosystem_id: id.to_string(),
            display_name: id.to_string(),
            discovery: if status == ExternalApplicationEffectiveStatusV2::NoConfiguration {
                ExternalApplicationDiscoveryStateV2::NotDiscovered
            } else {
                ExternalApplicationDiscoveryStateV2::Discovered
            },
            connection: if status == ExternalApplicationEffectiveStatusV2::Connected {
                ExternalApplicationConnectionStateV2::Connected
            } else {
                ExternalApplicationConnectionStateV2::Disconnected
            },
            desired_connection: ExternalApplicationDesiredConnectionV2::Unspecified,
            health: ExternalApplicationHealthV2::Healthy,
            effective_status: status,
            primary_action: action,
            default_connection_policy: ExternalApplicationDefaultConnectionPolicyV2::DiscoverOnly,
            default_connection_reason: "product_policy".to_string(),
            enabled_count: 1,
            pending_review_count: usize::from(
                status == ExternalApplicationEffectiveStatusV2::NeedsAttention,
            ),
            blocked_count: 0,
            conflict_count: 0,
            risk_summary: risk(),
            notice_key: None,
            user_decision: ExternalApplicationUserDecisionV2::None,
            recovery_actions: Vec::new(),
        }
    }

    fn snapshot(
        capabilities: ExternalApplicationHostCapabilitiesV2,
    ) -> ExternalApplicationSnapshotV2 {
        ExternalApplicationSnapshotV2 {
            schema_version: EXTERNAL_APPLICATION_SCHEMA_V2,
            execution_domain_id: ExecutionDomainId::new("host-a").unwrap(),
            workspace_scope_id: Some("workspace:0123456789abcdef".to_string()),
            effective_connection_scope: ExternalApplicationTargetScopeV2::WorkspaceOverride,
            refresh_generation: 7,
            preference_revision: 11,
            safe_mode: true,
            host_capabilities: capabilities,
            applications: vec![
                application(
                    "connected",
                    ExternalApplicationEffectiveStatusV2::Connected,
                    ExternalApplicationPrimaryActionV2::View,
                ),
                application(
                    "available",
                    ExternalApplicationEffectiveStatusV2::ConfigurationAvailable,
                    ExternalApplicationPrimaryActionV2::Connect,
                ),
                application(
                    "missing",
                    ExternalApplicationEffectiveStatusV2::NoConfiguration,
                    ExternalApplicationPrimaryActionV2::None,
                ),
                application(
                    "attention",
                    ExternalApplicationEffectiveStatusV2::NeedsAttention,
                    ExternalApplicationPrimaryActionV2::Review,
                ),
                application(
                    "unavailable",
                    ExternalApplicationEffectiveStatusV2::TemporarilyUnavailable,
                    ExternalApplicationPrimaryActionV2::Retry,
                ),
            ],
            review_summary: Some(ExternalApplicationReviewSummaryV2 {
                review_id: "review-7".to_string(),
                total_count: 2,
                category_counts: vec![ExternalApplicationReviewCategoryCountV2 {
                    kind: ExternalApplicationReviewItemKindV2::Tool,
                    count: 2,
                }],
                max_selection_count: 2,
                risk_summary: risk(),
                recommendation_summary: ExternalApplicationReviewRecommendationSummaryV2 {
                    recommended_count: 1,
                    optional_count: 1,
                    blocked_count: 0,
                },
                safety_ceiling: ExternalApplicationSafetyCeilingV2::ReviewRequired,
            }),
        }
    }

    fn item(stable_id: &str, recommended: bool) -> ExternalApplicationReviewItemV2 {
        ExternalApplicationReviewItemV2 {
            item_ref: ExternalApplicationReviewItemRefV2 {
                kind: ExternalApplicationReviewItemKindV2::Tool,
                stable_id: stable_id.to_string(),
            },
            display_name: stable_id.to_string(),
            display_summary: "Runs an external tool".to_string(),
            risk_level: ExternalApplicationRiskLevelV2::High,
            risk_reason_codes: vec!["process_execution".to_string()],
            recommended,
            safety_ceiling: ExternalApplicationSafetyCeilingV2::ReviewRequired,
        }
    }

    fn page(cursor: Option<&str>, next_cursor: Option<&str>) -> ExternalApplicationReviewPageV2 {
        ExternalApplicationReviewPageV2 {
            schema_version: EXTERNAL_APPLICATION_SCHEMA_V2,
            execution_domain_id: ExecutionDomainId::new("host-a").unwrap(),
            workspace_scope_id: Some("workspace:0123456789abcdef".to_string()),
            target_scope: ExternalApplicationTargetScopeV2::WorkspaceOverride,
            review_id: "review-7".to_string(),
            preference_revision: 11,
            expected_generations: vec![ExternalApplicationOwnerGenerationV2 {
                owner: ExternalApplicationReviewItemKindV2::Tool,
                generation: 7,
            }],
            cursor: cursor.map(str::to_string),
            next_cursor: next_cursor.map(str::to_string),
            total_count: 2,
            items: vec![item("tool-recommended", true), item("tool-optional", false)],
        }
    }

    #[test]
    fn overview_uses_five_shared_states_and_hides_mutations_for_read_only_hosts() {
        let writable = external_application_overview_text(&snapshot(
            ExternalApplicationHostCapabilitiesV2::read_write(),
        ));
        for expected in [
            "Connected",
            "Configuration available",
            "No configuration",
            "Needs attention",
            "Temporarily unavailable",
        ] {
            assert!(writable.contains(expected), "{expected}\n{writable}");
        }
        assert!(writable.contains("/extensions connect 2"));
        assert!(writable.contains("/extensions review"));
        assert!(writable.contains("Safe Mode: on"));

        let read_only = external_application_overview_text(&snapshot(
            ExternalApplicationHostCapabilitiesV2::read_only(),
        ));
        for forbidden in [
            "/extensions connect",
            "/extensions disconnect",
            "/extensions defer",
            "/extensions review apply",
        ] {
            assert!(!read_only.contains(forbidden), "{forbidden}\n{read_only}");
        }
    }

    #[test]
    fn legacy_status_fallback_is_embedded_read_only_only() {
        for code in [
            ExternalSourceOperationErrorCode::HostCapabilityUnavailable,
            ExternalSourceOperationErrorCode::Unsupported,
        ] {
            let error = ExternalSourceOperationError::new(code, "V2 unavailable", false);
            assert!(should_fallback_to_legacy_external_status(false, &error));
            assert!(!should_fallback_to_legacy_external_status(true, &error));
        }
        let unrelated = ExternalSourceOperationError::new(
            ExternalSourceOperationErrorCode::Internal,
            "Host failed",
            false,
        );
        assert!(!should_fallback_to_legacy_external_status(
            false, &unrelated
        ));
    }

    #[test]
    fn overview_preserves_host_health_and_recovery_without_recomputing_status() {
        let mut host = snapshot(ExternalApplicationHostCapabilitiesV2::read_write());
        host.applications[0].health = ExternalApplicationHealthV2::Degraded;
        host.applications[0].recovery_actions = vec![
            bitfun_product_domains::external_source_control::ExternalApplicationRecoveryActionV2::ReconnectHost,
            bitfun_product_domains::external_source_control::ExternalApplicationRecoveryActionV2::ViewReason,
        ];

        let text = external_application_overview_text(&host);
        assert!(text.contains("connected - Connected"));
        assert!(text.contains("Health: degraded"));
        assert!(text.contains("Recovery: reconnect host, view reason"));
    }

    #[test]
    fn numbered_application_actions_require_the_rendered_v2_snapshot() {
        let unavailable = ExternalApplicationUiState::default();
        assert!(parse_external_application_action("connect 1", &unavailable)
            .unwrap_err()
            .contains("V2"));

        let mut state = ExternalApplicationUiState::default();
        state
            .replace_snapshot(snapshot(ExternalApplicationHostCapabilitiesV2::read_write()))
            .unwrap();
        assert_eq!(
            parse_external_application_action("connect 2", &state).unwrap(),
            ExternalApplicationUiAction::ConnectApplication {
                application_id: "available".to_string()
            }
        );
        assert_eq!(
            parse_external_application_action("disconnect 1", &state).unwrap(),
            ExternalApplicationUiAction::DisconnectApplication {
                application_id: "connected".to_string()
            }
        );
        assert_eq!(
            parse_external_application_action("defer 4", &state).unwrap(),
            ExternalApplicationUiAction::DeferApplication {
                application_id: "attention".to_string()
            }
        );
        assert!(parse_external_application_action("connect 1", &state)
            .unwrap_err()
            .contains("next action"));
        assert!(parse_external_application_action("disconnect 2", &state)
            .unwrap_err()
            .contains("next action"));
        assert!(parse_external_application_action("defer 2", &state)
            .unwrap_err()
            .contains("next action"));

        let read_only = {
            let mut state = ExternalApplicationUiState::default();
            state
                .replace_snapshot(snapshot(ExternalApplicationHostCapabilitiesV2::read_only()))
                .unwrap();
            state
        };
        assert!(parse_external_application_action("connect 2", &read_only)
            .unwrap_err()
            .contains("read-only"));
    }

    #[test]
    fn workspace_context_targets_an_override_even_when_the_effective_value_is_inherited() {
        let mut inherited = snapshot(ExternalApplicationHostCapabilitiesV2::read_write());
        inherited.effective_connection_scope = ExternalApplicationTargetScopeV2::UserDefault;
        let mut state = ExternalApplicationUiState::default();
        state.replace_snapshot(inherited).unwrap();

        let request = state
            .control_request(
                "operation-workspace",
                ExternalApplicationControlActionV2::ConnectApplication {
                    application_id: "available".to_string(),
                },
            )
            .unwrap();
        assert_eq!(
            request.target_scope,
            ExternalApplicationTargetScopeV2::WorkspaceOverride
        );
        assert_eq!(
            request.workspace_scope_id.as_deref(),
            Some("workspace:0123456789abcdef")
        );

        let mut user_default = snapshot(ExternalApplicationHostCapabilitiesV2::read_write());
        user_default.workspace_scope_id = None;
        user_default.effective_connection_scope = ExternalApplicationTargetScopeV2::UserDefault;
        state.replace_snapshot(user_default).unwrap();
        let request = state
            .control_request(
                "operation-user",
                ExternalApplicationControlActionV2::ConnectApplication {
                    application_id: "available".to_string(),
                },
            )
            .unwrap();
        assert_eq!(
            request.target_scope,
            ExternalApplicationTargetScopeV2::UserDefault
        );
        assert_eq!(request.workspace_scope_id, None);
    }

    #[test]
    fn review_selection_stores_only_overrides_to_the_recommended_baseline() {
        let mut state = ExternalApplicationUiState::default();
        state
            .replace_snapshot(snapshot(ExternalApplicationHostCapabilitiesV2::read_write()))
            .unwrap();
        state
            .replace_review_page(page(None, Some("page-2")), ExternalReviewNavigation::Open)
            .unwrap();

        assert!(state.review_item_selected(0).unwrap());
        assert!(!state.review_item_selected(1).unwrap());
        state.set_review_item_selected(0, false).unwrap();
        state.set_review_item_selected(1, true).unwrap();

        let request = state.review_apply_request("operation-1").unwrap();
        let bitfun_product_domains::external_source_control::ExternalApplicationControlActionV2::SubmitApplicationReview {
            selection_baseline,
            selection_overrides,
            ..
        } = request.action else {
            panic!("expected review action");
        };
        assert_eq!(
            selection_baseline,
            bitfun_product_domains::external_source_control::ExternalApplicationReviewSelectionBaselineV2::Recommended
        );
        assert_eq!(selection_overrides.len(), 2);
        assert!(!selection_overrides[0].selected);
        assert!(selection_overrides[1].selected);
    }

    #[test]
    fn review_navigation_binds_cursors_and_rejects_stale_pages() {
        let mut state = ExternalApplicationUiState::default();
        state
            .replace_snapshot(snapshot(ExternalApplicationHostCapabilitiesV2::read_write()))
            .unwrap();
        state
            .replace_review_page(page(None, Some("page-2")), ExternalReviewNavigation::Open)
            .unwrap();

        let (next, navigation) = state
            .review_page_request(ExternalReviewDirection::Next)
            .unwrap();
        assert_eq!(next.cursor.as_deref(), Some("page-2"));
        state
            .replace_review_page(page(Some("page-2"), None), navigation)
            .unwrap();
        let (previous, navigation) = state
            .review_page_request(ExternalReviewDirection::Previous)
            .unwrap();
        assert_eq!(previous.cursor, None);
        state
            .replace_review_page(page(None, Some("page-2")), navigation)
            .unwrap();

        let mut stale = page(None, None);
        stale.preference_revision += 1;
        assert!(state
            .replace_review_page(stale, ExternalReviewNavigation::Open)
            .unwrap_err()
            .contains("stale"));

        let (next, navigation) = state
            .review_page_request(ExternalReviewDirection::Next)
            .unwrap();
        let mut stale_generation = page(next.cursor.as_deref(), None);
        stale_generation.expected_generations[0].generation += 1;
        assert!(state
            .replace_review_page(stale_generation, navigation)
            .unwrap_err()
            .contains("stale"));
    }

    #[test]
    fn review_commands_resolve_current_page_numbers_and_offer_a_non_mutating_defer() {
        let mut state = ExternalApplicationUiState::default();
        state
            .replace_snapshot(snapshot(ExternalApplicationHostCapabilitiesV2::read_write()))
            .unwrap();
        state
            .replace_review_page(page(None, None), ExternalReviewNavigation::Open)
            .unwrap();

        assert_eq!(
            parse_external_application_action("review include 2", &state).unwrap(),
            ExternalApplicationUiAction::SetReviewItem {
                item_ref: ExternalApplicationReviewItemRefV2 {
                    kind: ExternalApplicationReviewItemKindV2::Tool,
                    stable_id: "tool-optional".to_string(),
                },
                selected: true,
            }
        );
        assert_eq!(
            parse_external_application_action("review exclude 1", &state).unwrap(),
            ExternalApplicationUiAction::SetReviewItem {
                item_ref: ExternalApplicationReviewItemRefV2 {
                    kind: ExternalApplicationReviewItemKindV2::Tool,
                    stable_id: "tool-recommended".to_string(),
                },
                selected: false,
            }
        );
        assert_eq!(
            parse_external_application_action("review apply", &state).unwrap(),
            ExternalApplicationUiAction::ApplyReview
        );
        assert_eq!(
            parse_external_application_action("review defer", &state).unwrap(),
            ExternalApplicationUiAction::DeferReview
        );
    }
}

fn parse_external_agent_review_action(
    arguments: &str,
    current_snapshot: Option<&ExternalSourceCatalogSnapshot>,
    reviewed_snapshot: Option<&ExternalSourceCatalogSnapshot>,
) -> Result<ExternalAgentReviewAction, String> {
    let mut parts = arguments.split_whitespace();
    let Some(command) = parts.next() else {
        return Ok(ExternalAgentReviewAction::Show);
    };
    if command.eq_ignore_ascii_case("refresh") {
        if parts.next().is_some() {
            return Err("usage: /agent refresh".to_string());
        }
        return Ok(ExternalAgentReviewAction::Refresh);
    }
    if command.eq_ignore_ascii_case("help") {
        return Ok(ExternalAgentReviewAction::Show);
    }
    let snapshot = reviewed_snapshot.or(current_snapshot).ok_or_else(|| {
        "BitFun has not finished checking agents from external AI applications; run /agent refresh".to_string()
    })?;
    if command.eq_ignore_ascii_case("enable") || command.eq_ignore_ascii_case("disable") {
        let index = parse_positive_index(parts.next(), "agent number")?;
        if parts.next().is_some() {
            return Err(format!("usage: /agent {command} <agent-number>"));
        }
        let agent = snapshot
            .subagents
            .get(index)
            .ok_or_else(|| "that agent is no longer available; run /agent refresh".to_string())?;
        let approved = command.eq_ignore_ascii_case("enable");
        let allowed = if approved {
            matches!(
                agent.activation_state,
                ExternalSubagentActivationState::ApprovalRequired
                    | ExternalSubagentActivationState::Declined
            )
        } else {
            matches!(
                agent.activation_state,
                ExternalSubagentActivationState::Active
            )
        };
        if !allowed {
            return Err(format!(
                "agent {} is {}; run /agent refresh for its next step",
                index + 1,
                external_agent_activation_label(&agent.activation_state)
            ));
        }
        return Ok(ExternalAgentReviewAction::Decide {
            candidate_id: agent.candidate_id.clone(),
            decision_key: agent.decision_key.clone(),
            approved,
            expected_subagent_generation: snapshot.subagent_generation,
            expected_preference_revision: snapshot.preference_revision,
        });
    }
    if command.eq_ignore_ascii_case("bind") {
        let binding_index = parse_positive_index(parts.next(), "binding number")?;
        let raw_choice = parts
            .next()
            .ok_or_else(|| "missing choice number".to_string())?;
        let choice_number = raw_choice
            .parse::<usize>()
            .map_err(|_| "choice number must be zero or a positive number".to_string())?;
        if parts.next().is_some() {
            return Err("usage: /agent bind <binding-number> <choice-number>".to_string());
        }
        let binding = snapshot
            .subagent_model_binding_groups
            .get(binding_index)
            .ok_or_else(|| {
                "that model binding is no longer available; run /agent refresh".to_string()
            })?;
        if !matches!(
            binding.method,
            ExternalSubagentModelBindingMethod::BindingRequired
                | ExternalSubagentModelBindingMethod::Explicit
                | ExternalSubagentModelBindingMethod::BindingUnavailable
        ) {
            return Err(format!(
                "model binding {} is automatic and cannot be changed",
                binding_index + 1
            ));
        }
        let target = if choice_number == 0 {
            None
        } else {
            Some(
                snapshot
                    .subagent_model_binding_options
                    .get(choice_number - 1)
                    .ok_or_else(|| {
                        "that model choice is no longer available; run /agent refresh".to_string()
                    })?
                    .target
                    .clone(),
            )
        };
        return Ok(ExternalAgentReviewAction::Bind {
            binding_key: binding.binding_key.clone(),
            target,
            expected_subagent_generation: snapshot.subagent_generation,
            expected_preference_revision: snapshot.preference_revision,
        });
    }
    if command.eq_ignore_ascii_case("choose") {
        let conflict_index = parse_positive_index(parts.next(), "conflict number")?;
        let raw_candidate = parts
            .next()
            .ok_or_else(|| "missing choice number".to_string())?;
        let candidate_number = raw_candidate
            .parse::<usize>()
            .map_err(|_| "choice number must be zero or a positive number".to_string())?;
        if parts.next().is_some() {
            return Err("usage: /agent choose <conflict-number> <choice-number>".to_string());
        }
        let conflict = snapshot
            .subagent_conflicts
            .iter()
            .filter(|conflict| conflict.selected_candidate_id.is_none())
            .chain(
                snapshot
                    .subagent_conflicts
                    .iter()
                    .filter(|conflict| conflict.selected_candidate_id.is_some()),
            )
            .nth(conflict_index)
            .ok_or_else(|| {
                "that conflict is no longer available; run /agent refresh".to_string()
            })?;
        let (candidate_id, approve_external) = if candidate_number == 0 {
            (DISABLED_EXTERNAL_AGENT_CONFLICT_CHOICE.to_string(), false)
        } else {
            let candidate = conflict
                .candidates
                .get(candidate_number - 1)
                .ok_or_else(|| {
                    "that choice is no longer available; run /agent refresh".to_string()
                })?;
            (candidate.candidate_id.clone(), candidate.external)
        };
        return Ok(ExternalAgentReviewAction::Choose {
            conflict_key: conflict.conflict_key.clone(),
            candidate_id,
            approve_external,
            expected_subagent_generation: snapshot.subagent_generation,
            expected_preference_revision: snapshot.preference_revision,
        });
    }
    Err("usage: /agent [refresh | bind <binding-number> <choice-number> | enable <number> | disable <number> | choose <conflict-number> <choice-number>]".to_string())
}

fn external_agent_mutation_result_label(
    action: &ExternalAgentReviewAction,
    snapshot: &ExternalSourceCatalogSnapshot,
) -> String {
    match action {
        ExternalAgentReviewAction::Refresh => "External agents refreshed".to_string(),
        ExternalAgentReviewAction::Decide {
            candidate_id,
            approved,
            ..
        } => {
            let active = snapshot
                .subagents
                .iter()
                .find(|agent| agent.candidate_id == *candidate_id);
            match (approved, active.map(|agent| &agent.activation_state)) {
                (true, Some(ExternalSubagentActivationState::Active)) => {
                    "External agent enabled".to_string()
                }
                (false, Some(ExternalSubagentActivationState::Declined)) => {
                    "External agent disabled".to_string()
                }
                _ => {
                    "External agent decision saved; run /agent refresh to review its current state"
                        .to_string()
                }
            }
        }
        ExternalAgentReviewAction::Choose {
            conflict_key,
            candidate_id,
            ..
        } => {
            let selected = snapshot.subagent_conflicts.iter().any(|conflict| {
                conflict.conflict_key == *conflict_key
                    && conflict.selected_candidate_id.as_deref() == Some(candidate_id.as_str())
            });
            if selected {
                if candidate_id == DISABLED_EXTERNAL_AGENT_CONFLICT_CHOICE {
                    "Conflicting agent kept unavailable".to_string()
                } else {
                    "Agent source selected".to_string()
                }
            } else {
                "Agent choices changed; run /agent refresh before choosing".to_string()
            }
        }
        ExternalAgentReviewAction::Bind {
            binding_key,
            target,
            ..
        } => {
            let binding = snapshot
                .subagent_model_binding_groups
                .iter()
                .find(|binding| binding.binding_key == *binding_key);
            if target.is_none() && binding.is_some_and(|binding| binding.selected_target.is_none())
            {
                "Agent model binding cleared".to_string()
            } else if binding
                .is_some_and(|binding| binding.selected_target.as_ref() == target.as_ref())
            {
                "Agent model binding saved".to_string()
            } else {
                "Agent model choices changed; run /agent refresh before choosing".to_string()
            }
        }
        ExternalAgentReviewAction::Show => "External agents".to_string(),
    }
}
