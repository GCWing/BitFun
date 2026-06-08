// Boundary rules for source ownership, facades, and required owner content.

export const requiredContentRules = [
  {
    path: 'src/crates/execution/runtime-services/src/lib.rs',
    reason:
      'runtime-services must own typed runtime service assembly and capability validation contracts',
    patterns: [
      {
        regex: /\bpub struct RuntimeServices\b/,
        message: 'missing runtime services assembly container',
      },
      {
        regex: /\bpub struct RuntimeServicesBuilder\b/,
        message: 'missing runtime services builder',
      },
      {
        regex: /\bpub struct CapabilityAvailability\b/,
        message: 'missing capability availability contract',
      },
      {
        regex: /\bpub trait RuntimeServicesProvider\b/,
        message: 'missing runtime services provider contract',
      },
      {
        regex: /\bpub struct RuntimeServicesRegistry\b/,
        message: 'missing runtime services registry',
      },
      {
        regex: /\bCapabilityMismatch\b/,
        message: 'missing typed capability mismatch error',
      },
      {
        regex: /\brequire_capability\b/,
        message: 'missing typed capability requirement check',
      },
    ],
  },
  {
    path: 'src/crates/execution/runtime-services/tests/runtime_services_contracts.rs',
    reason:
      'runtime-services must keep behavior-equivalence contracts for required services, optional capabilities, registry assembly, and remote port exposure',
    patterns: [
      {
        regex: /\bbuilder_requires_mandatory_runtime_services\b/,
        message: 'missing mandatory runtime services regression',
      },
      {
        regex:
          /\bfake_provider_registers_required_and_remote_services_through_registry\b/,
        message: 'missing provider registry assembly regression',
      },
      {
        regex:
          /\bmissing_optional_capability_returns_typed_unsupported_error\b/,
        message: 'missing optional capability unsupported regression',
      },
      {
        regex:
          /\bcapability_availability_reports_optional_service_status_without_side_effects\b/,
        message: 'missing capability availability regression',
      },
      {
        regex: /\bbuilder_rejects_port_registered_under_the_wrong_capability\b/,
        message: 'missing capability mismatch regression',
      },
      {
        regex: /\bregistered_remote_ports_expose_owner_contract_methods\b/,
        message: 'missing remote port owner contract regression',
      },
    ],
  },
  {
    path: 'src/crates/execution/agent-runtime/src/prompt.rs',
    reason:
      'agent-runtime must own prompt-loop facts that do not require concrete workspace or product IO',
    patterns: [
      {
        regex: /\bpub enum UserContextSection\b/,
        message: 'missing agent-runtime user-context section contract',
      },
      {
        regex: /\bpub struct UserContextPolicy\b/,
        message: 'missing agent-runtime user-context policy contract',
      },
      {
        regex: /\bpub struct ToolListingSections\b/,
        message: 'missing agent-runtime tool-listing section contract',
      },
      {
        regex: /\bpub struct PrependedPromptReminders\b/,
        message: 'missing agent-runtime prepended-reminder contract',
      },
    ],
  },
  {
    path: 'src/crates/execution/agent-runtime/src/prompt_cache.rs',
    reason:
      'agent-runtime must own prompt-cache policy, identities, DTOs, scope keys, and in-memory runtime store',
    patterns: [
      {
        regex: /\bpub const PROMPT_CACHE_SCHEMA_VERSION\b/,
        message: 'missing agent-runtime prompt-cache schema fact',
      },
      {
        regex: /\bpub struct PromptCachePolicy\b/,
        message: 'missing agent-runtime prompt-cache policy',
      },
      {
        regex: /\bpub fn prompt_cache_scope_key\b/,
        message: 'missing agent-runtime prompt-cache scope-key helper',
      },
      {
        regex: /\bpub struct SessionPromptCacheStore\b/,
        message: 'missing agent-runtime in-memory prompt-cache store',
      },
      {
        regex: /\bpub enum PromptCacheLookup\b/,
        message: 'missing agent-runtime prompt-cache lookup contract',
      },
    ],
  },
  {
    path: 'src/crates/execution/agent-runtime/tests/prompt_cache_contracts.rs',
    reason:
      'agent-runtime prompt-cache owner must keep behavior-equivalence contracts for cache identity, expiry, invalidation, and scope-key shape',
    patterns: [
      {
        regex: /\bprompt_cache_policy_keeps_existing_default_persistence_ttl\b/,
        message: 'missing prompt-cache default TTL regression',
      },
      {
        regex: /\bprompt_cache_lookup_preserves_identity_and_expiry_semantics\b/,
        message: 'missing prompt-cache identity/expiry regression',
      },
      {
        regex: /\bprompt_cache_scope_key_preserves_legacy_mode_switch_shape\b/,
        message: 'missing prompt-cache scope-key shape regression',
      },
    ],
  },
  {
    path: 'src/crates/execution/harness/src/lib.rs',
    reason:
      'harness must own provider-neutral harness descriptors and descriptor registry wiring without concrete execution',
    patterns: [
      {
        regex: /\bpub struct HarnessProviderDescriptor\b/,
        message: 'missing provider-neutral harness provider descriptor',
      },
      {
        regex: /\bpub fn build_descriptor_harness_registry\b/,
        message: 'missing descriptor harness registry builder',
      },
      {
        regex: /\bDescriptorHarnessProvider::legacy_facade\b/,
        message: 'missing legacy-facade descriptor adapter',
      },
    ],
  },
  {
    path: 'src/crates/assembly/product-capabilities/src/lib.rs',
    reason:
      'product-capabilities must select harness descriptors from the harness owner instead of owning descriptor construction',
    patterns: [
      {
        regex: /\bHarnessProviderDescriptor\b/,
        message: 'missing harness descriptor selection in product capability packs',
      },
      {
        regex: /\bbuild_descriptor_harness_registry\b/,
        message: 'missing harness-owned descriptor registry assembly delegation',
      },
      {
        regex: /\bProductCapabilityAssembly\b/,
        message: 'missing product capability assembly owner',
      },
    ],
  },
  {
    path: 'src/crates/execution/agent-runtime/src/agents.rs',
    reason:
      'agent-runtime must own shared mode config profile facts that are runtime-visible and product-neutral',
    patterns: [
      {
        regex: /\bpub const SHARED_CODING_MODE_PROMPT_TEMPLATE\b/,
        message: 'missing shared coding-mode prompt template fact',
      },
      {
        regex: /\bpub const SHARED_CODING_MODE_CONFIG_PROFILE_ID\b/,
        message: 'missing shared coding-mode config profile id',
      },
      {
        regex: /\bpub fn resolve_mode_config_profile_id\b/,
        message: 'missing mode config profile resolver',
      },
      {
        regex: /\bpub fn mode_config_profile_member_mode_ids\b/,
        message: 'missing mode config profile member lookup',
      },
      {
        regex: /\bpub fn mode_presentation_rank\b/,
        message: 'missing mode presentation rank',
      },
      {
        regex: /\bpub fn shared_coding_mode_user_context_policy\b/,
        message: 'missing shared coding-mode user-context policy',
      },
      {
        regex: /\bpub enum SubagentListScope\b/,
        message: 'missing subagent query list-scope contract',
      },
      {
        regex: /\bpub struct SubagentQueryContext\b/,
        message: 'missing subagent query context contract',
      },
      {
        regex: /\bpub struct SubagentVisibilityPolicy\b/,
        message: 'missing subagent visibility policy contract',
      },
      {
        regex: /\bpub enum SubagentStateReason\b/,
        message: 'missing subagent state reason contract',
      },
      {
        regex: /\bpub struct SubagentOverrideLayers\b/,
        message: 'missing subagent override-layer contract',
      },
      {
        regex: /\bpub fn resolve_subagent_default_enabled\b/,
        message: 'missing subagent default-enabled decision helper',
      },
      {
        regex: /\bpub fn resolve_subagent_availability\b/,
        message: 'missing subagent availability decision helper',
      },
      {
        regex: /\bpub enum SubAgentSource\b/,
        message: 'missing subagent source DTO',
      },
      {
        regex: /\bpub const fn subagent_source_kind\b/,
        message: 'missing subagent source runtime-kind mapping',
      },
      {
        regex: /\bpub const fn subagent_source_presentation_rank\b/,
        message: 'missing subagent source presentation rank',
      },
    ],
  },
  {
    path: 'src/crates/execution/agent-runtime/tests/agent_registry_contracts.rs',
    reason:
      'agent-runtime agent registry owner must keep behavior-equivalence contracts for visibility, availability, shared mode config, and source ordering',
    patterns: [
      {
        regex:
          /\bvisibility_policy_supports_public_restricted_hidden_and_denied_parents\b/,
        message: 'missing subagent visibility policy regression',
      },
      {
        regex:
          /\bavailability_preserves_builtin_project_and_user_override_layering\b/,
        message: 'missing subagent availability layering regression',
      },
      {
        regex:
          /\bdefault_enabled_uses_visibility_only_for_builtin_subagents\b/,
        message: 'missing subagent default-enabled regression',
      },
      {
        regex: /\bshared_coding_modes_resolve_to_the_same_config_profile\b/,
        message: 'missing shared coding-mode profile regression',
      },
      {
        regex:
          /\bsubagent_source_contract_preserves_runtime_kind_and_presentation_order\b/,
        message: 'missing subagent source ordering regression',
      },
      {
        regex:
          /\bmode_presentation_and_shared_context_policy_match_existing_mode_contract\b/,
        message: 'missing mode presentation/context-policy regression',
      },
    ],
  },
  {
    path: 'src/crates/execution/agent-runtime/src/custom_subagent.rs',
    reason:
      'agent-runtime must own custom subagent portable schema defaults, discovery, and markdown front-matter IO',
    patterns: [
      {
        regex: /\bpub enum CustomSubagentKind\b/,
        message: 'missing custom subagent source-kind contract',
      },
      {
        regex: /\bpub struct CustomSubagentDiscoveryRoots\b/,
        message: 'missing custom subagent discovery root contract',
      },
      {
        regex: /\bpub struct CustomSubagentLoadReport\b/,
        message: 'missing custom subagent load report contract',
      },
      {
        regex: /\bpub struct CustomSubagentDefinition\b/,
        message: 'missing custom subagent definition schema',
      },
      {
        regex: /\bpub enum CustomSubagentDefinitionError\b/,
        message: 'missing custom subagent definition validation errors',
      },
      {
        regex: /\bDEFAULT_CUSTOM_SUBAGENT_TOOLS\b/,
        message: 'missing custom subagent default tools contract',
      },
      {
        regex: /\bpub fn custom_subagent_tools_from_front_matter\b/,
        message: 'missing custom subagent tools front-matter parser',
      },
      {
        regex: /\bpub fn custom_subagent_tools_to_front_matter\b/,
        message: 'missing custom subagent tools front-matter serializer',
      },
      {
        regex: /\bpub const fn custom_subagent_readonly_should_save\b/,
        message: 'missing custom subagent readonly save decision',
      },
      {
        regex: /\bpub const fn custom_subagent_review_should_save\b/,
        message: 'missing custom subagent review save decision',
      },
      {
        regex: /\bpub fn custom_subagent_model_should_save\b/,
        message: 'missing custom subagent model save decision',
      },
      {
        regex: /\bpub fn custom_subagent_read_markdown_file\b/,
        message: 'missing custom subagent markdown file reader',
      },
      {
        regex: /\bpub fn custom_subagent_save_markdown_parts\b/,
        message: 'missing custom subagent markdown file writer',
      },
      {
        regex: /\bpub fn custom_subagent_possible_dirs\b/,
        message: 'missing custom subagent directory discovery owner',
      },
      {
        regex: /\bpub fn load_custom_subagent_definitions\b/,
        message: 'missing custom subagent definition loading owner',
      },
    ],
  },
  {
    path: 'src/crates/execution/agent-runtime/tests/custom_subagent_discovery_contracts.rs',
    reason:
      'agent-runtime custom subagent discovery owner must keep behavior-equivalence contracts for directory priority, deduplication, and load errors',
    patterns: [
      {
        regex:
          /\bcustom_subagent_discovery_preserves_directory_priority_and_deduplication\b/,
        message: 'missing custom subagent discovery priority/dedup regression',
      },
      {
        regex:
          /\bcustom_subagent_discovery_reports_parse_errors_without_dropping_valid_files\b/,
        message: 'missing custom subagent discovery parse-error regression',
      },
    ],
  },
  {
    path: 'src/crates/execution/agent-runtime/tests/custom_subagent_contracts.rs',
    reason:
      'agent-runtime custom subagent owner must keep behavior-equivalence contracts for defaults and front-matter serialization decisions',
    patterns: [
      {
        regex: /\bcustom_subagent_defaults_match_existing_front_matter_contract\b/,
        message: 'missing custom subagent default regression',
      },
      {
        regex: /\bcustom_subagent_tool_front_matter_keeps_existing_comma_format\b/,
        message: 'missing custom subagent tools comma-format regression',
      },
      {
        regex: /\bcustom_subagent_default_fields_are_omitted_when_saved\b/,
        message: 'missing custom subagent default omission regression',
      },
      {
        regex: /\bcustom_subagent_definition_from_front_matter_preserves_schema_and_defaults\b/,
        message: 'missing custom subagent definition schema/default regression',
      },
      {
        regex: /\bcustom_subagent_definition_reports_legacy_missing_field_errors\b/,
        message: 'missing custom subagent missing-field regression',
      },
      {
        regex: /\bcustom_subagent_markdown_io_preserves_legacy_front_matter_shape\b/,
        message: 'missing custom subagent markdown IO regression',
      },
      {
        regex: /\bcustom_subagent_markdown_parse_errors_match_legacy_prefixes\b/,
        message: 'missing custom subagent markdown parse-error regression',
      },
    ],
  },
  {
    path: 'src/crates/execution/agent-runtime/src/post_call_hooks.rs',
    reason:
      'agent-runtime must own portable post-call hook routing decisions while concrete hook execution stays in the owning runtime',
    patterns: [
      {
        regex: /\bpub enum PostCallHookKind\b/,
        message: 'missing post-call hook kind contract',
      },
      {
        regex: /\bpub const fn successful_tool_post_call_hooks\b/,
        message: 'missing successful tool post-call hook routing decision',
      },
      {
        regex: /\bpub trait SuccessfulToolPostCallHookExecutor\b/,
        message: 'missing successful tool post-call hook executor contract',
      },
      {
        regex: /\bpub fn run_successful_tool_post_call_hooks\b/,
        message: 'missing successful tool post-call hook executor runner',
      },
    ],
  },
  {
    path: 'src/crates/execution/agent-runtime/tests/post_call_hook_contracts.rs',
    reason:
      'agent-runtime post-call hook owner must keep behavior-equivalence contracts for successful tool-call hook routing',
    patterns: [
      {
        regex: /\bsuccessful_tool_call_routes_to_shared_context_measurement_hook\b/,
        message: 'missing successful tool post-call hook routing regression',
      },
    ],
  },
  {
    path: 'src/crates/execution/agent-runtime/tests/post_call_hook_execution_contracts.rs',
    reason:
      'agent-runtime post-call hook owner must keep concrete-executor routing behavior-equivalence contracts',
    patterns: [
      {
        regex: /\bsuccessful_tool_post_call_executor_runs_deep_review_measurement_route\b/,
        message: 'missing successful tool post-call executor regression',
      },
    ],
  },
  {
    path: 'src/crates/execution/agent-runtime/src/tool_confirmation.rs',
    reason:
      'agent-runtime must own portable tool confirmation planning and failure mapping while core keeps UI/channel side effects',
    patterns: [
      {
        regex: /\bpub struct ToolConfirmationRequestFacts\b/,
        message: 'missing tool confirmation request facts',
      },
      {
        regex: /\bpub enum ToolConfirmationPlan\b/,
        message: 'missing tool confirmation plan contract',
      },
      {
        regex: /\bpub enum ToolConfirmationOutcome\b/,
        message: 'missing tool confirmation outcome contract',
      },
      {
        regex: /\bpub enum ToolConfirmationWaitResult\b/,
        message: 'missing tool confirmation wait-result contract',
      },
      {
        regex: /\bpub enum ConfirmationFailureKind\b/,
        message: 'missing tool confirmation failure kind',
      },
      {
        regex: /\bpub fn resolve_tool_confirmation_plan\b/,
        message: 'missing tool confirmation plan resolver',
      },
      {
        regex: /\bpub fn resolve_confirmation_failure\b/,
        message: 'missing tool confirmation failure resolver',
      },
      {
        regex: /\bpub fn resolve_confirmation_wait_result\b/,
        message: 'missing tool confirmation wait-result resolver',
      },
    ],
  },
  {
    path: 'src/crates/execution/agent-runtime/src/checkpoint.rs',
    reason:
      'agent-runtime must own provider-neutral light-checkpoint summary policy while core keeps concrete Git/session IO',
    patterns: [
      {
        regex: /\bpub struct LightCheckpoint\b/,
        message: 'missing light checkpoint DTO',
      },
      {
        regex: /\bpub enum LightCheckpointWorkspaceFacts\b/,
        message: 'missing light checkpoint workspace facts',
      },
      {
        regex: /\bpub struct GitStatusCheckpointFacts\b/,
        message: 'missing git status checkpoint facts',
      },
      {
        regex: /\bpub fn build_light_checkpoint\b/,
        message: 'missing light checkpoint builder',
      },
    ],
  },
  {
    path: 'src/crates/execution/agent-runtime/tests/tool_confirmation_contracts.rs',
    reason:
      'agent-runtime tool confirmation owner must keep behavior-equivalence contracts for legacy permission planning and failures',
    patterns: [
      {
        regex: /\bconfirmation_plan_requires_permission_only_when_both_flags_are_true\b/,
        message: 'missing tool confirmation gate regression',
      },
      {
        regex: /\bconfirmation_plan_preserves_legacy_no_timeout_one_year_deadline\b/,
        message: 'missing tool confirmation no-timeout regression',
      },
      {
        regex: /\bconfirmation_failure_mapping_preserves_legacy_reasons_and_errors\b/,
        message: 'missing tool confirmation failure mapping regression',
      },
      {
        regex: /\bconfirmation_wait_result_mapping_preserves_legacy_timeout_and_rejection\b/,
        message: 'missing tool confirmation wait-result mapping regression',
      },
    ],
  },
  {
    path: 'src/crates/execution/agent-runtime/src/scheduler.rs',
    reason:
      'agent-runtime scheduler owner must keep portable queue, background delivery, steering, reply, and round injection decisions outside concrete core session IO',
    patterns: [
      {
        regex: /\bpub const DEFAULT_MAX_DIALOG_QUEUE_DEPTH\b/,
        message: 'missing dialog queue depth contract',
      },
      {
        regex: /\bpub struct ActiveDialogTurn\b/,
        message: 'missing active dialog turn owner',
      },
      {
        regex: /\bpub struct ActiveDialogTurnStore\b/,
        message: 'missing active dialog turn store owner',
      },
      {
        regex: /\bpub enum AgentSessionReplyAction\b/,
        message: 'missing agent-session reply action contract',
      },
      {
        regex: /\bpub struct AgentSessionReplyPlan\b/,
        message: 'missing agent-session reply plan contract',
      },
      {
        regex: /\bpub struct BackgroundDeliveryFacts\b/,
        message: 'missing background delivery facts contract',
      },
      {
        regex: /\bpub enum BackgroundDeliveryAction\b/,
        message: 'missing background delivery action contract',
      },
      {
        regex: /\bpub enum BackgroundInjectionKind\b/,
        message: 'missing background injection kind contract',
      },
      {
        regex: /\bpub struct DialogReplySuppressionSet\b/,
        message: 'missing dialog reply suppression set owner',
      },
      {
        regex: /\bpub enum DialogSteeringAction\b/,
        message: 'missing dialog steering action contract',
      },
      {
        regex: /\bpub struct DialogTurnQueue\b/,
        message: 'missing dialog turn queue owner',
      },
      {
        regex: /\bpub struct SessionAbortFlags\b/,
        message: 'missing session abort flags owner',
      },
      {
        regex: /\bpub fn resolve_agent_session_reply_action\b/,
        message: 'missing agent-session reply action resolver',
      },
      {
        regex: /\bpub const fn resolve_background_delivery_action\b/,
        message: 'missing background delivery action resolver',
      },
      {
        regex: /\bpub fn resolve_background_delivery_injection\b/,
        message: 'missing background delivery injection resolver',
      },
      {
        regex: /\bpub fn resolve_dialog_steering_action\b/,
        message: 'missing dialog steering action resolver',
      },
      {
        regex: /\bfollow_up_submission_policy\b/,
        message: 'missing background follow-up submission policy helper',
      },
      {
        regex: /\bSubmitAgentSessionFollowUp\b/,
        message: 'missing agent-session follow-up action variant',
      },
      {
        regex: /\bInjectIntoRunningTurn\b/,
        message: 'missing running-turn injection action variant',
      },
      {
        regex: /\bpub struct SessionRoundYieldFlags\b/,
        message: 'missing session round yield flags owner',
      },
      {
        regex: /\bpub struct SessionRoundInjectionBuffer\b/,
        message: 'missing session round injection buffer owner',
      },
      {
        regex: /\bpub enum TurnOutcome\b/,
        message: 'missing turn outcome contract',
      },
      {
        regex: /\bpub enum TurnOutcomeQueueAction\b/,
        message: 'missing turn outcome queue action contract',
      },
    ],
  },
  {
    path: 'src/crates/execution/agent-runtime/tests/scheduler_contracts.rs',
    reason:
      'agent-runtime scheduler owner must keep behavior-equivalence contracts for background delivery, queueing, reply suppression, steering, round interruption, and turn outcomes',
    patterns: [
      {
        regex: /\bbackground_delivery_injects_when_session_is_processing\b/,
        message: 'missing background delivery processing regression',
      },
      {
        regex:
          /\bbackground_delivery_starts_agent_session_follow_up_when_session_is_not_processing\b/,
        message: 'missing background delivery follow-up regression',
      },
      {
        regex:
          /\bbackground_delivery_follow_up_uses_agent_session_source_semantics\b/,
        message: 'missing background follow-up source semantics regression',
      },
      {
        regex: /\bbackground_delivery_injection_does_not_expose_follow_up_policy\b/,
        message: 'missing background injection policy regression',
      },
      {
        regex:
          /\bbackground_delivery_injection_builds_thread_goal_current_turn_message\b/,
        message: 'missing thread-goal current-turn injection regression',
      },
      {
        regex:
          /\bbackground_delivery_injection_builds_background_result_with_display_fallback\b/,
        message: 'missing background result display fallback regression',
      },
      {
        regex: /\bdialog_turn_queue_preserves_priority_order_and_fifo_within_priority\b/,
        message: 'missing dialog queue ordering regression',
      },
      {
        regex:
          /\bdialog_turn_queue_rejects_overflow_and_preserves_current_error_shape\b/,
        message: 'missing dialog queue overflow regression',
      },
      {
        regex:
          /\bdialog_turn_queue_requeued_turn_keeps_original_priority_for_later_ordering\b/,
        message: 'missing dialog queue requeue regression',
      },
      {
        regex: /\bactive_dialog_turn_owns_agent_session_reply_suppression_facts\b/,
        message: 'missing active dialog turn suppression facts regression',
      },
      {
        regex:
          /\bactive_dialog_turn_store_owns_suppression_key_resolution_and_removal\b/,
        message: 'missing active dialog turn store regression',
      },
      {
        regex: /\breply_suppression_set_marks_takes_and_clears_turn_keys\b/,
        message: 'missing reply suppression set regression',
      },
      {
        regex: /\bsession_abort_flags_are_session_scoped\b/,
        message: 'missing session abort flags regression',
      },
      {
        regex:
          /\bagent_session_reply_action_forwards_completed_outcome_with_legacy_reminder_text\b/,
        message: 'missing agent-session completed reply regression',
      },
      {
        regex:
          /\bagent_session_reply_action_suppresses_cancelled_auto_reply_when_requested\b/,
        message: 'missing agent-session cancelled reply suppression regression',
      },
      {
        regex: /\bagent_session_reply_action_ignores_non_agent_session_turns\b/,
        message: 'missing non-agent-session reply suppression regression',
      },
      {
        regex: /\bdialog_steering_action_buffers_exact_running_turn_with_display_fallback\b/,
        message: 'missing dialog steering buffer regression',
      },
      {
        regex: /\bdialog_steering_action_rejects_when_target_turn_is_not_running\b/,
        message: 'missing dialog steering reject regression',
      },
      {
        regex: /\bround_yield_flags_are_session_scoped_and_clearable\b/,
        message: 'missing round yield flags regression',
      },
      {
        regex: /\bround_injection_buffer_drains_only_messages_for_the_active_turn\b/,
        message: 'missing round injection buffer regression',
      },
      {
        regex: /\bturn_outcome_status_reply_and_queue_policy_are_portable\b/,
        message: 'missing turn outcome queue policy regression',
      },
    ],
  },
  {
    path: 'src/crates/execution/agent-runtime/src/thread_goal.rs',
    reason:
      'agent-runtime must own persisted thread-goal runtime decisions, set/update planning, continuation decisions, and tool response shaping',
    patterns: [
      {
        regex: /\bpub struct ThreadGoalRuntime\b/,
        message: 'missing thread-goal runtime owner',
      },
      {
        regex: /\bpub struct SetThreadGoalRequest\b/,
        message: 'missing set-thread-goal request contract',
      },
      {
        regex: /\bpub fn build_set_thread_goal_result\b/,
        message: 'missing set-thread-goal result builder',
      },
      {
        regex: /\bcontinuation_after_turn\b/,
        message: 'missing thread-goal continuation-after-turn decision',
      },
      {
        regex: /\bpub struct ThreadGoalContinuationOutcome\b/,
        message: 'missing thread-goal continuation outcome contract',
      },
      {
        regex: /\bpub fn goal_tool_response\b/,
        message: 'missing thread-goal tool response helper',
      },
      {
        regex: /\bpub fn should_skip_goal_for_turn\b/,
        message: 'missing thread-goal skip-turn decision',
      },
    ],
  },
  {
    path: 'src/crates/execution/agent-runtime/tests/thread_goal_contracts.rs',
    reason:
      'agent-runtime thread-goal owner must keep behavior-equivalence contracts for goal creation, continuation limits, budget reporting, and wire response shape',
    patterns: [
      {
        regex: /\bset_thread_goal_creates_new_active_goal_with_trimmed_objective\b/,
        message: 'missing set-thread-goal creation regression',
      },
      {
        regex: /\bcontinuation_outcome_increments_active_goal_and_builds_plan\b/,
        message: 'missing thread-goal continuation increment regression',
      },
      {
        regex: /\bcontinuation_outcome_marks_active_goal_blocked_at_limit\b/,
        message: 'missing thread-goal blocked-at-limit regression',
      },
      {
        regex:
          /\bcontinuation_outcome_reports_budget_limit_once_when_tokens_cross_budget\b/,
        message: 'missing thread-goal budget-limit regression',
      },
      {
        regex: /\bprompt_and_tool_response_contracts_match_thread_goal_wire_shape\b/,
        message: 'missing thread-goal prompt/tool response wire-shape regression',
      },
    ],
  },
  {
    path: 'src/crates/execution/agent-runtime/tests/prompt_contracts.rs',
    reason:
      'agent-runtime prompt owner must keep behavior-equivalence contracts for user context and reminder ordering',
    patterns: [
      {
        regex: /\buser_context_policy_preserves_order_and_deduplicates_sections\b/,
        message: 'missing user-context policy order regression',
      },
      {
        regex: /\btool_listing_sections_render_only_present_sections\b/,
        message: 'missing tool-listing rendering regression',
      },
      {
        regex: /\bprepended_prompt_reminders_keep_runtime_injection_order\b/,
        message: 'missing prompt reminder ordering regression',
      },
    ],
  },
  {
    path: 'src/crates/execution/agent-runtime/src/events.rs',
    reason:
      'agent-runtime must own runtime event facts that do not require concrete scheduler or session IO',
    patterns: [
      {
        regex: /\bpub enum FinishReason\b/,
        message: 'missing agent-runtime finish-reason event fact',
      },
      {
        regex: /\bpub const fn session_state_label\b/,
        message: 'missing agent-runtime session-state label fact',
      },
      {
        regex: /\bpub fn turn_outcome_kind\b/,
        message: 'missing agent-runtime turn-outcome event fact',
      },
    ],
  },
  {
    path: 'src/crates/execution/agent-runtime/tests/events_contracts.rs',
    reason:
      'agent-runtime event owner must keep behavior-equivalence contracts for event wire labels',
    patterns: [
      {
        regex: /\bfinish_reason_display_preserves_wire_labels\b/,
        message: 'missing finish-reason wire-label regression',
      },
      {
        regex: /\bsession_state_labels_match_existing_event_wire_values\b/,
        message: 'missing session-state label regression',
      },
      {
        regex: /\bturn_outcome_kind_matches_existing_reply_policy_contract\b/,
        message: 'missing turn-outcome event fact regression',
      },
    ],
  },
  {
    path: 'src/crates/execution/agent-runtime/src/scheduled_job.rs',
    reason:
      'agent-runtime must own scheduled-job portable lifecycle state and transition decisions without concrete cron storage, schedule parsing, or session dispatch',
    patterns: [
      {
        regex: /\bpub struct ScheduledJobRuntimeState\b/,
        message: 'missing scheduled-job runtime state owner',
      },
      {
        regex: /\bpub enum ScheduledJobRunStatus\b/,
        message: 'missing scheduled-job run status owner',
      },
      {
        regex: /\bDEFAULT_SCHEDULED_JOB_RETRY_DELAY_MS\b/,
        message: 'missing scheduled-job retry delay contract',
      },
      {
        regex: /\bpub fn mark_manual_trigger\b/,
        message: 'missing manual trigger transition',
      },
      {
        regex: /\bpub fn apply_due_scheduled_trigger\b/,
        message: 'missing due scheduled trigger transition',
      },
      {
        regex: /\bpub fn mark_enqueued\b/,
        message: 'missing enqueue success transition',
      },
      {
        regex: /\bpub fn mark_enqueue_failed\b/,
        message: 'missing enqueue failure transition',
      },
      {
        regex: /\bpub fn recover_interrupted_turn_after_restart\b/,
        message: 'missing restart recovery transition',
      },
      {
        regex: /\bpub fn pending_is_due\b/,
        message: 'missing scheduled-job pending due decision',
      },
      {
        regex: /\bpub fn next_wakeup_at_ms\b/,
        message: 'missing scheduled-job wakeup decision',
      },
      {
        regex: /\bpub fn clear_pending_trigger\b/,
        message: 'missing scheduled-job pending trigger clear transition',
      },
    ],
  },
  {
    path: 'src/crates/execution/agent-runtime/tests/scheduled_job_contracts.rs',
    reason:
      'agent-runtime scheduled-job owner must keep behavior-equivalence contracts for wire shape, retry, coalescing, one-shot, missing-session, and restart recovery semantics',
    patterns: [
      {
        regex: /\bmanual_trigger_coalesces_existing_pending_run\b/,
        message: 'missing manual trigger coalescing regression',
      },
      {
        regex: /\bdue_scheduled_trigger_coalesces_when_active_or_pending\b/,
        message: 'missing scheduled trigger coalescing regression',
      },
      {
        regex: /\bpending_wakeup_prefers_retry_time_when_present\b/,
        message: 'missing retry wakeup regression',
      },
      {
        regex: /\bdisabled_and_config_clear_remove_pending_retry_without_touching_history\b/,
        message: 'missing disabled/config clear regression',
      },
      {
        regex: /\benqueue_success_sets_active_turn_and_disables_one_shot_next_run\b/,
        message: 'missing enqueue success one-shot regression',
      },
      {
        regex: /\benqueue_failure_preserves_retry_and_missing_session_disable_semantics\b/,
        message: 'missing enqueue failure regression',
      },
      {
        regex: /\brestart_recovery_marks_active_turn_error\b/,
        message: 'missing restart recovery regression',
      },
      {
        regex: /\bserde_shape_preserves_legacy_cron_state_wire_contract\b/,
        message: 'missing legacy cron state wire-shape regression',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/cron/types.rs',
    reason:
      'core cron types must preserve old import and wire paths while bitfun-agent-runtime owns scheduled-job runtime state',
    patterns: [
      {
        regex: /ScheduledJobRuntimeState as CronJobState/,
        message: 'missing scheduled-job state compatibility alias',
      },
      {
        regex: /ScheduledJobRunStatus as CronJobRunStatus/,
        message: 'missing scheduled-job status compatibility alias',
      },
      {
        regex: /DEFAULT_SCHEDULED_JOB_RETRY_DELAY_MS/,
        message: 'missing scheduled-job retry delay compatibility constant',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/cron/service.rs',
    reason:
      'core cron service may own concrete storage, schedule parsing, and scheduler dispatch, but scheduled-job lifecycle state transitions must delegate to agent-runtime',
    patterns: [
      {
        regex: /\bmark_manual_trigger\b/,
        message: 'missing manual trigger owner delegation',
      },
      {
        regex: /\bapply_due_scheduled_trigger\b/,
        message: 'missing scheduled trigger owner delegation',
      },
      {
        regex: /\bmark_enqueued\b/,
        message: 'missing enqueue success owner delegation',
      },
      {
        regex: /\bmark_enqueue_failed\b/,
        message: 'missing enqueue failure owner delegation',
      },
      {
        regex: /\brecover_interrupted_turn_after_restart\b/,
        message: 'missing restart recovery owner delegation',
      },
      {
        regex: /\bpending_is_due\b/,
        message: 'missing pending due owner delegation',
      },
      {
        regex: /\bnext_wakeup_at_ms\b/,
        message: 'missing wakeup owner delegation',
      },
      {
        regex: /\bclear_pending_trigger\b/,
        message: 'missing pending trigger clear owner delegation',
      },
      {
        regex: /\bScheduledJobEnqueueFailureAction\b/,
        message: 'missing enqueue failure action owner delegation',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/agents/definitions/custom/subagent.rs',
    reason:
      'core custom subagent path must stay a compatibility facade over agent-runtime schema/default and markdown IO decisions',
    patterns: [
      {
        regex: /pub use bitfun_agent_runtime::custom_subagent::CustomSubagentKind/,
        message: 'missing custom subagent kind compatibility re-export',
      },
      {
        regex: /\bCustomSubagentDefinition::new\b/,
        message: 'missing custom subagent definition construction delegation',
      },
      {
        regex: /\bcustom_subagent_read_markdown_file\b/,
        message: 'missing custom subagent markdown read delegation',
      },
      {
        regex: /\bcustom_subagent_save_markdown_parts\b/,
        message: 'missing custom subagent markdown save delegation',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/agents/registry/custom.rs',
    reason:
      'core custom subagent registry must delegate portable discovery/loading to agent-runtime while retaining validation and registry writes',
    patterns: [
      {
        regex: /\bload_custom_subagent_definitions\b/,
        message: 'missing custom subagent runtime load delegation',
      },
      {
        regex: /\bCustomSubagentDiscoveryRoots\b/,
        message: 'missing custom subagent runtime discovery root adapter',
      },
      {
        regex: /\bCustomSubagent::from_definition\b/,
        message: 'missing custom subagent runtime definition adapter',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/post_call_hooks.rs',
    reason:
      'core post-call hooks must delegate portable hook routing to agent-runtime while retaining concrete hook execution',
    patterns: [
      {
        regex: /\brun_successful_tool_post_call_hooks\b/,
        message: 'missing post-call hook executor runner delegation',
      },
      {
        regex: /\bSuccessfulToolPostCallHookExecutor\b/,
        message: 'missing post-call hook executor implementation',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/pipeline/tool_pipeline.rs',
    reason:
      'core tool pipeline must delegate portable confirmation planning and failure mapping to agent-runtime while retaining UI/channel side effects',
    patterns: [
      {
        regex: /\bresolve_tool_confirmation_plan\b/,
        message: 'missing tool confirmation plan delegation',
      },
      {
        regex: /\bresolve_confirmation_failure\b/,
        message: 'missing tool confirmation failure mapping delegation',
      },
      {
        regex: /\bresolve_confirmation_wait_result\b/,
        message: 'missing tool confirmation wait-result mapping delegation',
      },
      {
        regex: /\bToolConfirmationPlan::Await\b/,
        message: 'missing tool confirmation await-plan handling',
      },
      {
        regex: /\bshould_retry_tool_attempt\b/,
        message: 'missing tool-runtime retry decision delegation',
      },
      {
        regex: /\bretry_delay_ms\b/,
        message: 'missing tool-runtime retry backoff delegation',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/execution/types.rs',
    reason:
      'core execution types must preserve legacy import path while agent-runtime owns finish-reason event facts',
    patterns: [
      {
        regex: /bitfun_agent_runtime::events::FinishReason/,
        message: 'missing finish-reason compatibility re-export',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/events/types.rs',
    reason:
      'core event types must preserve legacy import path while agent-runtime owns session-state labels',
    patterns: [
      {
        regex: /bitfun_agent_runtime::events::session_state_label/,
        message: 'missing session-state label owner delegation',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/agents/prompt_builder/user_context.rs',
    reason:
      'core prompt_builder user_context path must stay a compatibility facade over agent-runtime',
    patterns: [
      {
        regex: /pub use bitfun_agent_runtime::prompt::\{UserContextPolicy, UserContextSection\};/,
        message: 'missing agent-runtime user-context compatibility re-export',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/session/prompt_cache.rs',
    reason:
      'core prompt_cache path must stay a compatibility facade over agent-runtime',
    patterns: [
      {
        regex: /pub use bitfun_agent_runtime::prompt_cache::\*;/,
        message: 'missing agent-runtime prompt-cache compatibility re-export',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/agents/mod.rs',
    reason:
      'core agent mode module must keep old import paths while agent-runtime owns shared mode profile facts',
    patterns: [
      {
        regex: /pub use bitfun_agent_runtime::agents::\{[\s\S]*mode_presentation_rank[\s\S]*resolve_mode_config_profile_id[\s\S]*shared_coding_mode_user_context_policy[\s\S]*SHARED_CODING_MODE_PROMPT_TEMPLATE[\s\S]*\};/,
        message: 'missing agent-runtime shared mode profile compatibility re-export',
      },
    ],
  },
  {
    path: 'src/crates/services/services-core/src/filesystem/mod.rs',
    reason:
      'services-core filesystem owner must expose local filesystem primitives behind a single module boundary',
    patterns: [
      {
        regex: /mod error;/,
        message: 'filesystem owner must expose its error boundary',
      },
      {
        regex: /mod operations;/,
        message: 'filesystem owner must expose local file operation primitives',
      },
      {
        regex: /mod tree;/,
        message: 'filesystem owner must expose local file tree/search primitives',
      },
      {
        regex: /pub use error::\{FileSystemError, FileSystemResult\};/,
        message: 'filesystem owner must re-export the unified filesystem error type',
      },
      {
        regex: /pub use service::FileSystemService;/,
        message: 'filesystem owner must keep the consolidated service facade',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/filesystem/service.rs',
    reason:
      'core filesystem service may keep remote-workspace overlay and BitFunError compatibility, but local filesystem owner must remain services-core',
    patterns: [
      {
        regex: /lookup_remote_connection_with_hint/,
        message: 'core filesystem wrapper must preserve remote workspace connection disambiguation',
      },
      {
        regex: /get_remote_workspace_manager/,
        message: 'core filesystem wrapper must preserve existing remote file service lookup',
      },
      {
        regex: /map_filesystem_error/,
        message: 'core filesystem wrapper must map services-core errors at the compatibility boundary',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/session/session_manager.rs',
    reason:
      'core session manager must keep forked Task prompt-cache and existing-context turn baselines until session branch ownership migrates',
    patterns: [
      {
        regex: /\bpub async fn clone_prompt_cache\b/,
        message: 'missing prompt cache clone runtime entry point',
      },
      {
        regex: /\bpub async fn start_dialog_turn_with_existing_context\b/,
        message: 'missing existing-context dialog turn entry point',
      },
      {
        regex: /\bstart_dialog_turn_with_existing_context_persists_turn_and_snapshot\b/,
        message: 'missing existing-context dialog turn persistence regression',
      },
      {
        regex: /\bclone_prompt_cache_copies_runtime_and_persisted_entries\b/,
        message: 'missing prompt cache clone runtime/disk regression',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/pipeline/tool_pipeline.rs',
    reason:
      'core tool pipeline must preserve latest-main truncation behavior through agent-tools delegation and keep per-tool denial behavior until tool runtime ownership migrates',
    patterns: [
      {
        regex: /\bbuild_tool_call_truncation_recovery_notice\b/,
        message: 'missing tool-call truncation recovery notice owner delegation',
      },
      {
        regex: /\btruncation_notice_for_interactive_tools_does_not_claim_file_write\b/,
        message: 'missing interactive-tool truncation recovery regression',
      },
      {
        regex: /\btruncation_notice_for_write_tools_keeps_write_continuation_guidance\b/,
        message: 'missing write-tool truncation recovery regression',
      },
      {
        regex: /\bdenied_tool_messages\b/,
        message: 'missing per-tool denial message propagation',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/restrictions.rs',
    reason:
      'core tool restrictions facade must preserve per-tool denial messages while runtime restrictions live in agent-tools',
    patterns: [
      {
        regex: /\bdenied_tool_messages\b/,
        message: 'missing per-tool denial message field propagation',
      },
      {
        regex: /\bcustom_deny_message_overrides_generic_runtime_error\b/,
        message: 'missing custom deny message regression',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/tool_result_storage.rs',
    reason:
      'core tool-result storage must keep explicit file flush until runtime artifact ownership migrates',
    patterns: [
      {
        regex: /\basync fn write_once\b/,
        message: 'missing single-write persistence helper',
      },
      {
        regex: /file\.flush\(\)\.await/,
        message: 'missing explicit persisted tool-result flush',
      },
    ],
  },
  {
    path: 'src/crates/execution/tool-execution/src/shell/mod.rs',
    reason:
      'tool-runtime must own reusable Bash shell execution policy, rendering, and background-result text helpers',
    patterns: [
      {
        regex: /\bpub fn banned_shell_command\b/,
        message: 'missing Bash banned-command policy owner',
      },
      {
        regex: /\bpub fn detect_osascript_keystroke_non_ascii\b/,
        message: 'missing Bash osascript keystroke guard owner',
      },
      {
        regex: /\bpub fn detect_osascript_im_app\b/,
        message: 'missing Bash IM AppleScript guard owner',
      },
      {
        regex: /\bpub fn command_for_working_directory\b/,
        message: 'missing Bash working-directory command wrapper owner',
      },
      {
        regex: /\bpub fn bash_noninteractive_env\b/,
        message: 'missing Bash noninteractive environment owner',
      },
      {
        regex: /\bpub fn render_local_shell_result\b/,
        message: 'missing local shell result rendering owner',
      },
      {
        regex: /\bpub fn render_remote_shell_result\b/,
        message: 'missing remote shell result rendering owner',
      },
      {
        regex: /\bpub fn format_background_command_delivery_text\b/,
        message: 'missing background command delivery text owner',
      },
    ],
  },
  {
    path: 'src/crates/execution/tool-execution/tests/tool_io_contracts.rs',
    reason:
      'tool-runtime shell owner must keep focused behavior-equivalence contracts for Bash execution helpers',
    patterns: [
      {
        regex: /\bbash_shell_owner_preserves_command_wrapping_and_env\b/,
        message: 'missing Bash command/env owner regression',
      },
      {
        regex: /\bbash_shell_owner_preserves_guard_and_result_rendering\b/,
        message: 'missing Bash guard/rendering owner regression',
      },
      {
        regex: /\bbash_shell_owner_preserves_background_delivery_texts\b/,
        message: 'missing Bash background-result text owner regression',
      },
    ],
  },
  {
    path: 'src/crates/execution/tool-execution/src/pipeline.rs',
    reason:
      'tool-runtime must own provider-neutral tool batching and retry policy while core keeps concrete execution state',
    patterns: [
      {
        regex: /\bpub struct ToolBatch\b/,
        message: 'missing tool batch DTO',
      },
      {
        regex: /\bpub fn partition_tool_batches\b/,
        message: 'missing tool batching policy',
      },
      {
        regex: /\bpub enum ToolExecutionErrorClass\b/,
        message: 'missing tool retry error class',
      },
      {
        regex: /\bpub struct ToolRetryAttemptFacts\b/,
        message: 'missing tool retry attempt facts',
      },
      {
        regex: /\bpub fn should_retry_tool_attempt\b/,
        message: 'missing tool retry decision policy',
      },
      {
        regex: /\bpub fn retry_delay_ms\b/,
        message: 'missing tool retry backoff policy',
      },
    ],
  },
  {
    path: 'src/crates/execution/tool-execution/tests/tool_pipeline_planning.rs',
    reason:
      'tool-runtime pipeline owner must keep behavior-equivalence contracts for batching and retry policy',
    patterns: [
      {
        regex: /\bpartitions_consecutive_concurrency_safe_tools_into_parallel_batches\b/,
        message: 'missing tool batching regression',
      },
      {
        regex: /\bretry_policy_preserves_attempt_limit_and_error_class_contract\b/,
        message: 'missing tool retry policy regression',
      },
    ],
  },
  {
    path: 'src/crates/services/services-integrations/src/mcp/server/connection.rs',
    reason:
      'services-integrations MCP connection must keep initialize-scoped timeout and channel-close cleanup until MCP owner migration is reviewed',
    patterns: [
      {
        regex: /\bsend_request_with_id\b/,
        message: 'missing stable local JSON-RPC request id path',
      },
      {
        regex: /\binitialize_timeout\b/,
        message: 'missing initialize-scoped timeout',
      },
      {
        regex: /notifications\/initialized/,
        message: 'missing MCP initialized notification',
      },
      {
        regex: /\bpending\.clear\(\)/,
        message: 'missing pending request waiter drain on channel close',
      },
      {
        regex: /\blocal_tool_calls_do_not_inherit_initialize_timeout\b/,
        message: 'missing local tool request timeout-scope regression',
      },
      {
        regex: /\blocal_initialize_uses_initialize_timeout\b/,
        message: 'missing local initialize timeout regression',
      },
    ],
  },
  {
    path: 'src/crates/services/services-integrations/src/mcp/protocol/transport.rs',
    reason:
      'services-integrations MCP local transport must keep explicit request ids and stdin flush semantics',
    patterns: [
      {
        regex: /\bpub async fn send_request_with_id\b/,
        message: 'missing explicit JSON-RPC request id send path',
      },
      {
        regex: /\.flush\(\)\s*\.await/,
        message: 'missing local MCP stdin flush',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/Cargo.toml',
    reason:
      'bitfun-core product-full must explicitly aggregate owner crate feature groups instead of forcing them through dependency declarations',
    patterns: [
      {
        regex:
          /bitfun-tool-packs = \{ path = "\.\.\/\.\.\/execution\/tool-provider-groups", default-features = false, optional = true \}/,
        message: 'bitfun-tool-packs dependency must stay optional and not force product-full outside the core feature graph',
      },
      {
        regex:
          /bitfun-services-integrations = \{ path = "\.\.\/\.\.\/services\/services-integrations", default-features = false, features = \["remote-ssh", "workspace-search"\] \}/,
        message:
          'bitfun-services-integrations dependency may keep remote workspace identity and workspace-search owner helpers but must not force product-full outside the core feature graph',
      },
      {
        regex:
          /bitfun-product-domains = \{ path = "\.\.\/\.\.\/contracts\/product-domains", default-features = false, optional = true \}/,
        message:
          'bitfun-product-domains dependency must stay optional and not force product-full outside the core feature graph',
      },
      {
        regex:
          /bitfun-product-capabilities = \{ path = "\.\.\/product-capabilities", default-features = false, optional = true \}/,
        message:
          'bitfun-product-capabilities dependency must stay optional and not force product-full outside the core feature graph',
      },
      {
        regex: /"dep:bitfun-tool-packs"/,
        message: 'core tool-packs feature must explicitly enable the optional dependency',
      },
      {
        regex: /"bitfun-tool-packs\/product-full"/,
        message: 'core product-full must explicitly enable tool pack product features',
      },
      {
        regex: /"bitfun-services-integrations\/product-full"/,
        message: 'core product-full must explicitly enable integration product features',
      },
      {
        regex: /"dep:bitfun-product-domains"/,
        message: 'core product-domains feature must explicitly enable the optional dependency',
      },
      {
        regex: /"dep:bitfun-product-capabilities"/,
        message:
          'core product-capabilities feature must explicitly enable the optional dependency',
      },
      {
        regex: /"bitfun-product-domains\/product-full"/,
        message: 'core product-full must explicitly enable product-domain features',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/lib.rs',
    reason:
      'no-default bitfun-core must keep product runtime surfaces behind explicit features',
    patterns: [
      {
        regex: /#\[cfg\(feature = "product-full"\)\]\s*pub mod agentic\b/s,
        message: 'agentic runtime must stay behind product-full for no-default builds',
      },
      {
        regex: /#\[cfg\(feature = "product-domains"\)\]\s*pub mod function_agents\b/s,
        message: 'function-agent product domain facade must stay behind product-domains',
      },
      {
        regex: /#\[cfg\(feature = "product-domains"\)\]\s*pub mod miniapp\b/s,
        message: 'MiniApp product domain facade must stay behind product-domains',
      },
      {
        regex: /#\[cfg\(feature = "service-integrations"\)\]\s*pub\(crate\) mod service_agent_runtime\b/s,
        message: 'service agent runtime owner assembly must stay behind service-integrations',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/mod.rs',
    reason:
      'service integration and agent-runtime surfaces must not compile in no-default core builds',
    patterns: [
      {
        regex: /#\[cfg\(feature = "service-integrations"\)\]\s*pub mod git\b/s,
        message: 'git service facade must stay behind service-integrations',
      },
      {
        regex: /#\[cfg\(feature = "service-integrations"\)\]\s*pub mod mcp\b/s,
        message: 'MCP service facade must stay behind service-integrations',
      },
      {
        regex: /#\[cfg\(feature = "service-integrations"\)\]\s*pub mod remote_connect\b/s,
        message: 'remote-connect service facade must stay behind service-integrations',
      },
      {
        regex: /#\[cfg\(feature = "service-integrations"\)\]\s*pub mod review_platform\b/s,
        message: 'review platform facade must stay behind service-integrations',
      },
      {
        regex: /#\[cfg\(feature = "product-full"\)\]\s*pub mod snapshot\b/s,
        message: 'snapshot service must stay behind product-full until tool-runtime ownership is split',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/config/mod.rs',
    reason:
      'mode config canonicalization depends on product agent/tool registries and must stay out of no-default builds',
    patterns: [
      {
        regex: /#\[cfg\(feature = "product-full"\)\]\s*pub mod mode_config_canonicalizer\b/s,
        message: 'mode config canonicalizer must stay behind product-full',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/workspace/manager.rs',
    reason:
      'workspace metadata may omit git worktree enrichment when service integrations are disabled',
    patterns: [
      {
        regex: /#\[cfg\(feature = "service-integrations"\)\]\s*use crate::service::git::GitService\b/s,
        message: 'GitService import must stay gated for no-default builds',
      },
      {
        regex: /#\[cfg\(not\(feature = "service-integrations"\)\)\]\s*\{\s*let _ = workspace_root;\s*return None;\s*\}/s,
        message: 'no-default worktree enrichment fallback must remain explicit',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/workspace_runtime/service.rs',
    reason:
      'workspace runtime binding helpers may depend on agentic runtime only in full product builds',
    patterns: [
      {
        regex: /#\[cfg\(feature = "product-full"\)\]\s*use crate::agentic::WorkspaceBinding\b/s,
        message: 'WorkspaceBinding import must stay gated for no-default builds',
      },
      {
        regex: /#\[cfg\(feature = "product-full"\)\]\s*pub async fn ensure_runtime_for_workspace_binding\b/s,
        message: 'WorkspaceBinding runtime helper must stay behind product-full',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/remote_ssh/mod.rs',
    reason:
      'core remote SSH runtime must keep concrete SSH dependencies behind the ssh-remote feature while preserving lightweight workspace identity helpers',
    patterns: [
      {
        regex: /#\[cfg\(not\(feature = "ssh-remote"\)\)\]\s*mod disabled\b/s,
        message: 'missing disabled remote SSH runtime surface for no-default builds',
      },
      {
        regex: /#\[cfg\(feature = "ssh-remote"\)\]\s*pub mod manager\b/s,
        message: 'remote SSH manager must stay gated behind ssh-remote',
      },
      {
        regex: /#\[cfg\(feature = "ssh-remote"\)\]\s*pub mod remote_fs\b/s,
        message: 'remote SSH filesystem runtime must stay gated behind ssh-remote',
      },
      {
        regex: /#\[cfg\(feature = "ssh-remote"\)\]\s*pub mod remote_terminal\b/s,
        message: 'remote SSH terminal runtime must stay gated behind ssh-remote',
      },
      {
        regex: /\bpub mod workspace_state\b/,
        message: 'remote workspace identity helpers must remain available without ssh-remote',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/remote_ssh/disabled.rs',
    reason:
      'no-default core builds must expose explicit unsupported remote SSH stubs instead of compiling russh-backed runtime code',
    patterns: [
      {
        regex: /Remote SSH support is disabled; enable the `ssh-remote` feature/,
        message: 'missing explicit disabled remote SSH diagnostic',
      },
      {
        regex: /\bpub struct SSHConnectionManager\b/,
        message: 'missing disabled SSH manager compatibility surface',
      },
      {
        regex: /\bpub struct RemoteFileService\b/,
        message: 'missing disabled remote file compatibility surface',
      },
      {
        regex: /\bpub struct RemoteTerminalManager\b/,
        message: 'missing disabled remote terminal compatibility surface',
      },
    ],
  },
  {
    path: 'src/crates/contracts/runtime-ports/src/lib.rs',
    reason:
      'runtime-ports must keep remote and subagent runtime boundary contracts DTO/trait-only',
    patterns: [
      {
        regex: /\bpub trait AgentTurnCancellationPort\b/,
        message: 'missing turn cancellation port contract',
      },
      {
        regex: /\bpub trait RemoteControlStatePort\b/,
        message: 'missing remote control state port contract',
      },
      {
        regex: /\bpub trait RuntimeEventSink\b/,
        message: 'missing runtime event sink contract',
      },
      {
        regex: /\bpub struct RemoteWorkspaceFacts\b/,
        message: 'missing remote workspace facts contract',
      },
      {
        regex: /\bpub trait RemoteWorkspaceRuntimeHost\b/,
        message: 'missing remote workspace runtime host contract',
      },
      {
        regex: /\bpub trait RemoteWorkspacePort\b/,
        message: 'missing remote workspace service port contract',
      },
      {
        regex: /\bpub trait RemoteWorkspaceFileRuntimeHost\b/,
        message: 'missing remote workspace file runtime host contract',
      },
      {
        regex: /\bpub trait RemoteProjectionPort\b/,
        message: 'missing remote projection service port contract',
      },
      {
        regex: /\bpub trait RemoteInitialSyncRuntimeHost\b/,
        message: 'missing remote initial sync runtime host contract',
      },
      {
        regex: /\bremote_workspace_contracts_preserve_workspace_and_session_facts\b/,
        message: 'missing remote workspace contract regression',
      },
      {
        regex: /\bremote_projection_contract_preserves_file_chunk_identity\b/,
        message: 'missing remote projection contract regression',
      },
      {
        regex: /\bpub trait WorkspaceFileSystem\b/,
        message: 'missing workspace file-system port contract',
      },
      {
        regex: /\bpub trait WorkspaceShell\b/,
        message: 'missing workspace shell port contract',
      },
      {
        regex: /\bpub struct WorkspaceServices\b/,
        message: 'missing workspace services bundle contract',
      },
      {
        regex: /\bpub struct WorkspaceCommandOptions\b/,
        message: 'missing workspace command options contract',
      },
      {
        regex: /\bpub struct WorkspaceCommandResult\b/,
        message: 'missing workspace command result contract',
      },
      {
        regex: /\bpub struct WorkspaceDirEntry\b/,
        message: 'missing workspace dir-entry contract',
      },
      {
        regex: /\bworkspace_services_contract_is_runtime_port_owned\b/,
        message: 'missing workspace service ownership regression',
      },
      {
        regex: /\bpub fn remote_image\b/,
        message: 'missing remote image attachment helper contract',
      },
      {
        regex: /\bpub type DialogTriggerSource = AgentSubmissionSource\b/,
        message: 'missing dialog trigger source compatibility contract',
      },
      {
        regex: /\bdialog_trigger_source_reuses_agent_submission_source_contract\b/,
        message: 'missing dialog trigger source alias regression',
      },
      {
        regex: /\bpub enum DialogQueuePriority\b/,
        message: 'missing dialog queue priority contract',
      },
      {
        regex: /\bpub struct DialogSubmissionPolicy\b/,
        message: 'missing dialog submission policy contract',
      },
      {
        regex: /\bdialog_submission_policy_preserves_current_surface_queue_defaults\b/,
        message: 'missing dialog submission policy regression',
      },
      {
        regex: /\bpub enum DialogSubmitOutcome\b/,
        message: 'missing dialog submit outcome contract',
      },
      {
        regex: /\bdialog_submit_outcome_preserves_started_and_queued_fields\b/,
        message: 'missing dialog submit outcome regression',
      },
      {
        regex: /\bpub enum DialogSessionStateFact\b/,
        message: 'missing dialog session state fact contract',
      },
      {
        regex: /\bpub struct DialogSubmitQueueFacts\b/,
        message: 'missing dialog submit queue facts contract',
      },
      {
        regex: /\bpub enum DialogSubmitQueueAction\b/,
        message: 'missing dialog submit queue action contract',
      },
      {
        regex: /\bpub const fn dialog_policy_may_preempt\b/,
        message: 'missing dialog preempt policy contract',
      },
      {
        regex: /\bpub const fn resolve_dialog_submit_queue_action\b/,
        message: 'missing dialog submit queue action resolver',
      },
      {
        regex: /\bdialog_submit_queue_action_preserves_current_scheduler_routing_policy\b/,
        message: 'missing dialog submit queue action regression',
      },
      {
        regex: /\bpub fn should_suppress_agent_session_cancelled_reply\b/,
        message: 'missing agent-session cancel suppression contract',
      },
      {
        regex: /\bpub enum DialogTurnOutcomeKind\b/,
        message: 'missing dialog turn outcome kind contract',
      },
      {
        regex: /\bpub const fn should_skip_agent_session_reply\b/,
        message: 'missing agent-session reply skip contract',
      },
      {
        regex: /\bagent_session_reply_decisions_preserve_cancel_suppression_boundary\b/,
        message: 'missing agent-session reply decision regression',
      },
      {
        regex: /\bpub struct AgentSessionReplyRoute\b/,
        message: 'missing agent session reply route contract',
      },
      {
        regex: /\bagent_session_reply_route_keeps_requester_fields\b/,
        message: 'missing agent session reply route regression',
      },
      {
        regex: /\bpub enum DialogSteerOutcome\b/,
        message: 'missing dialog steer outcome contract',
      },
      {
        regex: /\bdialog_steer_outcome_preserves_buffered_fields\b/,
        message: 'missing dialog steer outcome regression',
      },
      {
        regex: /\bpub enum RoundInjectionKind\b/,
        message: 'missing round injection kind contract',
      },
      {
        regex: /\bpub enum RoundInjectionTarget\b/,
        message: 'missing round injection target contract',
      },
      {
        regex: /\bpub struct RoundInjection\b/,
        message: 'missing round injection message contract',
      },
      {
        regex: /\bpub trait DialogRoundPreemptSource\b/,
        message: 'missing dialog round preempt source contract',
      },
      {
        regex: /\bpub trait DialogRoundInjectionSource\b/,
        message: 'missing dialog round injection source contract',
      },
      {
        regex: /\bround_injection_contract_keeps_kind_and_target_identity\b/,
        message: 'missing round injection contract regression',
      },
      {
        regex: /\bround_injection_source_contract_drains_portable_injections\b/,
        message: 'missing round injection source contract regression',
      },
      {
        regex: /\bpub enum ThreadGoalStatus\b/,
        message: 'missing thread goal status contract',
      },
      {
        regex: /\bpub struct ThreadGoal\b/,
        message: 'missing thread goal contract',
      },
      {
        regex: /\bpub struct SetThreadGoalResult\b/,
        message: 'missing set thread goal result contract',
      },
      {
        regex: /\bpub struct ThreadGoalContinuationPlan\b/,
        message: 'missing thread goal continuation plan contract',
      },
      {
        regex: /\bpub struct ThreadGoalToolResponse\b/,
        message: 'missing thread goal tool response contract',
      },
      {
        regex: /\bthread_goal_active_status_includes_budget_limited\b/,
        message: 'missing thread goal status contract regression',
      },
      {
        regex: /\bthread_goal_tool_response_serializes_optional_fields\b/,
        message: 'missing thread goal tool response wire-shape regression',
      },
      {
        regex: /\bpub struct CompressionContract\b/,
        message: 'missing compression contract',
      },
      {
        regex: /\bpub struct CompressionContractItem\b/,
        message: 'missing compression contract item',
      },
      {
        regex: /\bcompression_contract_renders_model_visible_fields\b/,
        message: 'missing compression contract rendering regression',
      },
      {
        regex: /\bpub struct RelatedPath\b/,
        message: 'missing related path request-context contract',
      },
      {
        regex: /\brelated_path_serializes_as_request_context_fact\b/,
        message: 'missing related path serialization regression',
      },
      {
        regex: /\bpub struct DelegationPolicy\b/,
        message: 'missing delegation policy contract',
      },
      {
        regex: /\bpub enum SubagentContextMode\b/,
        message: 'missing subagent context mode contract',
      },
      {
        regex: /\bdelegation_policy_child_blocks_recursive_spawn_without_losing_depth\b/,
        message: 'missing delegation policy contract regression',
      },
      {
        regex: /\bsubagent_context_mode_preserves_fork_wire_value\b/,
        message: 'missing subagent context mode contract regression',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/subagent_runtime/mod.rs',
    reason:
      'core subagent runtime must preserve legacy import path while runtime-ports owns portable subagent contracts',
    patterns: [
      {
        regex: /pub\(crate\) use bitfun_runtime_ports::\{DelegationPolicy, SubagentContextMode\};/,
        message: 'missing core compatibility re-export for subagent runtime contracts',
      },
      {
        regex: /pub\(crate\) mod queue_timing;/,
        message: 'queue timing must remain core-owned until it has a reviewed non-DTO owner',
      },
    ],
  },
  {
    path: 'src/crates/execution/tool-contracts/src/framework.rs',
    reason:
      'agent-tools may own pure and generic prompt-visible tool contracts and provider-neutral execution gate policy without owning product registry or concrete execution',
    patterns: [
      {
        regex: /\bpub const GET_TOOL_SPEC_TOOL_NAME\b/,
        message: 'missing shared GetToolSpec manifest name contract',
      },
      {
        regex: /\bpub enum ToolExposure\b/,
        message: 'missing lightweight tool exposure contract',
      },
      {
        regex: /\bpub struct ToolManifestPolicyTool\b/,
        message: 'missing pure tool manifest policy input contract',
      },
      {
        regex: /\bpub fn resolve_tool_manifest_policy\b/,
        message: 'missing pure tool manifest policy resolver',
      },
      {
        regex: /\bfn default_exposure\b/,
        message: 'missing generic tool exposure contract',
      },
      {
        regex: /\bpub fn build_tool_manifest_policy_tools\b/,
        message: 'missing registry snapshot to manifest policy input helper',
      },
      {
        regex: /\bpub fn build_collapsed_tool_stub_definition\b/,
        message: 'missing collapsed-tool prompt stub contract',
      },
      {
        regex: /\bpub enum PromptVisibleToolManifestItem\b/,
        message: 'missing prompt-visible manifest item contract',
      },
      {
        regex: /\bpub fn build_prompt_visible_tool_manifest_definitions\b/,
        message: 'missing prompt-visible manifest definition builder',
      },
      {
        regex: /\bpub trait ContextualToolManifestItem\b/,
        message: 'missing generic contextual manifest item adapter contract',
      },
      {
        regex: /\bpub trait ToolCatalogSnapshotProvider\b/,
        message: 'missing generic tool catalog snapshot provider contract',
      },
      {
        regex: /\bpub trait GetToolSpecCatalogProvider\b/,
        message: 'missing generic GetToolSpec catalog provider contract',
      },
      {
        regex: /\bpub struct ContextualVisibleTools\b/,
        message: 'missing generic contextual visible-tools result contract',
      },
      {
        regex: /\bpub struct ContextualToolManifest\b/,
        message: 'missing generic contextual tool manifest result contract',
      },
      {
        regex: /\bpub async fn resolve_contextual_visible_tools\b/,
        message: 'missing generic contextual visible-tools resolver',
      },
      {
        regex: /\bpub async fn resolve_contextual_tool_manifest\b/,
        message: 'missing generic contextual tool manifest resolver',
      },
      {
        regex: /\bpub async fn resolve_contextual_visible_tools_from_provider\b/,
        message: 'missing provider-backed contextual visible-tools resolver',
      },
      {
        regex: /\bpub async fn resolve_contextual_tool_manifest_from_provider\b/,
        message: 'missing provider-backed contextual manifest resolver',
      },
      {
        regex: /\bpub async fn build_get_tool_spec_catalog_description_from_provider\b/,
        message: 'missing provider-backed GetToolSpec catalog description builder',
      },
      {
        regex: /\bpub async fn resolve_get_tool_spec_detail_from_provider\b/,
        message: 'missing provider-backed GetToolSpec detail resolver',
      },
      {
        regex: /\bpub fn build_get_tool_spec_description\b/,
        message: 'missing pure GetToolSpec prompt description contract',
      },
      {
        regex: /\bpub struct GetToolSpecCollapsedToolSummary\b/,
        message: 'missing pure GetToolSpec collapsed catalog summary',
      },
      {
        regex: /\bpub struct GetToolSpecDetail\b/,
        message: 'missing pure GetToolSpec detail contract',
      },
      {
        regex: /\bpub fn summarize_get_tool_spec_collapsed_tools\b/,
        message: 'missing pure GetToolSpec collapsed summary helper',
      },
      {
        regex: /\bpub async fn resolve_get_tool_spec_detail\b/,
        message: 'missing generic GetToolSpec detail resolver',
      },
      {
        regex: /\bpub fn build_get_tool_spec_catalog_description\b/,
        message: 'missing pure GetToolSpec catalog description builder',
      },
      {
        regex: /\bpub fn get_tool_spec_input_schema\b/,
        message: 'missing pure GetToolSpec input schema contract',
      },
      {
        regex: /\bpub fn get_tool_spec_short_description\b/,
        message: 'missing pure GetToolSpec short description contract',
      },
      {
        regex: /\bpub fn render_get_tool_spec_tool_use_message\b/,
        message: 'missing pure GetToolSpec tool-use message renderer',
      },
      {
        regex: /\bpub fn get_tool_spec_is_readonly\b/,
        message: 'missing pure GetToolSpec readonly metadata contract',
      },
      {
        regex: /\bpub fn get_tool_spec_is_concurrency_safe\b/,
        message: 'missing pure GetToolSpec concurrency metadata contract',
      },
      {
        regex: /\bpub fn get_tool_spec_needs_permissions\b/,
        message: 'missing pure GetToolSpec permission metadata contract',
      },
      {
        regex: /\bpub fn validate_get_tool_spec_input\b/,
        message: 'missing pure GetToolSpec input validation contract',
      },
      {
        regex: /\bpub fn build_get_tool_spec_assistant_detail\b/,
        message: 'missing pure GetToolSpec assistant detail rendering contract',
      },
      {
        regex: /\bpub fn build_get_tool_spec_duplicate_load_result\b/,
        message: 'missing pure GetToolSpec duplicate-load result assembly contract',
      },
      {
        regex: /\bpub fn build_get_tool_spec_detail_result\b/,
        message: 'missing pure GetToolSpec detail result assembly contract',
      },
      {
        regex: /\bpub enum GetToolSpecExecutionPlan\b/,
        message: 'missing pure GetToolSpec execution plan contract',
      },
      {
        regex: /\bpub enum GetToolSpecExecutionError\b/,
        message: 'missing pure GetToolSpec execution error contract',
      },
      {
        regex: /\bpub fn resolve_get_tool_spec_execution_plan\b/,
        message: 'missing pure GetToolSpec execution plan resolver',
      },
      {
        regex: /\bpub async fn resolve_get_tool_spec_execution_result_from_provider\b/,
        message: 'missing provider-backed GetToolSpec execution result resolver',
      },
      {
        regex: /\bpub struct GetToolSpecRuntime\b/,
        message: 'missing provider-backed GetToolSpec runtime facade',
      },
      {
        regex: /\bpub async fn call_results\b/,
        message: 'missing provider-backed GetToolSpec Tool-result vector adapter facade',
      },
      {
        regex: /\bpub struct GetToolSpecLoadObservation\b/,
        message: 'missing pure GetToolSpec load observation contract',
      },
      {
        regex: /\bpub fn collect_loaded_collapsed_tool_names\b/,
        message: 'missing pure collapsed-tool load collection contract',
      },
      {
        regex: /\bpub enum CollapsedToolUsageError\b/,
        message: 'missing collapsed-tool execution gate error contract',
      },
      {
        regex: /\bpub enum ToolExecutionAccessError\b/,
        message: 'missing tool execution allowed-list gate error contract',
      },
      {
        regex: /\bpub fn validate_tool_allowed_by_list\b/,
        message: 'missing tool execution allowed-list gate policy',
      },
      {
        regex: /\bpub fn validate_collapsed_tool_usage\b/,
        message: 'missing collapsed-tool execution gate policy',
      },
      {
        regex: /\bpub fn is_tool_path_allowed_by_resolved_roots\b/,
        message: 'missing provider-neutral path policy root matcher',
      },
      {
        regex: /\bpub fn build_tool_path_policy_denial_message\b/,
        message: 'missing provider-neutral path policy denial message',
      },
      {
        regex: /\bpub fn resolve_tool_path_with_context\b/,
        message: 'missing provider-neutral tool path resolution owner',
      },
      {
        regex: /\bpub fn tool_path_is_effectively_absolute\b/,
        message: 'missing provider-neutral tool path absolute check',
      },
      {
        regex: /\bpub fn build_tool_runtime_artifact_reference\b/,
        message: 'missing provider-neutral runtime artifact reference builder',
      },
      {
        regex: /\bpub fn build_tool_session_runtime_artifact_reference\b/,
        message: 'missing provider-neutral session runtime artifact reference builder',
      },
      {
        regex: /\bpub fn sort_tool_manifest_definitions\b/,
        message: 'missing prompt-visible manifest ordering helper',
      },
      {
        regex: /\bpub struct StaticToolProviderGroup\b/,
        message: 'missing generic static provider group container',
      },
      {
        regex: /\bpub trait StaticToolProviderPlan\b/,
        message: 'missing provider-neutral static tool provider plan contract',
      },
      {
        regex: /\bpub trait StaticToolProviderFactory\b/,
        message: 'missing provider-neutral static tool factory contract',
      },
      {
        regex: /\bpub enum StaticToolMaterializationError\b/,
        message: 'missing provider-neutral static tool materialization error',
      },
      {
        regex: /\bpub fn materialize_static_tool_provider_groups\b/,
        message: 'missing provider-neutral static tool materializer',
      },
      {
        regex: /\bpub struct ToolRuntimeAssembly\b/,
        message: 'missing generic tool runtime assembly owner',
      },
      {
        regex: /\bpub type ToolDecoratorRef\b/,
        message: 'missing generic tool decorator reference contract',
      },
      {
        regex: /\bpub trait SnapshotToolWrapper\b/,
        message: 'missing generic snapshot wrapper port',
      },
      {
        regex: /\bpub struct SnapshotToolDecorator\b/,
        message: 'missing generic snapshot decorator adapter',
      },
      {
        regex: /\bcreate_registry_from_static_providers\b/,
        message: 'missing generic static-provider runtime assembly helper',
      },
      {
        regex: /\bcreate_registry_from_static_provider_plans\b/,
        message: 'missing generic static-provider plan-to-registry assembly helper',
      },
      {
        regex: /\bpub fn is_tool_collapsed\b/,
        message: 'missing generic collapsed-tool registry query',
      },
      {
        regex: /\bpub fn get_collapsed_tool_names\b/,
        message: 'missing generic collapsed-tool registry catalog query',
      },
      {
        regex: /\bpub async fn resolve_readonly_enabled_tools\b/,
        message: 'missing generic readonly enabled tool filter',
      },
      {
        regex: /\bpub struct ToolCatalogRuntime\b/,
        message: 'missing provider-backed tool catalog runtime facade',
      },
    ],
  },
  {
    path: 'src/crates/execution/tool-contracts/src/file_guidance.rs',
    reason: 'agent-tools owns provider-neutral file tool guidance marker contracts',
    patterns: [
      {
        regex: /\bpub const FILE_TOOL_GUIDANCE_PREFIX\b/,
        message: 'missing file tool guidance marker prefix',
      },
      {
        regex: /\bpub fn file_tool_guidance_message\b/,
        message: 'missing file tool guidance message helper',
      },
      {
        regex: /\bpub fn is_file_tool_guidance_message\b/,
        message: 'missing file tool guidance classifier',
      },
    ],
  },
  {
    path: 'src/crates/execution/tool-contracts/src/file_read_freshness.rs',
    reason: 'agent-tools owns pure file-read freshness policy for Read/Edit/Write guardrails',
    patterns: [
      {
        regex: /\bpub struct FileReadFreshnessFacts\b/,
        message: 'missing file-read freshness facts contract',
      },
      {
        regex: /\bpub fn normalize_tool_file_content\b/,
        message: 'missing provider-neutral file content normalization helper',
      },
      {
        regex: /\bpub fn file_read_facts_content_matches\b/,
        message: 'missing file-read content equivalence helper',
      },
      {
        regex: /\bpub fn file_read_facts_are_fresh\b/,
        message: 'missing file-read freshness policy helper',
      },
    ],
  },
  {
    path: 'src/crates/execution/tool-contracts/src/tool_result_storage.rs',
    reason:
      'agent-tools owns pure oversized tool-result storage policy and rendering without session IO',
    patterns: [
      {
        regex: /\bpub struct ToolResultStoragePolicy\b/,
        message: 'missing provider-neutral tool result storage policy',
      },
      {
        regex: /\bpub struct PersistedToolOutput\b/,
        message: 'missing persisted tool output render contract',
      },
      {
        regex: /\bpub struct ToolResultPersistenceCandidate\b/,
        message: 'missing provider-neutral persistence candidate contract',
      },
      {
        regex: /\bpub fn select_tool_result_indices_for_persistence\b/,
        message: 'missing round-budget persistence candidate selector',
      },
      {
        regex: /\bpub fn sanitize_tool_result_file_component\b/,
        message: 'missing tool-result file component sanitizer',
      },
      {
        regex: /\bpub fn generate_tool_result_preview\b/,
        message: 'missing tool-result preview generator',
      },
      {
        regex: /\bpub fn count_tool_result_lines\b/,
        message: 'missing tool-result line counter',
      },
      {
        regex: /\bpub fn build_persisted_tool_output_message\b/,
        message: 'missing persisted-output message renderer',
      },
      {
        regex: /\bpub fn tool_result_is_persisted_output\b/,
        message: 'missing persisted-output classifier',
      },
    ],
  },
  {
    path: 'src/crates/execution/tool-contracts/src/tool_execution_presentation.rs',
    reason:
      'agent-tools owns provider-neutral tool execution result and error presentation helpers',
    patterns: [
      {
        regex: /\bpub const TOOL_ERROR_ARGUMENTS_PREVIEW_BYTES\b/,
        message: 'missing tool error argument preview limit contract',
      },
      {
        regex: /\bpub const USER_STEERING_INTERRUPTED_MESSAGE\b/,
        message: 'missing steering-interrupted assistant message contract',
      },
      {
        regex: /\bpub struct ToolExecutionErrorPresentation\b/,
        message: 'missing tool execution error presentation DTO',
      },
      {
        regex: /\bpub fn render_tool_result_for_assistant\b/,
        message: 'missing tool result assistant rendering helper',
      },
      {
        regex: /\bpub fn truncate_tool_arguments_preview\b/,
        message: 'missing structured tool argument preview helper',
      },
      {
        regex: /\bpub fn truncate_raw_tool_arguments_preview\b/,
        message: 'missing raw tool argument preview helper',
      },
      {
        regex: /\bpub fn build_tool_execution_error_presentation\b/,
        message: 'missing tool execution error presentation helper',
      },
      {
        regex: /\bpub fn build_user_steering_interrupted_presentation\b/,
        message: 'missing steering-interrupted presentation helper',
      },
      {
        regex: /\bpub fn build_invalid_tool_call_error_message\b/,
        message: 'missing invalid tool call error message helper',
      },
      {
        regex: /\bpub fn is_write_like_tool_name\b/,
        message: 'missing write-like tool classification helper',
      },
      {
        regex: /\bpub fn build_tool_call_truncation_recovery_notice\b/,
        message: 'missing truncation recovery notice helper',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/coordination/coordinator.rs',
    reason:
      'core must keep current coordinator port adapters and attachment guard until remote runtime migration is reviewed',
    patterns: [
      {
        regex: /impl bitfun_runtime_ports::AgentSubmissionPort for ConversationCoordinator/,
        message: 'missing agent submission port adapter',
      },
      {
        regex: /impl bitfun_runtime_ports::SessionTranscriptReader for ConversationCoordinator/,
        message: 'missing session transcript reader adapter',
      },
      {
        regex: /impl bitfun_runtime_ports::AgentTurnCancellationPort for ConversationCoordinator/,
        message: 'missing turn cancellation port adapter',
      },
      {
        regex: /impl bitfun_runtime_ports::RemoteControlStatePort for ConversationCoordinator/,
        message: 'missing remote control state port adapter',
      },
      {
        regex: /agent submission port does not yet accept generic attachments/,
        message: 'missing generic attachment guard on agent submission port',
      },
      {
        regex: /pub use bitfun_runtime_ports::DialogTriggerSource;/,
        message: 'missing dialog trigger source compatibility re-export',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/coordination/scheduler.rs',
    reason:
      'core scheduler must preserve legacy submission policy import path while runtime-ports owns portable dialog policy contracts',
    patterns: [
      {
        regex:
          /pub use bitfun_runtime_ports::\{[\s\S]*AgentSessionReplyRoute[\s\S]*DialogQueuePriority[\s\S]*DialogSteerOutcome[\s\S]*DialogSubmissionPolicy[\s\S]*DialogSubmitOutcome[\s\S]*\};/,
        message: 'missing dialog submission policy compatibility re-export',
      },
      {
        regex:
          /use bitfun_runtime_ports::\{(?=[\s\S]*DialogSessionStateFact)(?=[\s\S]*DialogSubmitQueueAction)(?=[\s\S]*DialogSubmitQueueFacts)(?=[\s\S]*resolve_dialog_submit_queue_action)[\s\S]*\};/,
        message: 'missing dialog scheduler decision contract import',
      },
      {
        regex: /\bdialog_policy_may_preempt\b/,
        message: 'missing dialog preempt policy owner delegation',
      },
      {
        regex:
          /use bitfun_agent_runtime::scheduler::\{(?=[\s\S]*ActiveDialogTurn)(?=[\s\S]*ActiveDialogTurnStore)(?=[\s\S]*AgentSessionReplyAction)(?=[\s\S]*AgentSessionReplyPlan)(?=[\s\S]*BackgroundDeliveryAction)(?=[\s\S]*BackgroundDeliveryFacts)(?=[\s\S]*BackgroundInjectionKind)(?=[\s\S]*DialogReplySuppressionSet)(?=[\s\S]*DialogSteeringAction)(?=[\s\S]*DialogTurnQueue)(?=[\s\S]*SessionAbortFlags)(?=[\s\S]*resolve_agent_session_reply_action)(?=[\s\S]*resolve_background_delivery_action)(?=[\s\S]*resolve_background_delivery_injection)(?=[\s\S]*resolve_dialog_steering_action)[\s\S]*\};/,
        message: 'missing agent-runtime scheduler owner imports',
      },
      {
        regex: /\bBackgroundResult\b/,
        message: 'missing background-result injection owner delegation',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/round_preempt.rs',
    reason:
      'core round preempt runtime must preserve legacy import paths while runtime-ports owns portable contracts and agent-runtime owns round-boundary state',
    patterns: [
      {
        regex:
          /pub use bitfun_agent_runtime::scheduler::\{[\s\S]*DialogRoundInjectionInterrupt[\s\S]*SessionRoundInjectionBuffer[\s\S]*SessionRoundYieldFlags[\s\S]*\};/,
        message: 'missing agent-runtime round-boundary state compatibility re-export',
      },
      {
        regex:
          /pub use bitfun_runtime_ports::\{[\s\S]*DialogRoundInjectionSource[\s\S]*DialogRoundPreemptSource[\s\S]*RoundInjection[\s\S]*RoundInjectionKind[\s\S]*RoundInjectionTarget[\s\S]*\};/,
        message: 'missing round injection compatibility re-export',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/goal_mode/mod.rs',
    reason:
      'core goal mode must preserve legacy import paths while runtime-ports owns portable contracts and agent-runtime owns runtime decisions',
    patterns: [
      {
        regex:
          /pub use bitfun_runtime_ports::\{[\s\S]*SetThreadGoalResult[\s\S]*ThreadGoal[\s\S]*ThreadGoalContinuationPlan[\s\S]*ThreadGoalStatus[\s\S]*ThreadGoalToolResponse[\s\S]*GOAL_MODE_METADATA_KEY[\s\S]*MAX_CONTEXT_SUMMARY_CHARS[\s\S]*MAX_THREAD_GOAL_OBJECTIVE_CHARS[\s\S]*THREAD_GOAL_METADATA_KEY[\s\S]*\};/,
        message: 'missing thread goal compatibility re-export',
      },
      {
        regex:
          /pub use bitfun_agent_runtime::thread_goal::\{[\s\S]*build_thread_goal_continuation_plan[\s\S]*goal_tool_response[\s\S]*should_skip_goal_for_turn[\s\S]*ThreadGoalRuntime[\s\S]*\};/,
        message: 'missing thread goal runtime owner compatibility re-export',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/core/message.rs',
    reason:
      'core message model must preserve legacy compression contract import path while runtime-ports owns portable compaction facts',
    patterns: [
      {
        regex: /pub use bitfun_runtime_ports::\{CompressionContract, CompressionContractItem\};/,
        message: 'missing compression contract compatibility re-export',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/workspace/manager.rs',
    reason:
      'core workspace manager must preserve legacy related-path import path while runtime-ports owns portable request-context facts',
    patterns: [
      {
        regex: /pub use bitfun_runtime_ports::RelatedPath;/,
        message: 'missing related path compatibility re-export',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service_agent_runtime.rs',
    reason:
      'core service/agent runtime owner must centralize concrete remote-connect and agent runtime port bindings without moving runtime behavior',
    patterns: [
      {
        regex: /\bpub\(crate\) struct CoreServiceAgentRuntime\b/,
        message: 'missing core service/agent runtime owner type',
      },
      {
        regex: /\bfn remote_dialog_host\b/,
        message: 'missing remote dialog host owner factory',
      },
      {
        regex: /\bfn remote_cancel_host\b/,
        message: 'missing remote cancel host owner factory',
      },
      {
        regex: /\bfn remote_image_context\b/,
        message: 'missing remote image context owner adapter',
      },
      {
        regex: /\bfn load_remote_model_catalog\b/,
        message: 'missing remote model catalog owner adapter',
      },
      {
        regex: /\bRemoteModelCatalogFacts\b/,
        message: 'missing remote model catalog fact projection',
      },
      {
        regex: /\bRemoteModelCapabilityFact\b/,
        message: 'missing remote model capability fact projection',
      },
      {
        regex: /\bRemoteReasoningModeFact\b/,
        message: 'missing remote reasoning mode fact projection',
      },
      {
        regex: /\bbuild_remote_model_catalog\b/,
        message: 'missing remote model catalog assembly delegation',
      },
      {
        regex: /\bfn update_remote_session_model\b/,
        message: 'missing remote session model update owner adapter',
      },
      {
        regex: /\bfn normalize_remote_session_model_id\b/,
        message: 'missing remote session model id normalization regression hook',
      },
      {
        regex: /\bnormalize_remote_session_model_id_contract\b/,
        message: 'missing remote session model id owner delegation',
      },
      {
        regex: /\bfn normalize_remote_model_selection\b/,
        message: 'missing remote model selection normalization regression hook',
      },
      {
        regex: /\bnormalize_remote_model_selection_contract\b/,
        message: 'missing remote model selection owner delegation',
      },
      {
        regex: /\bfn remote_chat_messages_from_turns\b/,
        message: 'missing remote chat history conversion owner adapter',
      },
      {
        regex: /\bRemoteDialogSchedulerOutcomeFact\b/,
        message: 'missing remote dialog scheduler outcome fact projection',
      },
      {
        regex: /\bremote_dialog_submit_outcome_from_scheduler\b/,
        message: 'missing remote dialog submit outcome assembly delegation',
      },
      {
        regex: /\bRemoteChatHistoryTurn\b/,
        message: 'missing remote chat history owner DTO projection',
      },
      {
        regex: /\bbuild_remote_chat_messages\b/,
        message: 'missing remote chat history assembly delegation',
      },
      {
        regex: /\bfn strip_remote_user_input_tags\b/,
        message: 'missing remote user input display cleanup owner adapter',
      },
      {
        regex: /\bfn compress_remote_chat_data_url_for_mobile\b/,
        message: 'missing remote chat thumbnail compression owner adapter',
      },
      {
        regex: /\bfn load_remote_chat_messages\b/,
        message: 'missing remote chat history persistence owner adapter',
      },
      {
        regex: /\bfn agent_submission_port\b/,
        message: 'missing agent submission port owner binding',
      },
      {
        regex: /\bfn agent_turn_cancellation_port\b/,
        message: 'missing agent turn cancellation port owner binding',
      },
      {
        regex: /\bfn remote_control_state_port\b/,
        message: 'missing remote control state port owner binding',
      },
      {
        regex: /\bCoreRemoteDialogRuntimeHost\b/,
        message: 'missing core remote dialog host binding',
      },
      {
        regex: /\bCoreRemoteCancelRuntimeHost\b/,
        message: 'missing core remote cancel host binding',
      },
      {
        regex: /\bCoreRemoteWorkspaceFileRuntimeHost\b/,
        message: 'missing core remote workspace file host binding',
      },
      {
        regex: /\bCoreRemoteSessionTrackerHost\b/,
        message: 'missing core remote session tracker host binding',
      },
      {
        regex: /\bCoreRemoteWorkspaceRuntimeHost\b/,
        message: 'missing core remote workspace runtime host binding',
      },
      {
        regex: /\bCoreRemoteSessionRuntimeHost\b/,
        message: 'missing core remote session runtime host binding',
      },
      {
        regex: /\bCoreRemotePollRuntimeHost\b/,
        message: 'missing core remote poll runtime host binding',
      },
      {
        regex: /\bCoreRemoteInteractionRuntimeHost\b/,
        message: 'missing core remote interaction runtime host binding',
      },
      {
        regex: /\bRemoteExecutionDispatcher\b/,
        message: 'missing remote execution dispatcher binding',
      },
      {
        regex: /\bimpl RemoteDialogRuntimeHost for CoreRemoteDialogRuntimeHost\b/,
        message: 'missing remote dialog host adapter implementation in runtime owner',
      },
      {
        regex: /\bimpl RemoteCancelRuntimeHost for CoreRemoteCancelRuntimeHost\b/,
        message: 'missing remote cancel host adapter implementation in runtime owner',
      },
      {
        regex: /\bimpl RemoteWorkspaceFileRuntimeHost for CoreRemoteWorkspaceFileRuntimeHost\b/,
        message: 'missing remote workspace file host adapter implementation in runtime owner',
      },
      {
        regex: /\bimpl RemoteSessionTrackerHost for CoreRemoteSessionTrackerHost\b/,
        message: 'missing remote tracker host adapter implementation in runtime owner',
      },
      {
        regex: /\bImageContextData\b/,
        message: 'missing core image context binding',
      },
      {
        regex: /\bRemoteImageContextAdapter\b/,
        message: 'missing remote image context adapter implementation',
      },
      {
        regex: /\bAgentSubmissionPort\b/,
        message: 'missing agent submission port binding',
      },
      {
        regex: /\bAgentTurnCancellationPort\b/,
        message: 'missing agent turn cancellation port contract guard',
      },
      {
        regex: /\bRemoteControlStatePort\b/,
        message: 'missing remote control state port contract guard',
      },
      {
        regex: /\bSessionTranscriptReader\b/,
        message: 'missing session transcript reader contract guard',
      },
      {
        regex: /\bcore_service_agent_runtime_owner_keeps_coordinator_port_contracts\b/,
        message: 'missing coordinator runtime port contract regression',
      },
      {
        regex: /\bcore_service_agent_runtime_owner_normalizes_remote_session_model_ids\b/,
        message: 'missing remote session model id normalization regression',
      },
      {
        regex: /\bcore_service_agent_runtime_owner_normalizes_remote_model_selection_aliases\b/,
        message: 'missing remote model selection alias regression',
      },
      {
        regex: /\bcore_service_agent_runtime_owner_preserves_remote_chat_history_shape\b/,
        message: 'missing remote chat history conversion regression',
      },
      {
        regex: /\bcore_service_agent_runtime_owner_skips_in_progress_remote_assistant_history\b/,
        message: 'missing in-progress remote assistant history regression',
      },
    ],
  },
  {
    path: 'src/crates/services/services-integrations/src/remote_connect.rs',
    reason:
      'services-integrations must own remote-connect wire/response assembly and preserve remote owner compatibility re-exports',
    patterns: [
      {
        regex: /\bpub struct RemoteSessionStateTracker\b/,
        message: 'missing remote session state tracker owner',
      },
      {
        regex: /\bpub enum TrackerEvent\b/,
        message: 'missing remote tracker event owner',
      },
      {
        regex: /\bpub trait RemoteSessionTrackerHost\b/,
        message: 'missing remote tracker host port',
      },
      {
        regex: /\bpub struct RemoteSessionTrackerRegistry\b/,
        message: 'missing remote tracker registry owner',
      },
      {
        regex: /\bpub fn make_slim_tool_params\b/,
        message: 'missing remote tool preview slimming helper',
      },
      {
        regex: /\bfn handle_agentic_event\b/,
        message: 'missing tracker event reducer',
      },
      {
        regex: /\bpub fn resolve_remote_agent_type\b/,
        message: 'missing remote agent type helper',
      },
      {
        regex: /\bpub struct RemoteImageContext\b/,
        message: 'missing portable remote image context contract',
      },
      {
        regex: /\bpub trait RemoteImageContextAdapter\b/,
        message: 'missing remote image context adapter contract',
      },
      {
        regex: /\bpub fn build_remote_image_contexts\b/,
        message: 'missing legacy remote image context builder',
      },
      {
        regex: /\bpub fn resolve_remote_execution_image_contexts\b/,
        message: 'missing remote image context preference helper',
      },
      {
        regex: /\bpub fn remote_session_restore_target\b/,
        message: 'missing remote restore-target helper',
      },
      {
        regex: /\bpub enum RemoteCancelDecision\b/,
        message: 'missing remote cancel decision contract',
      },
      {
        regex: /\bpub fn resolve_remote_cancel_decision\b/,
        message: 'missing remote cancel decision resolver',
      },
      {
        regex: /\bpub struct RemoteCancelTaskRequest\b/,
        message: 'missing remote cancel task request contract',
      },
      {
        regex: /\bpub trait RemoteCancelRuntimeHost\b/,
        message: 'missing remote cancel runtime host port',
      },
      {
        regex: /\bpub async fn cancel_remote_task\b/,
        message: 'missing remote cancel orchestration owner',
      },
      {
        regex: /\bpub trait RemoteDialogRuntimeHost\b/,
        message: 'missing remote dialog runtime host port',
      },
      {
        regex: /\bpub async fn submit_remote_dialog\b/,
        message: 'missing remote dialog orchestration owner',
      },
      {
        regex: /\bpub struct RemoteChatHistoryTurn\b/,
        message: 'missing remote chat history turn DTO',
      },
      {
        regex: /\bpub struct RemoteChatHistoryRound\b/,
        message: 'missing remote chat history round DTO',
      },
      {
        regex: /\bpub struct RemoteChatHistoryToolItem\b/,
        message: 'missing remote chat history tool item DTO',
      },
      {
        regex: /\bpub fn build_remote_chat_messages\b/,
        message: 'missing remote chat history assembly owner',
      },
      {
        regex: /\bpub const REMOTE_FILE_MAX_READ_BYTES\b/,
        message: 'missing remote file max-read policy',
      },
      {
        regex: /\bpub const REMOTE_FILE_MAX_CHUNK_BYTES\b/,
        message: 'missing remote file chunk policy',
      },
      {
        regex: /\bpub fn resolve_remote_file_chunk_range\b/,
        message: 'missing remote file chunk range helper',
      },
      {
        regex: /\bpub fn remote_file_display_name\b/,
        message: 'missing remote file display-name fallback',
      },
      {
        regex: /\bpub fn resolve_remote_workspace_path\b/,
        message: 'missing remote workspace path resolver',
      },
      {
        regex: /\bpub fn detect_remote_mime_type\b/,
        message: 'missing remote MIME detector',
      },
      {
        regex: /\bpub async fn read_remote_workspace_file\b/,
        message: 'missing remote workspace full-file reader',
      },
      {
        regex: /\bpub async fn read_remote_workspace_file_chunk\b/,
        message: 'missing remote workspace chunk reader',
      },
      {
        regex: /\bpub async fn read_remote_workspace_file_info\b/,
        message: 'missing remote workspace file-info reader',
      },
      {
        regex: /\bRemoteWorkspaceFileRuntimeHost\b/,
        message: 'missing remote workspace file runtime host contract',
      },
      {
        regex: /\bRemoteWorkspaceRuntimeHost\b/,
        message: 'missing remote workspace runtime host contract',
      },
      {
        regex: /\bRemoteInitialSyncRuntimeHost\b/,
        message: 'missing remote initial-sync runtime host contract',
      },
      {
        regex: /\bRemoteSessionRuntimeHost\b/,
        message: 'missing remote session runtime host contract',
      },
      {
        regex: /\bRemotePollRuntimeHost\b/,
        message: 'missing remote poll runtime host contract',
      },
      {
        regex: /\bRemoteInteractionRuntimeHost\b/,
        message: 'missing remote interaction runtime host contract',
      },
      {
        regex: /\bpub async fn handle_remote_workspace_file_command\b/,
        message: 'missing remote workspace file command owner handler',
      },
      {
        regex: /\bpub async fn handle_remote_workspace_command\b/,
        message: 'missing remote workspace command owner handler',
      },
      {
        regex: /\bpub async fn generate_remote_initial_sync\b/,
        message: 'missing remote initial-sync owner handler',
      },
      {
        regex: /\bpub async fn handle_remote_session_command\b/,
        message: 'missing remote session command owner handler',
      },
      {
        regex: /\bpub async fn handle_remote_poll_command\b/,
        message: 'missing remote poll command owner handler',
      },
      {
        regex: /\bpub async fn handle_remote_interaction_command\b/,
        message: 'missing remote interaction command owner handler',
      },
      {
        regex: /\bpub trait RemoteCommandRuntimeHost\b/,
        message: 'missing remote command runtime host contract',
      },
      {
        regex: /\bpub async fn handle_remote_command\b/,
        message: 'missing remote command routing owner',
      },
      {
        regex: /\bpub fn remote_file_content_response\b/,
        message: 'missing remote file content response assembly helper',
      },
      {
        regex: /\bpub fn remote_file_chunk_response\b/,
        message: 'missing remote file chunk response assembly helper',
      },
      {
        regex: /\bpub fn remote_file_info_response\b/,
        message: 'missing remote file-info response assembly helper',
      },
      {
        regex: /\bpub fn remote_dialog_submit_response\b/,
        message: 'missing remote dialog response assembly helper',
      },
      {
        regex: /\bpub fn remote_task_cancel_response\b/,
        message: 'missing remote task cancel response assembly helper',
      },
      {
        regex: /\bpub fn remote_interaction_accepted_response\b/,
        message: 'missing remote interaction response assembly helper',
      },
      {
        regex: /\bpub fn remote_answer_question_response\b/,
        message: 'missing remote answer response assembly helper',
      },
      {
        regex: /\bRemoteWorkspaceFacts\b/,
        message: 'missing remote workspace response facts DTO',
      },
      {
        regex: /\bRemoteSessionMetadata\b/,
        message: 'missing remote session response metadata DTO',
      },
      {
        regex: /\bpub fn remote_workspace_info_response\b/,
        message: 'missing remote workspace-info response assembly helper',
      },
      {
        regex: /\bpub fn remote_recent_workspaces_response\b/,
        message: 'missing remote recent-workspaces response assembly helper',
      },
      {
        regex: /\bpub fn remote_assistant_list_response\b/,
        message: 'missing remote assistant-list response assembly helper',
      },
      {
        regex: /\bpub fn remote_session_info\b/,
        message: 'missing remote session response facts helper',
      },
      {
        regex: /\bpub fn remote_session_list_response\b/,
        message: 'missing remote session-list response assembly helper',
      },
      {
        regex: /\bpub fn remote_initial_sync_response\b/,
        message: 'missing remote initial-sync response assembly helper',
      },
      {
        regex: /\bpub fn remote_messages_response\b/,
        message: 'missing remote messages response assembly helper',
      },
      {
        regex: /\bpub struct RemoteDefaultModelsConfig\b/,
        message: 'missing remote model default DTO',
      },
      {
        regex: /\bpub struct RemoteModelConfig\b/,
        message: 'missing remote model DTO',
      },
      {
        regex: /\bpub struct RemoteModelCatalog\b/,
        message: 'missing remote model catalog DTO',
      },
      {
        regex: /\bpub enum RemoteModelCapabilityFact\b/,
        message: 'missing remote model capability owner fact',
      },
      {
        regex: /\bpub enum RemoteReasoningModeFact\b/,
        message: 'missing remote reasoning mode owner fact',
      },
      {
        regex: /\bpub struct RemoteModelFacts\b/,
        message: 'missing remote model owner facts',
      },
      {
        regex: /\bpub struct RemoteModelCatalogFacts\b/,
        message: 'missing remote model catalog owner facts',
      },
      {
        regex: /\bpub fn build_remote_model_catalog\b/,
        message: 'missing remote model catalog assembly owner',
      },
      {
        regex: /\bpub struct RemoteModelCatalogPollDelta\b/,
        message: 'missing remote model catalog poll delta',
      },
      {
        regex: /\bpub fn normalize_remote_session_model_id\b/,
        message: 'missing remote session model normalization policy',
      },
      {
        regex: /\bpub fn normalize_remote_model_selection\b/,
        message: 'missing remote model selection policy',
      },
      {
        regex: /\bpub fn remote_model_selection_needs_config\b/,
        message: 'missing remote model selection config-gate policy',
      },
      {
        regex: /\bpub enum RemoteCommand\b/,
        message: 'missing remote command wire contract',
      },
      {
        regex: /\bpub enum RemoteDialogSchedulerOutcomeFact\b/,
        message: 'missing remote dialog scheduler outcome fact',
      },
      {
        regex: /\bpub fn remote_dialog_submit_outcome_from_scheduler\b/,
        message: 'missing remote dialog submit outcome assembly owner',
      },
      {
        regex: /\bpub enum RemoteResponse\b/,
        message: 'missing remote response wire contract',
      },
      {
        regex: /\bpub fn should_send_remote_model_catalog\b/,
        message: 'missing remote model catalog poll policy',
      },
      {
        regex: /\bpub fn remote_model_catalog_poll_delta\b/,
        message: 'missing remote model catalog poll delta helper',
      },
      {
        regex: /\bpub fn remote_no_change_poll_response\b/,
        message: 'missing remote no-change poll response helper',
      },
      {
        regex: /\bpub fn remote_snapshot_poll_response\b/,
        message: 'missing remote snapshot poll response helper',
      },
      {
        regex: /\bpub fn remote_persisted_poll_response\b/,
        message: 'missing remote persisted poll response helper',
      },
      {
        regex: /\bremote_workspace_handler_preserves_response_shapes\b/,
        message: 'missing remote workspace command handler regression',
      },
      {
        regex: /\bremote_session_handler_preserves_list_and_create_policy\b/,
        message: 'missing remote session command handler regression',
      },
      {
        regex: /\bremote_session_handler_removes_tracker_after_delete_success\b/,
        message: 'missing remote session delete tracker cleanup regression',
      },
      {
        regex: /\bremote_poll_handler_preserves_missing_workspace_error\b/,
        message: 'missing remote poll missing-workspace regression',
      },
      {
        regex: /\bremote_interaction_handler_preserves_default_reject_reason\b/,
        message: 'missing remote interaction default reject regression',
      },
    ],
  },
  {
    path: 'src/crates/services/services-integrations/tests/remote_connect_contracts.rs',
    reason: 'remote-connect owner crate must keep focused behavior contracts',
    patterns: [
      {
        regex: /\bremote_connect_command_wire_shape_lives_in_owner_contract\b/,
        message: 'missing remote command wire contract test',
      },
      {
        regex: /\bremote_connect_response_wire_shape_lives_in_owner_contract\b/,
        message: 'missing remote response wire contract test',
      },
      {
        regex: /\bremote_connect_model_catalog_delta_preserves_poll_invalidation_policy\b/,
        message: 'missing remote model catalog delta contract test',
      },
      {
        regex: /\bremote_connect_model_catalog_builder_preserves_config_shape\b/,
        message: 'missing remote model catalog builder contract test',
      },
      {
        regex: /\bremote_connect_model_selection_policy_owns_alias_and_config_reference_rules\b/,
        message: 'missing remote model selection policy contract test',
      },
      {
        regex: /\bremote_connect_poll_helpers_preserve_delta_and_completion_policy\b/,
        message: 'missing remote poll helper contract test',
      },
      {
        regex: /\bremote_connect_image_context_policy_preserves_legacy_fallback_shape\b/,
        message: 'missing legacy image context fallback test',
      },
      {
        regex: /\bremote_connect_image_context_policy_prefers_explicit_contexts\b/,
        message: 'missing explicit image context preference test',
      },
      {
        regex: /\bremote_connect_image_context_adapter_owns_portable_conversion_shape\b/,
        message: 'missing image context adapter contract test',
      },
      {
        regex: /\bremote_connect_cancel_and_restore_policy_preserve_runtime_decisions\b/,
        message: 'missing cancel/restore policy test',
      },
      {
        regex: /\bremote_connect_dialog_runtime_owns_restore_prewarm_and_submit_order\b/,
        message: 'missing dialog runtime order test',
      },
      {
        regex: /\bremote_connect_dialog_runtime_preserves_explicit_turn_without_restore\b/,
        message: 'missing dialog explicit-turn test',
      },
      {
        regex: /\bremote_connect_dialog_submit_outcome_builder_preserves_scheduler_shape\b/,
        message: 'missing remote dialog outcome builder contract test',
      },
      {
        regex: /\bremote_connect_dialog_runtime_keeps_legacy_restore_failure_tolerance\b/,
        message: 'missing restore failure tolerance test',
      },
      {
        regex: /\bremote_chat_history_assembly_preserves_message_shape_and_item_order\b/,
        message: 'missing remote chat history assembly shape/order test',
      },
      {
        regex: /\bremote_chat_history_assembly_skips_in_progress_assistant_history\b/,
        message: 'missing remote chat history in-progress guard test',
      },
      {
        regex: /\bremote_connect_file_transfer_policy_preserves_limits_and_chunk_ranges\b/,
        message: 'missing remote file transfer policy test',
      },
      {
        regex: /\bremote_connect_file_transfer_policy_preserves_name_fallback\b/,
        message: 'missing remote file display-name test',
      },
      {
        regex: /\bremote_connect_file_path_resolution_stays_within_workspace_root\b/,
        message: 'missing remote file path resolution test',
      },
      {
        regex: /\bremote_connect_file_read_helpers_preserve_current_wire_inputs\b/,
        message: 'missing remote full-read helper test',
      },
      {
        regex: /\bremote_connect_file_chunk_and_info_helpers_preserve_response_facts\b/,
        message: 'missing remote chunk/info helper test',
      },
      {
        regex: /\bremote_connect_file_response_assembly_owns_base64_wire_shape\b/,
        message: 'missing remote file response assembly contract test',
      },
      {
        regex: /\bremote_connect_file_command_handler_owns_owner_flow_and_uses_host_root\b/,
        message: 'missing remote file command handler owner-flow test',
      },
      {
        regex: /\bremote_connect_execution_response_helpers_preserve_wire_shape\b/,
        message: 'missing remote execution response helper contract test',
      },
      {
        regex: /\bremote_connect_command_owner_routes_send_message_and_prefers_explicit_images\b/,
        message: 'missing remote command routing image/source regression',
      },
      {
        regex: /\bremote_connect_command_owner_preserves_cancel_and_group_routing\b/,
        message: 'missing remote command routing group/cancel regression',
      },
      {
        regex: /\bremote_connect_tracker_keeps_finished_turn_snapshot_until_persistence_finalizes\b/,
        message: 'missing tracker completion contract test',
      },
      {
        regex: /\bremote_connect_tracker_registry_owns_lifecycle_without_core_state\b/,
        message: 'missing tracker registry owner test',
      },
      {
        regex: /\bremote_connect_tracker_ignores_unrelated_direct_session_events\b/,
        message: 'missing tracker unrelated-event guard test',
      },
      {
        regex: /\bremote_connect_tool_preview_slimming_keeps_short_fields_and_drops_large_strings\b/,
        message: 'missing remote tool preview slimming test',
      },
      {
        regex: /\bremote_connect_cancel_runtime_restores_missing_session_before_cancel\b/,
        message: 'missing remote cancel restore/order regression',
      },
      {
        regex: /\bremote_connect_cancel_runtime_preserves_stale_and_idle_errors_without_restore\b/,
        message: 'missing remote cancel stale/idle regression',
      },
      {
        regex: /\bremote_connect_cancel_runtime_preserves_restore_failure_error\b/,
        message: 'missing remote cancel restore failure regression',
      },
      {
        regex: /\bremote_connect_workspace_response_helpers_own_wire_shape\b/,
        message: 'missing remote workspace response assembly regression',
      },
      {
        regex: /\bremote_connect_session_response_helpers_own_pagination_and_timestamps\b/,
        message: 'missing remote session response assembly regression',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/remote_connect/remote_server.rs',
    reason:
      'core remote-connect server must remain a product runtime adapter around integrations-owned contracts',
    patterns: [
      {
        regex: /\bCoreServiceAgentRuntime\b/,
        message: 'missing core service/agent runtime owner routing',
      },
      {
        regex: /\bsubmit_remote_dialog\b/,
        message: 'missing remote dialog owner orchestration delegation',
      },
      {
        regex: /\bcancel_remote_task\b/,
        message: 'missing remote cancel owner orchestration delegation',
      },
      {
        regex: /\bhandle_remote_workspace_file_command\b/,
        message: 'missing remote file command owner delegation',
      },
      {
        regex: /\bRemoteCommandRuntimeHost\b/,
        message: 'missing remote command runtime host adapter',
      },
      {
        regex: /\bhandle_remote_command\b/,
        message: 'missing remote command routing owner delegation',
      },
      {
        regex: /\bhandle_remote_interaction_command\b/,
        message: 'missing remote interaction command owner orchestration delegation',
      },
      {
        regex: /\bgenerate_remote_initial_sync\b/,
        message: 'missing remote initial-sync owner orchestration delegation',
      },
      {
        regex: /\bhandle_remote_workspace_command\b/,
        message: 'missing remote workspace command owner orchestration delegation',
      },
      {
        regex: /\bhandle_remote_session_command\b/,
        message: 'missing remote session command owner orchestration delegation',
      },
      {
        regex: /\bhandle_remote_poll_command\b/,
        message: 'missing remote poll command owner orchestration delegation',
      },
      {
        regex: /\bhandle_remote_interaction_command\b/,
        message: 'missing remote interaction command owner orchestration delegation',
      },
      {
        regex: /\bremote_image_context\b/,
        message: 'missing image context adapter contract delegation',
      },
      {
        regex: /\bcore_service_agent_runtime_owner_maps_remote_image_context\b/,
        message: 'missing core service/agent image-context owner regression',
      },
      {
        regex: /\bremote_execution_prefers_unified_image_contexts_over_legacy_images\b/,
        message: 'missing unified image context preference regression',
      },
      {
        regex: /\bremote_execution_falls_back_to_legacy_images_as_image_contexts\b/,
        message: 'missing legacy image context fallback regression',
      },
      {
        regex: /\bremote_cancel_decision_preserves_current_turn_boundaries\b/,
        message: 'missing remote cancel boundary regression',
      },
      {
        regex: /\bremote_restore_target_only_restores_cold_sessions_with_workspace_binding\b/,
        message: 'missing remote restore target regression',
      },
      {
        regex: /\bremote_command_snapshot_covers_execution_poll_and_cancel_surfaces\b/,
        message: 'missing remote command snapshot regression',
      },
      {
        regex: /\bremote_response_snapshot_preserves_active_turn_and_result_shapes\b/,
        message: 'missing remote response snapshot regression',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/remote_connect/bot/command_router.rs',
    reason:
      'remote-connect bot must route concrete agent runtime port bindings through the core service/agent runtime owner',
    patterns: [
      {
        regex: /\bCoreServiceAgentRuntime\b/,
        message: 'missing core service/agent runtime owner routing',
      },
      {
        regex: /\bagent_submission_port\b/,
        message: 'missing agent submission port owner binding',
      },
      {
        regex: /\bbuild_remote_session_create_request\b/,
        message: 'missing integrations-owned remote session create request builder',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/coordination/scheduler.rs',
    reason:
      'core scheduler keeps remote queue policy semantics until agent-runtime migration is reviewed',
    patterns: [
      {
        regex: /\bremote_queue_policy_preserves_interactive_preempt_and_confirmation_boundary\b/,
        message: 'missing remote queue policy regression',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/registry.rs',
    reason:
      'core registry must stay a compatibility container that delegates product tool runtime assembly through the core owner module',
    patterns: [
      {
        regex: /\bfrom_inner\b/,
        message: 'missing generic agent-tools registry adapter hook',
      },
      {
        regex: /\bProductToolDecoratorRef\b/,
        message: 'missing product decorator ref alias using agent-tools contract',
      },
      {
        regex: /\bProductToolRuntime\b/,
        message: 'missing product tool runtime owner delegation',
      },
      {
        regex: /\bget_collapsed_tool_names\b/,
        message: 'missing collapsed-tool catalog owner',
      },
      {
        regex: /\bresolve_product_readonly_enabled_tools\b/,
        message: 'missing product tool catalog readonly facade delegation',
      },
      {
        regex: /\bproduct_tool_runtime_owner_preserves_registry_contract\b/,
        message: 'missing collapsed-tool manifest migration baseline',
      },
      {
        regex: /\binner\.is_tool_collapsed\b/,
        message: 'missing collapsed exposure lookup delegation',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/product_runtime.rs',
    reason:
      'core product tool runtime owner delegates generic registry assembly and only wires product plan, decorator, and compatibility facade',
    patterns: [
      {
        regex: /\bProductToolRuntime\b/,
        message: 'missing core product tool runtime owner',
      },
      {
        regex: /\bSnapshotToolDecorator\b/,
        message: 'missing generic snapshot decorator injection',
      },
      {
        regex: /\bcreate_product_tool_registry_from_plan\b/,
        message: 'missing product registry assembly adapter delegation',
      },
      {
        regex: /\bproduct_assembly_plan_for_profile\b/,
        message: 'missing product assembly plan provider group plan delegation',
      },
      {
        regex: /\bproduct_tool_runtime_owner_preserves_registry_contract\b/,
        message: 'missing product runtime owner registry equivalence regression',
      },
      {
        regex: /\bproduct_tool_runtime_registry_preserves_provider_plan_order\b/,
        message: 'missing product tool provider plan-to-registry order regression',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/product_runtime/materialization.rs',
    reason:
      'product runtime materialization must keep only concrete tool construction and product plan adapter while delegating generic registry assembly to agent-tools',
    patterns: [
      {
        regex: /\bProductConcreteToolFactory\b/,
        message: 'missing product concrete tool factory adapter',
      },
      {
        regex: /\bimpl StaticToolProviderFactory<dyn Tool> for ProductConcreteToolFactory\b/,
        message: 'missing concrete tool factory implementation',
      },
      {
        regex: /\bProductToolProviderPlanAdapter\b/,
        message: 'missing product provider plan adapter',
      },
      {
        regex: /\bimpl StaticToolProviderPlan for ProductToolProviderPlanAdapter\b/,
        message: 'missing product provider plan adapter contract',
      },
      {
        regex: /\bcreate_registry_from_static_provider_plans\b/,
        message: 'missing generic agent-tools plan-to-registry delegation',
      },
      {
        regex: /\bcreate_product_tool_registry_from_plan\b/,
        message: 'missing product registry creation adapter',
      },
      {
        regex: /\bmaterialize_tool\b/,
        message: 'missing concrete tool materialization boundary',
      },
      {
        regex: /\bGetToolSpecTool::new\(\)/,
        message: 'missing GetToolSpec registration anchor',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/product_runtime/snapshot.rs',
    reason:
      'product runtime snapshot wrapper must stay isolated from registry and catalog ownership',
    patterns: [
      {
        regex: /\bProductSnapshotToolWrapper\b/,
        message: 'missing core product snapshot wrapper adapter',
      },
      {
        regex: /\bimpl SnapshotToolWrapper<dyn Tool> for ProductSnapshotToolWrapper\b/,
        message: 'missing generic snapshot wrapper implementation',
      },
      {
        regex: /\bwrap_tool_for_snapshot_tracking\b/,
        message: 'missing snapshot wrapper boundary',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/product_runtime/catalog.rs',
    reason:
      'product runtime catalog owner keeps manifest, snapshot, readonly, and GetToolSpec product facades explicit',
    patterns: [
      {
        regex: /\bProductToolCatalogProvider\b/,
        message: 'missing core product tool catalog provider owner',
      },
      {
        regex: /\bimpl ToolCatalogSnapshotProvider<dyn Tool> for ProductToolCatalogProvider\b/,
        message: 'missing core tool catalog snapshot provider implementation',
      },
      {
        regex: /\bimpl GetToolSpecCatalogProvider<dyn Tool, ToolUseContext> for ProductToolCatalogProvider\b/,
        message: 'missing core product GetToolSpec catalog provider implementation',
      },
      {
        regex: /\bget_global_tool_registry\b/,
        message: 'missing core product registry snapshot access',
      },
      {
        regex: /\bget_agent_registry\b/,
        message: 'missing core agent policy source for contextual catalog',
      },
      {
        regex: /\bToolCatalogRuntime\b/,
        message: 'missing agent-tools product catalog runtime facade delegation',
      },
      {
        regex: /\bproduct_tool_catalog_runtime\b/,
        message: 'missing product catalog runtime factory',
      },
      {
        regex: /\bGetToolSpecRuntime\b/,
        message: 'missing agent-tools GetToolSpec runtime facade delegation',
      },
      {
        regex: /\bproduct_get_tool_spec_runtime\b/,
        message: 'missing product GetToolSpec runtime factory',
      },
      {
        regex: /\bresolve_product_tool_manifest\b/,
        message: 'missing product manifest facade',
      },
      {
        regex: /\bresolve_product_resolved_tool_manifest\b/,
        message: 'missing product resolved manifest compatibility facade',
      },
      {
        regex: /\bresolve_product_resolved_visible_tools\b/,
        message: 'missing product resolved visible-tools compatibility facade',
      },
      {
        regex: /\bresolve_product_readonly_enabled_tools\b/,
        message: 'missing product readonly enabled tools facade',
      },
      {
        regex: /\bresolve_product_get_tool_spec_results\b/,
        message: 'missing product GetToolSpec Tool-result vector facade',
      },
      {
        regex: /\bunlocked_collapsed_tools\b/,
        message: 'missing product runtime collapsed-tool unlock state source',
      },
      {
        regex: /\bproduct_catalog_provider_default_get_tool_spec_catalog_matches_registry\b/,
        message: 'missing product catalog provider collapsed catalog regression',
      },
      {
        regex: /\bproduct_resolved_manifest_owner_matches_legacy_shape\b/,
        message: 'missing product resolved manifest compatibility regression',
      },
      {
        regex: /\bGetToolSpec requires agent type context\b/,
        message: 'missing contextual GetToolSpec catalog validation boundary',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/tool_adapter.rs',
    reason:
      'core must keep the product Tool-to-agent-tools adapters explicit until ToolUseContext and concrete tools migrate',
    patterns: [
      {
        regex: /\bimpl ToolRegistryItem for dyn Tool\b/,
        message: 'missing core Tool registry adapter',
      },
      {
        regex: /\bimpl ContextualToolManifestItem<ToolUseContext> for dyn Tool\b/,
        message: 'missing core ToolUseContext contextual manifest adapter',
      },
      {
        regex: /\bTool::dynamic_tool_info\b/,
        message: 'missing dynamic tool metadata adapter',
      },
      {
        regex: /\bTool::is_readonly\b/,
        message: 'missing readonly metadata adapter',
      },
      {
        regex: /\bTool::is_enabled\b/,
        message: 'missing enabled-state metadata adapter',
      },
      {
        regex: /\bTool::description_with_context\b/,
        message: 'missing context-aware tool description adapter',
      },
      {
        regex: /\bTool::input_schema_for_model_with_context\b/,
        message: 'missing context-aware tool schema adapter',
      },
    ],
  },
  {
    path: 'src/crates/execution/tool-contracts/src/framework.rs',
    reason: 'agent-tools owns portable tool facts plus generic registry and provider contracts',
    patterns: [
      {
        regex: /\bpub struct ToolContextFacts\b/,
        message: 'missing portable tool context facts contract',
      },
      {
        regex: /\bpub trait PortableToolContextProvider\b/,
        message: 'missing portable tool context provider contract',
      },
      {
        regex: /\bpub enum ToolWorkspaceKind\b/,
        message: 'missing portable workspace kind contract',
      },
      {
        regex: /\bpub trait StaticToolProvider\b/,
        message: 'missing static tool provider contract',
      },
      {
        regex: /\bpub struct ToolRuntimeAssembly\b/,
        message: 'missing generic runtime assembly contract',
      },
      {
        regex: /\bpub type ToolDecoratorRef\b/,
        message: 'missing generic decorator ref contract',
      },
      {
        regex: /\bpub trait SnapshotToolWrapper\b/,
        message: 'missing generic snapshot wrapper contract',
      },
      {
        regex: /\bpub struct SnapshotToolDecorator\b/,
        message: 'missing generic snapshot decorator contract',
      },
      {
        regex: /\bcreate_registry_from_static_providers\b/,
        message: 'missing generic static-provider assembly helper',
      },
      {
        regex: /\bcreate_registry_from_static_provider_plans\b/,
        message: 'missing generic static-provider plan-to-registry helper',
      },
      {
        regex: /\bpub fn install_static_provider\b/,
        message: 'missing static provider registry installer',
      },
      {
        regex: /\bpub fn build_get_tool_spec_duplicate_load_result\b/,
        message: 'missing provider-neutral GetToolSpec duplicate-load result helper',
      },
      {
        regex: /\bpub fn build_get_tool_spec_detail_result\b/,
        message: 'missing provider-neutral GetToolSpec detail result helper',
      },
      {
        regex: /\bpub fn resolve_get_tool_spec_execution_plan\b/,
        message: 'missing provider-neutral GetToolSpec execution plan helper',
      },
      {
        regex: /\bpub async fn resolve_get_tool_spec_execution_result_from_provider\b/,
        message: 'missing provider-backed GetToolSpec execution result helper',
      },
      {
        regex: /\bpub struct GetToolSpecRuntime\b/,
        message: 'missing provider-backed GetToolSpec runtime facade',
      },
    ],
  },
  {
    path: 'src/crates/execution/tool-provider-groups/src/lib.rs',
    reason:
      'tool-packs must keep its feature-group scaffold explicit without owning concrete tools yet',
    patterns: [
      {
        regex: /\bpub enum ToolPackFeatureGroup\b/,
        message: 'missing tool-pack feature group scaffold',
      },
      {
        regex: /\bpub fn all_feature_groups\b/,
        message: 'missing tool-pack full feature group metadata helper',
      },
      {
        regex: /\bpub fn enabled_feature_groups\b/,
        message: 'missing tool-pack compile-time feature metadata helper',
      },
      {
        regex: /\bpub struct ToolProviderGroupPlan\b/,
        message: 'missing tool-pack provider group plan contract',
      },
      {
        regex: /\bpub fn product_tool_provider_group_plan\b/,
        message: 'missing product tool provider group plan',
      },
      {
        regex: /\bpub enum ToolProviderGroupPlanSelectionError\b/,
        message: 'missing tool provider group plan selection error',
      },
      {
        regex: /\bpub fn try_product_tool_provider_group_plan_for_ids\b/,
        message: 'missing product tool provider group plan selector',
      },
      {
        regex: /\bproduct_provider_group_plan_selector_rejects_unknown_provider_ids\b/,
        message: 'missing provider group selector unknown-id regression',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/manifest_resolver.rs',
    reason:
      'core must continue owning manifest resolver wrappers while delegating product catalog access and generic manifest assembly',
    patterns: [
      {
        regex: /\bpub async fn resolve_tool_manifest\b/,
        message: 'missing tool manifest resolver owner',
      },
      {
        regex: /\bGET_TOOL_SPEC_TOOL_NAME\b/,
        message: 'missing GetToolSpec manifest insertion anchor',
      },
      {
        regex: /\bresolve_product_resolved_visible_tools\b/,
        message: 'missing core product visible-tools facade delegation',
      },
      {
        regex: /\bresolve_product_resolved_tool_manifest\b/,
        message: 'missing core product manifest facade delegation',
      },
      {
        regex: /\bcollapsed_tool_names\b/,
        message: 'missing collapsed-tool name tracking',
      },
      {
        regex: /\bmanifest_resolver_facade_preserves_product_owner_output\b/,
        message: 'missing manifest resolver facade parity regression',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/product_runtime/get_tool_spec_tool.rs',
    reason:
      'product runtime must own the GetToolSpec Tool adapter while delegating generic runtime surface to agent-tools',
    patterns: [
      {
        regex: /\bpub(?:\(crate\))? struct GetToolSpecTool\b/,
        message: 'missing GetToolSpec owner type',
      },
      {
        regex: /\bresolve_product_get_tool_spec_results\b/,
        message: 'missing product GetToolSpec Tool-result vector facade delegation',
      },
      {
        regex: /\bmap_get_tool_spec_execution_error\b/,
        message: 'missing core GetToolSpec execution error mapping boundary',
      },
      {
        regex: /\bbuild_collapsed_tools_context_section\b/,
        message: 'missing core collapsed-tool request-context section renderer',
      },
      {
        regex: /\bproduct_get_tool_spec_runtime\b/,
        message: 'missing product GetToolSpec runtime facade delegation',
      },
      {
        regex: /\bwith_runtime\b/,
        message: 'missing core GetToolSpec static surface facade boundary',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/framework.rs',
    reason:
      'core tool framework must keep compatibility re-exports while ToolUseContext is owned by tool_context_runtime',
    patterns: [
      {
        regex: /\bToolExposure\b/,
        message: 'missing ToolExposure compatibility re-export',
      },
      {
        regex: /\bpub use crate::agentic::tools::tool_context_runtime::ToolUseContext\b/,
        message: 'missing ToolUseContext compatibility re-export',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/tool_context_runtime.rs',
    reason:
      'core must keep ToolUseContext runtime/service bindings centralized while ToolUseContext and concrete tools remain core-owned',
    patterns: [
      {
        regex: /\bpub struct ToolUseContext\b/,
        message: 'missing ToolUseContext owner type',
      },
      {
        regex: /\bto_tool_context_facts\b/,
        message: 'missing portable ToolUseContext facts projection',
      },
      {
        regex: /\bimpl PortableToolContextProvider for ToolUseContext\b/,
        message: 'missing portable ToolUseContext facts provider impl',
      },
      {
        regex: /\btool_context_facts_omit_runtime_owner_fields_even_when_context_is_populated\b/,
        message: 'missing portable facts runtime-owner leak guard',
      },
      {
        regex: /customData/,
        message: 'missing custom data runtime-only facts guard',
      },
      {
        regex: /cancellationToken/,
        message: 'missing cancellation token runtime-only facts guard',
      },
      {
        regex: /\bunlocked_collapsed_tools\b/,
        message: 'missing collapsed-tool unlock state',
      },
      {
        regex: /\bimpl ToolUseContext\b/,
        message: 'missing ToolUseContext runtime binding owner impl',
      },
      {
        regex: /\brecord_light_checkpoint\b/,
        message: 'missing Deep Review checkpoint binding',
      },
      {
        regex: /\bbuild_runtime_light_checkpoint\b/,
        message: 'missing agent-runtime light checkpoint policy delegation',
      },
      {
        regex: /\bLightCheckpointWorkspaceFacts::LocalWorkspace\b/,
        message: 'missing local checkpoint facts delegation',
      },
      {
        regex: /\bcall_with_tool_runtime_hooks\b/,
        message: 'missing tool-call cancellation/post-call hook binding',
      },
      {
        regex: /\bcall_tool_with_runtime_hooks\b/,
        message: 'missing unified Tool::call runtime hook facade',
      },
      {
        regex: /\bcall_records_deep_review_read_file_measurement_without_touching_result\b/,
        message: 'missing Deep Review post-call hook regression in runtime owner',
      },
      {
        regex: /\bbuild_tool_use_context_for_task\b/,
        message: 'missing tool pipeline context materialization binding',
      },
      {
        regex: /\bbuild_tool_description_context\b/,
        message: 'missing tool manifest description context materialization binding',
      },
      {
        regex: /\bensure_current_workspace_runtime\b/,
        message: 'missing workspace runtime ensure binding',
      },
      {
        regex: /\bresolve_tool_path\b/,
        message: 'missing tool path resolution binding',
      },
      {
        regex: /\benforce_path_operation\b/,
        message: 'missing runtime path policy binding',
      },
      {
        regex: /\bis_tool_path_allowed_by_resolved_roots\b/,
        message: 'missing path policy owner delegation to agent-tools',
      },
      {
        regex: /\bbuild_tool_path_policy_denial_message\b/,
        message: 'missing shared path policy denial contract',
      },
      {
        regex: /\bresolve_tool_path_with_context\b/,
        message: 'missing shared tool path resolution owner delegation',
      },
      {
        regex: /\btool_path_is_effectively_absolute\b/,
        message: 'missing shared tool path absolute owner delegation',
      },
      {
        regex: /\bbuild_tool_runtime_artifact_reference\b/,
        message: 'missing runtime artifact reference owner delegation',
      },
      {
        regex: /\bbuild_tool_session_runtime_artifact_reference\b/,
        message: 'missing session runtime artifact reference owner delegation',
      },
      {
        regex: /\bworkspace_path_resolution_rejects_absolute_paths_outside_remote_workspace\b/,
        message: 'missing remote workspace containment regression',
      },
      {
        regex: /\bruntime_uri_resolution_rejects_different_workspace_scope\b/,
        message: 'missing runtime artifact scope regression',
      },
      {
        regex: /\bpath_policy_allows_only_configured_local_roots\b/,
        message: 'missing path policy enforcement regression',
      },
      {
        regex: /\btool_call_runtime_hook_returns_cancelled_before_impl_completes\b/,
        message: 'missing tool-call cancellation regression',
      },
      {
        regex: /\btool_task_context_materialization_preserves_runtime_fields\b/,
        message: 'missing tool task context materialization regression',
      },
      {
        regex: /\btool_description_context_preserves_manifest_custom_data_shape\b/,
        message: 'missing tool description context regression',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/pipeline/tool_pipeline.rs',
    reason:
      'core must continue carrying collapsed-tool unlock state while delegating provider-neutral execution gate policy to agent-tools',
    patterns: [
      {
        regex: /\bvalidate_tool_execution_admission\b/,
        message: 'missing provider-neutral tool execution admission gate delegation',
      },
      {
        regex: /\bunlocked_collapsed_tools\b/,
        message: 'missing collapsed-tool unlock state propagation',
      },
      {
        regex: /\bpipeline_preserves_core_owned_tool_context_without_portable_runtime_leak\b/,
        message: 'missing ToolUseContext runtime boundary regression',
      },
      {
        regex: /\bGetToolSpec\b/,
        message: 'missing GetToolSpec gating contract',
      },
      {
        regex: /\brender_tool_result_for_assistant\b/,
        message: 'missing tool result presentation owner delegation',
      },
      {
        regex: /\bbuild_tool_execution_error_presentation\b/,
        message: 'missing tool execution error presentation owner delegation',
      },
      {
        regex: /\bbuild_user_steering_interrupted_presentation\b/,
        message: 'missing steering-interrupted presentation owner delegation',
      },
      {
        regex: /\bbuild_invalid_tool_call_error_message\b/,
        message: 'missing invalid tool call presentation owner delegation',
      },
      {
        regex: /\bbuild_tool_call_truncation_recovery_notice\b/,
        message: 'missing truncation recovery notice owner delegation',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/execution/execution_engine.rs',
    reason:
      'core execution must pass collapsed-tool unlock state through product runtime owner and keep DeepResearch post-turn hooks',
    patterns: [
      {
        regex: /\bcollect_product_unlocked_collapsed_tools\b/,
        message: 'missing product runtime collapsed-tool unlock state handoff',
      },
      {
        regex: /\bunlocked_collapsed_tools\b/,
        message: 'missing collapsed-tool unlock propagation into round context',
      },
      {
        regex: /\bcollapsed_tool_names\b/,
        message: 'missing manifest collapsed-tool handoff',
      },
      {
        regex: /\bGetToolSpec\b/,
        message: 'missing GetToolSpec execution contract',
      },
      {
        regex: /\bcitation_renumber\b/,
        message: 'missing DeepResearch citation renumber hook',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/product_runtime/unlock_state.rs',
    reason:
      'product runtime owns collapsed-tool unlock observation adaptation while preserving generic agent-tools policy',
    patterns: [
      {
        regex: /\bcollect_product_unlocked_collapsed_tools\b/,
        message: 'missing product runtime collapsed-tool unlock collector',
      },
      {
        regex: /\bGetToolSpecLoadObservation\b/,
        message: 'missing GetToolSpec load observation adapter',
      },
      {
        regex: /\bcollect_loaded_collapsed_tool_names\b/,
        message: 'missing generic collapsed-tool load collector delegation',
      },
      {
        regex: /\bproduct_unlock_state_dedupes_and_filters_runtime_unlocks\b/,
        message: 'missing collapsed-tool unlock filtering regression',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/agents/registry/availability.rs',
    reason:
      'core agent registry must adapt config and AgentEntry facts while bitfun-agent-runtime owns mode-scoped subagent availability decisions',
    patterns: [
      {
        regex: /\bpub fn resolve_availability\b/,
        message: 'missing core compatibility availability adapter',
      },
      {
        regex: /\bpub fn resolve_override_layers\b/,
        message: 'missing project/user override layering adapter',
      },
      {
        regex: /\bresolve_subagent_availability\b/,
        message: 'missing agent-runtime availability decision delegation',
      },
      {
        regex: /\bto_runtime_override_state\b/,
        message: 'missing config override to runtime override adapter',
      },
      {
        regex: /\bAgentSubagentOverrideState\b/,
        message: 'missing config override state adapter source',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/agents/registry/types.rs',
    reason:
      'core agent registry must preserve legacy DTO fields while bitfun-agent-runtime owns query scope and availability reason contracts',
    patterns: [
      {
        regex: /pub use bitfun_agent_runtime::agents::\{[\s\S]*SubAgentSource[\s\S]*SubagentListScope[\s\S]*SubagentOverrideState[\s\S]*SubagentQueryContext[\s\S]*SubagentStateReason[\s\S]*\};/,
        message: 'missing agent-runtime subagent registry contract re-export',
      },
      {
        regex: /\bpub struct AgentInfo\b/,
        message: 'missing core AgentInfo facade DTO',
      },
      {
        regex: /\bdefault_enabled\b/,
        message: 'missing default availability field',
      },
      {
        regex: /\beffective_enabled\b/,
        message: 'missing effective availability field',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/agents/definitions/modes/mod.rs',
    reason:
      'core agent mode definitions must continue exposing Multitask mode until an approved agent-runtime migration preserves mode registration semantics',
    patterns: [
      {
        regex: /\bmod multitask\b/,
        message: 'missing Multitask mode module',
      },
      {
        regex: /\bpub use multitask::MultitaskMode\b/,
        message: 'missing Multitask mode export',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/agents/definitions/subagents/mod.rs',
    reason:
      'core subagent definitions must continue exposing the built-in GeneralPurpose subagent until registry ownership migration has equivalence coverage',
    patterns: [
      {
        regex: /\bmod general_purpose\b/,
        message: 'missing GeneralPurpose subagent module',
      },
      {
        regex: /\bpub use general_purpose::GeneralPurposeAgent\b/,
        message: 'missing GeneralPurpose subagent export',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/agents/registry/builtin.rs',
    reason:
      'core builtin registry must delegate builtin default model facts to agent-runtime while preserving latest-main compatibility',
    patterns: [
      {
        regex: /\bbuiltin_agent_specs\(\)/,
        message: 'missing builtin agent spec registration source',
      },
      {
        regex: /\bruntime_agents::default_model_id_for_builtin_agent\(agent_type\)/,
        message: 'missing default model delegation to agent-runtime',
      },
    ],
  },
  {
    path: 'src/crates/execution/agent-runtime/src/agents.rs',
    reason:
      'agent-runtime builtin agent catalog must own latest-main mode/subagent categories, visibility, and default model facts',
    patterns: [
      {
        regex: /\bbuiltin_agent_definition_specs\(\)/,
        message: 'missing builtin agent definition catalog owner',
      },
      {
        regex: /builtin_agent_spec\(\s*"Multitask",\s*Mode,\s*"auto"/,
        message: 'missing Multitask runtime default model mapping',
      },
      {
        regex: /builtin_agent_spec\(\s*"GeneralPurpose",\s*SubAgent,\s*"fast"/,
        message: 'missing GeneralPurpose runtime default model mapping',
      },
      {
        regex: /SubagentVisibilityPolicy::restricted\(\["Claw",\s*"Team"\]\)/,
        message: 'missing ComputerUse restricted visibility mapping',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/implementations/task_tool.rs',
    reason:
      'core Task tool must continue owning fork-aware background subagent launch semantics until a reviewed agent-runtime port preserves delivery behavior',
    patterns: [
      {
        regex: /\bfork_context\b/,
        message: 'missing Task fork_context schema and validation surface',
      },
      {
        regex: /\bSubagentContextMode::Fork\b/,
        message: 'missing forked subagent context mode path',
      },
      {
        regex: /delegation_policy\(\)\.spawn_child\(\)/,
        message: 'missing child delegation policy propagation',
      },
      {
        regex: /"run_in_background"/,
        message: 'missing Task run_in_background schema flag',
      },
      {
        regex: /\bstart_background_subagent\b/,
        message: 'missing background subagent launch path',
      },
      {
        regex: /\bbackground_task_id\b/,
        message: 'missing background task id result contract',
      },
      {
        regex: /Background \{\} started successfully/,
        message: 'missing assistant-visible background start acknowledgement',
      },
      {
        regex: /<background_task status=\\"started\\"/,
        message: 'missing structured background task start acknowledgement',
      },
      {
        regex: /\bbackground_subagent_start_acknowledgement_keeps_structured_task_marker\b/,
        message: 'missing background task start acknowledgement regression',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/coordination/scheduler.rs',
    reason:
      'core scheduler keeps concrete background delivery entry points while bitfun-agent-runtime owns running-turn injection construction',
    patterns: [
      {
        regex: /\bdeliver_background_result\b/,
        message: 'missing background subagent delivery entry point',
      },
      {
        regex: /\bresolve_background_delivery_injection\b/,
        message: 'missing runtime-owned background injection construction',
      },
      {
        regex: /DialogTriggerSource::AgentSession/,
        message: 'missing idle-session agent-session follow-up turn source',
      },
    ],
  },
  {
    path: 'src/apps/cli/src/ui/startup.rs',
    reason:
      'CLI mode-aware subagent management remains an app-layer product surface until agent registry migration has CLI equivalence coverage',
    patterns: [
      {
        regex: /\bfn show_available_subagent_list\b/,
        message: 'missing CLI subagent list surface',
      },
      {
        regex: /\bfn show_subagent_config_selector\b/,
        message: 'missing CLI subagent config surface',
      },
      {
        regex: /\bget_subagents_for_query\b/,
        message: 'missing CLI mode-scoped subagent query',
      },
      {
        regex: /\bSubagentQueryContext\b/,
        message: 'missing CLI subagent query context',
      },
      {
        regex: /\bupdate_subagent_override\b/,
        message: 'missing CLI subagent availability update path',
      },
    ],
  },
  {
    path: 'src/apps/cli/src/ui/subagent_selector.rs',
    reason:
      'CLI subagent selector presentation must remain app-layer UI while registry availability semantics stay in core',
    patterns: [
      {
        regex: /\bpub enum SubagentSelectorAction\b/,
        message: 'missing CLI subagent selector action contract',
      },
      {
        regex: /\bpub fn show_list\b/,
        message: 'missing CLI subagent list mode',
      },
      {
        regex: /\bpub fn show_config\b/,
        message: 'missing CLI subagent config mode',
      },
      {
        regex: /\bdefault_enabled\b/,
        message: 'missing CLI default availability display',
      },
      {
        regex: /\bfn render_subagent_line\b/,
        message: 'missing CLI subagent presentation renderer',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/agents/citation_renumber.rs',
    reason:
      'core DeepResearch runtime must continue owning citation renumber post-processing until agent-runtime migration is reviewed',
    patterns: [
      {
        regex: /\bpub async fn run_for_session_workspace\b/,
        message: 'missing DeepResearch citation hook entry point',
      },
      {
        regex: /\bpub async fn try_renumber_research_report\b/,
        message: 'missing deterministic citation renumber implementation',
      },
      {
        regex: /display_map\.json/,
        message: 'missing citation display map sidecar contract',
      },
      {
        regex: /\bREJECTED\b/,
        message: 'missing rejected-citation filtering contract',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/workspace/service.rs',
    reason:
      'core workspace runtime must continue owning startup remote-workspace guards until workspace service migration is reviewed',
    patterns: [
      {
        regex: /\bprepare_startup_restored_workspaces\b/,
        message: 'missing restored-workspace startup guard',
      },
      {
        regex: /\bWorkspaceKind::Remote\b/,
        message: 'missing remote workspace branch',
      },
      {
        regex: /\bensure_remote_workspace_runtime\b/,
        message: 'missing remote workspace runtime ensure call',
      },
      {
        regex: /\bsshHost\b/,
        message: 'missing remote workspace host metadata contract',
      },
    ],
  },
  {
    path: 'src/crates/services/services-integrations/src/workspace_search/mod.rs',
    reason:
      'workspace_search must keep flashgrep protocol internals crate-private and expose stable DTO/service APIs only',
    patterns: [
      {
        regex: /\bpub\(crate\)\s+mod\s+flashgrep\b/,
        message: 'flashgrep protocol internals must stay crate-private',
      },
    ],
  },
  {
    path: 'src/crates/services/services-integrations/src/workspace_search/service.rs',
    reason:
      'services-integrations workspace_search must own local flashgrep fallback and session lifecycle',
    patterns: [
      {
        regex: /\bpub struct WorkspaceSearchRepoConfig\b/,
        message: 'missing stable workspace-search repo config contract',
      },
      {
        regex: /\bwith_scan_fallback\b/,
        message: 'missing flashgrep scan fallback request flag',
      },
    ],
  },
  {
    path: 'src/crates/services/services-integrations/src/workspace_search/result_mapping.rs',
    reason:
      'services-integrations workspace_search result mapping must own shared flashgrep preview/result conversion',
    patterns: [
      {
        regex: /\bconvert_hits_to_file_search_results\b/,
        message: 'missing hit-to-file-result conversion owner',
      },
      {
        regex: /\bsplit_preview\b/,
        message: 'missing preview split contract',
      },
      {
        regex: /\bpreview_inside\b/,
        message: 'missing preview-inside rendering contract',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/search/service.rs',
    reason:
      'core workspace search service must remain a compatibility facade that injects product config/bootstrap hooks into the integration owner',
    patterns: [
      {
        regex: /\bowner::WorkspaceSearchService::new_with_hooks\b/,
        message: 'missing workspace-search owner delegation',
      },
      {
        regex: /\bCoreWorkspaceSearchRuntimeHooks\b/,
        message: 'missing core runtime hook adapter',
      },
      {
        regex: /\bWorkspaceSearchRepoConfig\b/,
        message: 'missing stable workspace-search repo config hook',
      },
      {
        regex: /\bget_global_config_service\b/,
        message: 'missing product config hook for workspace-search repo config',
      },
      {
        regex: /\bensure_workspace_gitignore_ignores_bitfun\b/,
        message: 'missing workspace bootstrap hook for search warmup',
      },
    ],
  },
  {
    path: 'src/crates/services/services-integrations/src/remote_ssh/workspace_search/mod.rs',
    reason:
      'remote SSH workspace_search strategy helpers must stay crate-internal and keep behavior-equivalence tests near the owner',
    patterns: [
      {
        regex: /\bpub\(crate\)\s+fn\s+build_remote_scope\b/,
        message: 'remote scope helper must stay crate-internal',
      },
      {
        regex: /\bpub\(crate\)\s+fn\s+shell_escape\b/,
        message: 'remote shell escaping helper must stay crate-internal',
      },
      {
        regex: /\bpub\(crate\)\s+fn\s+should_retry_remote_scan_fallback_as_files_with_matches\b/,
        message: 'remote scan fallback retry policy must stay crate-internal',
      },
      {
        regex: /\bremote_workspace_search_paths_preserve_current_contract\b/,
        message: 'missing remote path contract regression',
      },
      {
        regex: /\bremote_scan_fallback_retry_policy_preserves_current_contract\b/,
        message: 'missing remote scan fallback retry regression',
      },
    ],
  },
  {
    path: 'src/crates/services/services-integrations/src/remote_ssh/workspace_search/service.rs',
    reason:
      'services-integrations remote SSH workspace_search must own remote flashgrep concrete session, fallback, and binary lifecycle behind provider traits',
    patterns: [
      {
        regex: /\bpub trait RemoteWorkspaceSearchProvider\b/,
        message: 'missing remote search provider boundary',
      },
      {
        regex: /\bpub struct RemoteWorkspaceSearchService\b/,
        message: 'missing remote workspace search service owner',
      },
      {
        regex: /\bpub struct RemoteWorkspaceSearchStdioProtocol\b/,
        message: 'missing narrow remote stdio protocol facade',
      },
      {
        regex: /\bREMOTE_STDIO_SESSIONS\b/,
        message: 'missing remote stdio session lifecycle owner',
      },
      {
        regex: /\bensure_remote_search_context\b/,
        message: 'missing remote search context lifecycle owner',
      },
      {
        regex: /\ballow_scan_fallback:\s*true\b/,
        message: 'missing remote scan fallback contract',
      },
      {
        regex: /\bfallback_query\b/,
        message: 'missing FilesWithMatches fallback query',
      },
      {
        regex: /\bremote_search_rejects_non_linux_before_stdio_open\b/,
        message: 'missing remote OS gate regression',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/search/remote.rs',
    reason:
      'core remote search runtime must remain a compatibility facade over services-integrations while retaining concrete SSH/russh bridge adapters',
    patterns: [
      {
        regex: /\bServiceRemoteWorkspaceSearchService\b/,
        message: 'missing services remote search owner delegation',
      },
      {
        regex: /\bimpl RemoteWorkspaceSearchProvider for CoreRemoteWorkspaceSearchProvider\b/,
        message: 'missing core remote search provider adapter',
      },
      {
        regex: /\blookup_remote_connection_with_hint\b/,
        message: 'missing preferred remote connection lookup adapter',
      },
      {
        regex: /\bopen_exec_channel\b/,
        message: 'missing SSH stdio bridge adapter',
      },
      {
        regex: /\bRemoteWorkspaceSearchStdioProtocol\b/,
        message: 'missing narrow stdio protocol facade in core remote bridge',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/search/mod.rs',
    reason:
      'remote workspace search must route to the real implementation only when ssh-remote is enabled',
    patterns: [
      {
        regex: /#\[cfg\(feature = "ssh-remote"\)\]\s*mod remote\b/s,
        message: 'missing ssh-remote gate for real remote search implementation',
      },
      {
        regex: /#\[cfg\(not\(feature = "ssh-remote"\)\)\]\s*mod remote_disabled\b/s,
        message: 'missing disabled remote search implementation for no-default builds',
      },
      {
        regex: /#\[cfg\(not\(feature = "ssh-remote"\)\)\]\s*pub use remote_disabled/s,
        message: 'missing disabled remote search export',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/search/remote_disabled.rs',
    reason:
      'no-default core builds must keep remote search unavailable with an explicit diagnostic',
    patterns: [
      {
        regex: /Remote SSH search is disabled; enable the `ssh-remote` feature/,
        message: 'missing explicit disabled remote search diagnostic',
      },
      {
        regex: /\bpub struct RemoteWorkspaceSearchService\b/,
        message: 'missing disabled remote workspace search service surface',
      },
      {
        regex: /\bremote_workspace_search_service_for_path\b/,
        message: 'missing disabled remote workspace search resolver',
      },
    ],
  },
  {
    path: 'src/crates/interfaces/acp/src/client/manager.rs',
    reason:
      'ACP surface runtime must continue owning startup timeout diagnostics until ACP migration is reviewed',
    patterns: [
      {
        regex: /\bCLIENT_STARTUP_TIMEOUT_SECS\b/,
        message: 'missing ACP startup timeout duration contract',
      },
      {
        regex: /\bstartup_timeout_error_message\b/,
        message: 'missing ACP startup timeout diagnostic formatter',
      },
      {
        regex: /\bformats_startup_timeout_error_message\b/,
        message: 'missing ACP startup timeout regression',
      },
    ],
  },
  {
    path: 'src/web-ui/src/flow_chat/tool-cards/FileOperationToolCard.tsx',
    reason:
      'web-ui file operation surface must continue owning snapshot-to-local diff fallback until product surface migration is reviewed',
    patterns: [
      {
        regex: /\bopenLocalDiff\b/,
        message: 'missing local tool diff fallback',
      },
      {
        regex: /snapshotAPI\.getOperationDiff/,
        message: 'missing snapshot operation diff path',
      },
      {
        regex: /Snapshot diff unavailable/,
        message: 'missing snapshot-unavailable fallback diagnostic',
      },
      {
        regex: /\blocalDiffContent\b/,
        message: 'missing local diff content fallback state',
      },
    ],
  },
  {
    path: 'src/web-ui/src/main.tsx',
    reason:
      'web startup scheduling and trace orchestration remain web product-surface behavior, not core contract runtime',
    patterns: [
      {
        regex: /\bstartupTrace\b/,
        message: 'missing web startup trace surface',
      },
      {
        regex: /\bbackgroundTaskScheduler\b/,
        message: 'missing deferred startup scheduler surface',
      },
      {
        regex: /\binitializeAllTools\b/,
        message: 'missing narrow tool-startup entry integration',
      },
      {
        regex: /\bafter_render_start\b/,
        message: 'missing post-render startup phase',
      },
    ],
  },
  {
    path: 'src/web-ui/src/shared/utils/startupTrace.ts',
    reason:
      'web startup trace classification and redaction remain web infrastructure behavior until a telemetry contract is reviewed',
    patterns: [
      {
        regex: /\bfunction sanitizeTraceData\b/,
        message: 'missing startup trace sanitization',
      },
      {
        regex: /\bexport function isRemoteTraceRequest\b/,
        message: 'missing remote request classifier',
      },
      {
        regex: /\brecordApiCall\b/,
        message: 'missing startup API-call trace recorder',
      },
      {
        regex: /\bflushSummary\b/,
        message: 'missing bounded startup summary flush',
      },
      {
        regex: /\bmarkPhaseAfterAnimationFrames\b/,
        message: 'missing frame-delayed startup marker',
      },
    ],
  },
  {
    path: 'src/web-ui/src/shared/utils/backgroundTaskScheduler.ts',
    reason:
      'web background startup scheduling remains web infrastructure behavior and must preserve dedupe/cancel semantics',
    patterns: [
      {
        regex: /\bexport class BackgroundTaskScheduler\b/,
        message: 'missing background task scheduler',
      },
      {
        regex: /\binFlightKey\b/,
        message: 'missing in-flight dedupe key',
      },
      {
        regex: /\bAbortController\b/,
        message: 'missing cancellation controller',
      },
      {
        regex: /\bBackgroundTaskCancelledError\b/,
        message: 'missing cancellation error contract',
      },
      {
        regex: /\bcancelIdle\b/,
        message: 'missing idle callback cancellation',
      },
    ],
  },
  {
    path: 'src/web-ui/src/tools/initializeTools.ts',
    reason:
      'web tool startup must stay behind a narrow app-layer entry instead of importing product tools through shared contracts',
    patterns: [
      {
        regex: /\bexport async function initializeAllTools\b/,
        message: 'missing narrow tool startup entry',
      },
      {
        regex: /\binitializeLsp\b/,
        message: 'missing LSP startup initializer call',
      },
      {
        regex: /\binitializeGit\b/,
        message: 'missing Git startup initializer call',
      },
      {
        regex: /does not import every tool/,
        message: 'missing narrow startup import guard',
      },
    ],
  },
  {
    path: 'src/web-ui/src/tools/editor/services/MonacoStartupWarmup.ts',
    reason:
      'Monaco startup warmup remains a deferred web-app optimization, not a core runtime dependency',
    patterns: [
      {
        regex: /\bexport function scheduleMonacoStartupWarmup\b/,
        message: 'missing deferred Monaco warmup entry',
      },
      {
        regex: /\bbackgroundTaskScheduler\b/,
        message: 'missing background scheduler integration',
      },
      {
        regex: /startup:monaco-warmup/,
        message: 'missing Monaco warmup dedupe key',
      },
    ],
  },
  {
    path: 'src/web-ui/src/flow_chat/services/flow-chat-manager/SessionModule.ts',
    reason:
      'flow-chat history hydration remains web startup/product-surface behavior until a UI equivalence plan exists',
    patterns: [
      {
        regex: /\bhistorical_session_hydrate_request\b/,
        message: 'missing historical session hydrate trace',
      },
      {
        regex: /Load history in the background/,
        message: 'missing non-blocking history load contract',
      },
      {
        regex: /\bhistoryState: 'ready'/,
        message: 'missing history-ready state contract',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/miniapp/storage.rs',
    reason:
      'core must continue owning MiniApp storage runtime adapter until storage IO migration is reviewed',
    patterns: [
      {
        regex: /\bimpl MiniAppStoragePort for MiniAppStorage\b/,
        message: 'missing MiniApp storage port adapter owner',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/miniapp/builtin/mod.rs',
    reason:
      'core must continue owning built-in MiniApp seeding IO, marker writes, and recompilation while product-domains owns bundle assets',
    patterns: [
      {
        regex: /\bBUILTIN_APPS\b/,
        message: 'missing product-domain built-in MiniApp bundle re-export/use',
      },
      {
        regex: /\bbuiltin_content_hash\b/,
        message: 'missing product-domain built-in MiniApp content hash use',
      },
      {
        regex: /\bshould_seed_builtin_app\b/,
        message: 'missing product-domain built-in MiniApp seed decision use',
      },
      {
        regex: /\bresolve_builtin_seed_check\b/,
        message: 'missing product-domain built-in MiniApp seed check use',
      },
      {
        regex: /\bresolve_builtin_seed_action\b/,
        message: 'missing product-domain built-in MiniApp seed action use',
      },
      {
        regex: /\bbuiltin_source_files\b/,
        message: 'missing product-domain built-in MiniApp source payload use',
      },
      {
        regex: /\bbuild_builtin_seed_meta\b/,
        message: 'missing product-domain built-in MiniApp seed meta helper use',
      },
      {
        regex: /\bpreserved_builtin_created_at\b/,
        message: 'missing product-domain built-in MiniApp timestamp preservation helper use',
      },
      {
        regex: /\bBUILTIN_PLACEHOLDER_COMPILED_HTML\b/,
        message: 'missing product-domain built-in MiniApp placeholder payload use',
      },
      {
        regex: /\bread_builtin_install_marker\b/,
        message: 'missing core-owned built-in MiniApp marker read IO',
      },
      {
        regex: /\bparse_builtin_install_marker\b/,
        message: 'missing product-domain built-in MiniApp marker parse helper use',
      },
      {
        regex: /\bwrite_builtin_install_marker\b/,
        message: 'missing core-owned built-in MiniApp marker write IO',
      },
      {
        regex: /\bserialize_builtin_install_marker\b/,
        message: 'missing product-domain built-in MiniApp marker serialization helper use',
      },
      {
        regex: /\brecompile\b/,
        message: 'missing core-owned built-in MiniApp recompile orchestration',
      },
      {
        regex: /\bload_customization_metadata\b/,
        message: 'missing customized built-in preservation path',
      },
      {
        regex: /\bavailable_builtin_update\b/,
        message: 'missing customized built-in update metadata path',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/miniapp/host_dispatch.rs',
    reason:
      'core must continue owning MiniApp host-dispatch execution until host/runtime migration is reviewed',
    patterns: [
      {
        regex: /\bpub async fn dispatch_host\b/,
        message: 'missing MiniApp host dispatch entry',
      },
      {
        regex: /\bsplit_host_method\b/,
        message: 'missing product-domain MiniApp host method split use',
      },
      {
        regex: /\basync fn dispatch_fs\b/,
        message: 'missing MiniApp fs host dispatch',
      },
      {
        regex: /\bplan_fs_legacy_path_check\b/,
        message: 'missing product-domain MiniApp legacy fs path-gate plan use',
      },
      {
        regex: /\bplan_fs_host_call\b/,
        message: 'missing product-domain MiniApp fs host-call plan use',
      },
      {
        regex: /\bfs_policy_scopes\b/,
        message: 'missing product-domain MiniApp fs scope extraction policy use',
      },
      {
        regex: /\bfs_resolved_path_allowed\b/,
        message: 'missing product-domain MiniApp fs resolved path policy use',
      },
      {
        regex: /\basync fn dispatch_shell\b/,
        message: 'missing MiniApp shell host dispatch',
      },
      {
        regex: /\bplan_shell_host_call\b/,
        message: 'missing product-domain MiniApp shell host-call plan use',
      },
      {
        regex: /\bshell_exec_default_env\b/,
        message: 'missing product-domain MiniApp shell env policy use',
      },
      {
        regex: /\bcommand_basename_allowed\b/,
        message: 'missing MiniApp shell allowlist policy use',
      },
      {
        regex: /\bhost_allowed_by_allowlist\b/,
        message: 'missing MiniApp net allowlist policy use',
      },
    ],
  },
  {
    path: 'src/crates/services/services-integrations/src/remote_ssh/paths.rs',
    reason:
      'services-integrations remote-ssh owns workspace path/session identity helpers that do not require concrete SSH runtime handles',
    patterns: [
      {
        regex: /\bpub fn remote_workspace_runtime_root\b/,
        message: 'missing remote workspace runtime root helper',
      },
      {
        regex: /\bpub fn remote_workspace_session_mirror_dir\b/,
        message: 'missing remote workspace session mirror helper',
      },
      {
        regex: /\bpub fn canonicalize_local_workspace_root\b/,
        message: 'missing local workspace canonicalization helper',
      },
      {
        regex: /\bpub fn normalize_local_workspace_root_for_stable_id\b/,
        message: 'missing local workspace stable-root helper',
      },
      {
        regex: /\bpub fn local_workspace_roots_equal\b/,
        message: 'missing local workspace equality helper',
      },
      {
        regex: /\bpub fn unresolved_remote_session_storage_dir\b/,
        message: 'missing unresolved remote session storage helper',
      },
    ],
  },
  {
    path: 'src/crates/contracts/product-domains/src/miniapp/storage.rs',
    reason:
      'product-domains owns MiniApp storage shape contracts while core/adapters keep filesystem IO',
    patterns: [
      {
        regex: /\bpub struct MiniAppStorageLayout\b/,
        message: 'missing MiniApp storage layout contract',
      },
      {
        regex: /\bpub const META_JSON\b/,
        message: 'missing MiniApp metadata filename contract',
      },
      {
        regex: /\bpub fn source_file_path\b/,
        message: 'missing MiniApp source file layout helper',
      },
      {
        regex: /\bpub fn versions_dir\b/,
        message: 'missing MiniApp versions directory layout helper',
      },
      {
        regex: /\bpub const DRAFT_JSON\b/,
        message: 'missing MiniApp draft manifest filename contract',
      },
      {
        regex: /\bpub const REQUIRED_SOURCE_FILES\b/,
        message: 'missing MiniApp import source file list contract',
      },
      {
        regex: /\bpub const PLACEHOLDER_COMPILED_HTML\b/,
        message: 'missing MiniApp placeholder compiled HTML contract',
      },
      {
        regex: /\bpub struct MiniAppImportLayout\b/,
        message: 'missing MiniApp import layout contract',
      },
      {
        regex: /\bpub fn build_import_fallbacks\b/,
        message: 'missing MiniApp import fallback payload helper',
      },
      {
        regex: /\bpub fn draft_dir\b/,
        message: 'missing MiniApp draft directory layout helper',
      },
      {
        regex: /\bpub fn customization_path\b/,
        message: 'missing MiniApp customization metadata path helper',
      },
    ],
  },
  {
    path: 'src/crates/contracts/product-domains/src/miniapp/lifecycle.rs',
    reason:
      'product-domains owns pure MiniApp lifecycle state transitions while core keeps compile, storage IO, and runtime execution',
    patterns: [
      {
        regex: /\bpub fn mark_deps_installed_state\b/,
        message: 'missing MiniApp deps-installed state helper',
      },
      {
        regex: /\bpub struct MiniAppCreateInput\b/,
        message: 'missing MiniApp create input contract',
      },
      {
        regex: /\bpub struct MiniAppUpdatePatch\b/,
        message: 'missing MiniApp update patch contract',
      },
      {
        regex: /\bpub fn build_created_app\b/,
        message: 'missing MiniApp create state helper',
      },
      {
        regex: /\bpub fn apply_update_patch\b/,
        message: 'missing MiniApp update state helper',
      },
      {
        regex: /\bpub fn prepare_draft_app\b/,
        message: 'missing MiniApp draft prepare state helper',
      },
      {
        regex: /\bpub fn apply_draft_source_sync_result\b/,
        message: 'missing MiniApp draft source-sync state helper',
      },
      {
        regex: /\bpub fn apply_draft_permission_update_result\b/,
        message: 'missing MiniApp draft permission-update state helper',
      },
      {
        regex: /\bpub fn apply_draft_to_active\b/,
        message: 'missing MiniApp draft apply state helper',
      },
      {
        regex: /\bpub fn clear_worker_restart_required_state\b/,
        message: 'missing MiniApp worker-restart clear state helper',
      },
      {
        regex: /\bpub fn prepare_rollback_app\b/,
        message: 'missing MiniApp rollback state helper',
      },
      {
        regex: /\bpub fn apply_recompile_result\b/,
        message: 'missing MiniApp recompile result state helper',
      },
      {
        regex: /\bpub fn apply_sync_from_fs_result\b/,
        message: 'missing MiniApp sync-from-fs state helper',
      },
      {
        regex: /\bpub fn apply_import_runtime_state\b/,
        message: 'missing MiniApp import runtime state helper',
      },
      {
        regex: /\bpub fn prepare_imported_meta\b/,
        message: 'missing MiniApp imported metadata identity/timestamp helper',
      },
    ],
  },
  {
    path: 'src/crates/contracts/product-domains/src/miniapp/draft.rs',
    reason:
      'product-domains owns MiniApp draft DTO and response shape while core keeps draft filesystem IO',
    patterns: [
      {
        regex: /\bpub struct MiniAppDraftManifest\b/,
        message: 'missing MiniApp draft manifest DTO',
      },
      {
        regex: /\bpub struct MiniAppDraft\b/,
        message: 'missing MiniApp draft response DTO',
      },
      {
        regex: /\bpub fn build_draft_manifest\b/,
        message: 'missing MiniApp draft manifest helper',
      },
      {
        regex: /\bpub fn build_draft_response\b/,
        message: 'missing MiniApp draft response helper',
      },
    ],
  },
  {
    path: 'src/crates/contracts/product-domains/src/miniapp/runtime.rs',
    reason:
      'product-domains owns MiniApp runtime detection, including the reviewed concrete PATH/fs/version probe',
    patterns: [
      {
        regex: /\bpub fn detect_runtime\b/,
        message: 'missing MiniApp concrete runtime detector',
      },
      {
        regex: /\bstruct DefaultMiniAppRuntimeProbe\b/,
        message: 'missing MiniApp default runtime probe owner',
      },
      {
        regex: /\bwhich::which\b/,
        message: 'missing MiniApp PATH lookup owner',
      },
      {
        regex: /\bstd::fs::read_dir\b/,
        message: 'missing MiniApp version-manager directory scan owner',
      },
      {
        regex: /\bcreate_version_command\b/,
        message: 'missing MiniApp version process command owner',
      },
      {
        regex: /\bpub fn runtime_lookup_order\b/,
        message: 'missing MiniApp runtime lookup order contract',
      },
      {
        regex: /\bpub trait MiniAppRuntimeProbe\b/,
        message: 'missing MiniApp runtime probe contract',
      },
      {
        regex: /\bpub fn detect_runtime_with_probe\b/,
        message: 'missing MiniApp runtime detector facade',
      },
      {
        regex: /\bpub fn candidate_executable_path\b/,
        message: 'missing MiniApp runtime candidate executable helper',
      },
      {
        regex: /\bpub fn versioned_executable_candidate\b/,
        message: 'missing MiniApp version-manager executable helper',
      },
    ],
  },
  {
    path: 'src/crates/contracts/product-domains/src/miniapp/worker.rs',
    reason:
      'product-domains owns MiniApp worker pool policy and install-deps planning while core keeps worker process execution',
    patterns: [
      {
        regex: /\bpub enum InstallDepsPlan\b/,
        message: 'missing MiniApp install-deps plan contract',
      },
      {
        regex: /\bpub fn plan_install_deps\b/,
        message: 'missing MiniApp install-deps planning helper',
      },
      {
        regex: /\bpub fn worker_pool_capacity\b/,
        message: 'missing MiniApp worker pool capacity policy helper',
      },
      {
        regex: /\bpub fn worker_idle_timeout_ms\b/,
        message: 'missing MiniApp worker idle timeout policy helper',
      },
      {
        regex: /\bpub fn worker_is_idle\b/,
        message: 'missing MiniApp worker idle policy helper',
      },
      {
        regex: /\bpub fn select_lru_worker\b/,
        message: 'missing MiniApp worker LRU selection helper',
      },
      {
        regex: /\binstall_deps_plan_preserves_no_package_noop_and_runtime_commands\b/,
        message: 'missing MiniApp install-deps planning regression test',
      },
      {
        regex: /\bworker_pool_policy_keeps_existing_capacity_and_idle_timeout_contract\b/,
        message: 'missing MiniApp worker pool policy regression test',
      },
    ],
  },
  {
    path: 'src/crates/contracts/product-domains/src/miniapp/host_routing.rs',
    reason:
      'product-domains owns MiniApp host-routing and allowlist decision policy while core keeps host execution',
    patterns: [
      {
        regex: /\bpub fn split_host_method\b/,
        message: 'missing MiniApp host method split helper',
      },
      {
        regex: /\bpub enum FsAccessMode\b/,
        message: 'missing MiniApp fs access mode contract',
      },
      {
        regex: /\bpub fn fs_method_access_mode\b/,
        message: 'missing MiniApp fs access mode helper',
      },
      {
        regex: /\bpub enum MiniAppFsHostCallPlan\b/,
        message: 'missing MiniApp fs host-call plan contract',
      },
      {
        regex: /\bpub fn plan_fs_host_call\b/,
        message: 'missing MiniApp fs host-call planner',
      },
      {
        regex: /\bpub fn plan_fs_legacy_path_check\b/,
        message: 'missing MiniApp legacy fs path-gate planner',
      },
      {
        regex: /\bpub fn fs_policy_scopes\b/,
        message: 'missing MiniApp fs policy scope helper',
      },
      {
        regex: /\bpub fn fs_resolved_path_allowed\b/,
        message: 'missing MiniApp fs resolved path helper',
      },
      {
        regex: /\bpub fn command_basename_for_allowlist\b/,
        message: 'missing MiniApp command basename allowlist helper',
      },
      {
        regex: /\bpub fn command_basename_allowed\b/,
        message: 'missing MiniApp command allowlist policy helper',
      },
      {
        regex: /\bpub fn host_allowed_by_allowlist\b/,
        message: 'missing MiniApp host allowlist policy helper',
      },
      {
        regex: /\bpub fn shell_exec_first_token\b/,
        message: 'missing MiniApp shell first-token policy helper',
      },
      {
        regex: /\bpub fn shell_exec_input_is_empty\b/,
        message: 'missing MiniApp shell empty-input policy helper',
      },
      {
        regex: /\bpub fn shell_exec_cwd\b/,
        message: 'missing MiniApp shell cwd policy helper',
      },
      {
        regex: /\bpub fn shell_exec_timeout_ms\b/,
        message: 'missing MiniApp shell timeout policy helper',
      },
      {
        regex: /\bpub fn shell_exec_default_env\b/,
        message: 'missing MiniApp shell env policy helper',
      },
      {
        regex: /\bpub struct MiniAppShellHostCallPlan\b/,
        message: 'missing MiniApp shell host-call plan contract',
      },
      {
        regex: /\bpub fn plan_shell_host_call\b/,
        message: 'missing MiniApp shell host-call planner',
      },
      {
        regex: /\bfs_method_access_mode_preserves_access_bypass_and_default_read_contract\b/,
        message: 'missing MiniApp fs access mode regression test',
      },
      {
        regex: /\bfs_policy_scopes_and_resolved_prefix_check_preserve_path_boundary\b/,
        message: 'missing MiniApp fs path policy regression test',
      },
      {
        regex: /\bshell_exec_first_token_prefers_argv_over_shell_command_text\b/,
        message: 'missing MiniApp shell first-token regression test',
      },
      {
        regex: /\bshell_exec_plan_helpers_preserve_defaults_and_precedence\b/,
        message: 'missing MiniApp shell plan regression test',
      },
      {
        regex: /\bminiapp_host_fs_call_plans_preserve_existing_path_and_permission_contract\b/,
        message: 'missing MiniApp fs host-call plan regression test',
      },
      {
        regex: /\bminiapp_host_shell_call_plans_preserve_existing_input_and_default_contract\b/,
        message: 'missing MiniApp shell host-call plan regression test',
      },
    ],
  },
  {
    path: 'src/crates/contracts/product-domains/src/miniapp/exporter.rs',
    reason:
      'product-domains owns MiniApp export check result policy while core keeps runtime detection',
    patterns: [
      {
        regex: /\bpub const MISSING_JS_RUNTIME_MESSAGE\b/,
        message: 'missing MiniApp export missing-runtime message contract',
      },
      {
        regex: /\bpub fn export_runtime_label\b/,
        message: 'missing MiniApp export runtime label helper',
      },
      {
        regex: /\bpub fn build_export_check_result\b/,
        message: 'missing MiniApp export check result helper',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/miniapp/exporter.rs',
    reason:
      'core MiniApp exporter must delegate export check result policy while retaining runtime detection and export skeleton',
    patterns: [
      {
        regex: /\bdetect_runtime\b/,
        message: 'missing core-owned MiniApp export runtime detection',
      },
      {
        regex: /\bbuild_export_check_result\b/,
        message: 'missing product-domain MiniApp export check helper use',
      },
      {
        regex: /Export not yet implemented \(skeleton\)/,
        message: 'missing core-owned MiniApp export skeleton behavior',
      },
    ],
  },
  {
    path: 'src/crates/contracts/product-domains/src/miniapp/customization.rs',
    reason:
      'product-domains owns MiniApp customization metadata, built-in update policy, and permission-diff contracts while core keeps draft storage/runtime',
    patterns: [
      {
        regex: /\bpub struct MiniAppCustomizationMetadata\b/,
        message: 'missing MiniApp customization metadata contract',
      },
      {
        regex: /\bpub struct MiniAppDeclinedBuiltinUpdate\b/,
        message: 'missing MiniApp declined built-in update contract',
      },
      {
        regex: /\bpub struct MiniAppPermissionDiff\b/,
        message: 'missing MiniApp permission diff contract',
      },
      {
        regex: /\bpub fn diff_permissions\b/,
        message: 'missing MiniApp permission diff helper',
      },
      {
        regex: /\bpub fn apply_draft_customization_metadata\b/,
        message: 'missing MiniApp customization draft-apply helper',
      },
      {
        regex: /\bpub fn mark_builtin_update_available_metadata\b/,
        message: 'missing MiniApp built-in update availability helper',
      },
      {
        regex: /\bpub fn decline_builtin_update_metadata\b/,
        message: 'missing MiniApp built-in update decline helper',
      },
      {
        regex: /\bpub fn is_current_declined_builtin_update\b/,
        message: 'missing MiniApp declined update current-state helper',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/miniapp/manager.rs',
    reason:
      'core MiniApp manager must use product-domain policy/facade helpers while retaining compile, storage IO, and built-in source-hash lookup',
    patterns: [
      {
        regex: /\bapply_draft_customization_metadata\b/,
        message: 'missing product-domain draft customization helper use',
      },
      {
        regex: /\bmark_builtin_update_available_metadata\b/,
        message: 'missing product-domain built-in update availability helper use',
      },
      {
        regex: /\bdecline_builtin_update_metadata\b/,
        message: 'missing product-domain built-in update decline helper use',
      },
      {
        regex: /\bMiniAppRuntimeFacade\b/,
        message: 'missing product-domain MiniApp runtime-state facade use',
      },
      {
        regex: /\bbuild_created_app\b/,
        message: 'missing product-domain MiniApp create lifecycle helper use',
      },
      {
        regex: /\bapply_update_patch\b/,
        message: 'missing product-domain MiniApp update lifecycle helper use',
      },
      {
        regex: /\bprepare_draft_app\b/,
        message: 'missing product-domain MiniApp draft prepare lifecycle helper use',
      },
      {
        regex: /\bapply_draft_to_active\b/,
        message: 'missing product-domain MiniApp draft apply lifecycle helper use',
      },
      {
        regex: /\bCoreProductDomainRuntime\b/,
        message: 'missing core-owned product-domain runtime owner delegation',
      },
      {
        regex: /\bpersist_sync_from_fs_result_for_app\b/,
        message: 'missing product-domain MiniApp sync-from-fs facade delegation',
      },
      {
        regex: /\bcompile_source\b/,
        message: 'missing core-owned MiniApp compile orchestration',
      },
      {
        regex: /\bREQUIRED_SOURCE_FILES\b/,
        message: 'missing product-domain MiniApp import file-shape contract use',
      },
      {
        regex: /\bMiniAppImportLayout\b/,
        message: 'missing product-domain MiniApp import layout helper use',
      },
      {
        regex: /\bbuild_import_fallbacks\b/,
        message: 'missing product-domain MiniApp import fallback helper use',
      },
      {
        regex: /\bprepare_imported_meta\b/,
        message: 'missing product-domain MiniApp imported metadata helper use',
      },
      {
        regex: /\bpersist_import_runtime_state\b/,
        message: 'missing product-domain MiniApp import runtime-state facade delegation',
      },
      {
        regex: /\bstorage\.load_customization_metadata\b/,
        message: 'missing core-owned customization metadata storage IO',
      },
      {
        regex: /\bruntime_preflight_preserves_recompile_sync_rollback_and_deps_state\b/,
        message: 'missing MiniApp manager runtime preflight regression test',
      },
      {
        regex: /\bimport_from_path_preserves_fallback_files_recompile_and_runtime_state\b/,
        message: 'missing MiniApp import runtime preflight regression test',
      },
    ],
  },
  {
    path: 'src/crates/contracts/product-domains/src/miniapp/ports.rs',
    reason:
      'product-domains owns MiniApp runtime-state port facade while core keeps concrete storage IO, compile, worker, and host execution',
    patterns: [
      {
        regex: /\bpub struct MiniAppRuntimeFacade\b/,
        message: 'missing MiniApp runtime-state facade',
      },
      {
        regex: /\bmark_deps_installed_state\b/,
        message: 'missing MiniApp deps-installed state transition in facade',
      },
      {
        regex: /\bpersist_sync_from_fs_result_for_app\b/,
        message: 'missing MiniApp sync-from-fs preloaded snapshot facade path',
      },
      {
        regex: /\bpersist_import_runtime_state\b/,
        message: 'missing MiniApp import runtime-state facade path',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/function_agents/runtime_services.rs',
    reason:
      'core function-agent runtime services must continue owning AI concrete calls while product-domains owns prompt, parser, and facade policy',
    patterns: [
      {
        regex: /\bprepare_commit_ai_prompt\b/,
        message: 'missing product-domain Git function-agent prompt policy use',
      },
      {
        regex: /\bparse_commit_ai_response\b/,
        message: 'missing product-domain Git function-agent response policy use',
      },
      {
        regex: /\bbuild_work_state_analysis_prompt\b/,
        message: 'missing product-domain Startchat prompt policy use',
      },
      {
        regex: /\bparse_work_state_analysis_response\b/,
        message: 'missing product-domain Startchat response policy use',
      },
      {
        regex: /\bai_client\s*\.\s*send_message\b/,
        message: 'missing core-owned function-agent AI call',
      },
      {
        regex: /\bAgentError::internal_error\b/,
        message: 'missing core-owned function-agent AI transport error mapping',
      },
      {
        regex: /\bCoreCommitAiAnalysisService\b/,
        message: 'missing core-owned commit AI concrete service',
      },
      {
        regex: /\bCoreWorkStateAiAnalysisService\b/,
        message: 'missing core-owned Startchat AI concrete service',
      },
      {
        regex: /\bparse_commit_response_preserves_product_domain_response_policy\b/,
        message: 'missing Git function-agent AI response boundary regression test',
      },
      {
        regex: /\bparse_complete_analysis_preserves_product_domain_response_policy\b/,
        message: 'missing Startchat AI response boundary regression test',
      },
    ],
  },
  {
    path: 'src/crates/services/services-integrations/src/function_agents.rs',
    reason:
      'services-integrations must own function-agent concrete Git snapshots without depending on bitfun-core',
    patterns: [
      {
        regex: /\bpub struct FunctionAgentGitService\b/,
        message: 'missing function-agent Git service owner',
      },
      {
        regex: /\bgit_commit_snapshot\b/,
        message: 'missing commit snapshot service method',
      },
      {
        regex: /\bstartchat_git_snapshot\b/,
        message: 'missing Startchat Git snapshot service method',
      },
      {
        regex: /\bstartchat_time_snapshot\b/,
        message: 'missing Startchat time snapshot service method',
      },
      {
        regex: /\bGitService::get_status\b/,
        message: 'missing shared GitService status lookup',
      },
      {
        regex: /\bGitService::get_diff\b/,
        message: 'missing shared GitService staged diff lookup',
      },
      {
        regex: /\bContextAnalyzer::analyze_project_context\b/,
        message: 'missing product-domain project context analysis',
      },
      {
        regex: /\bprocess_manager::create_command\("git"\)/,
        message: 'missing process-manager backed lenient Git command fallback',
      },
      {
        regex: /\bgit_unpushed_commits\b/,
        message: 'missing unpushed-commit fallback helper',
      },
      {
        regex: /\bgit_ahead_behind\b/,
        message: 'missing ahead/behind fallback helper',
      },
      {
        regex: /\bgit_last_commit_timestamp\b/,
        message: 'missing last-commit timestamp helper',
      },
    ],
  },
  {
    path: 'src/crates/services/services-integrations/tests/function_agent_contracts.rs',
    reason:
      'services-integrations function-agent Git service must preserve legacy Git snapshot behavior',
    patterns: [
      {
        regex: /\bgit_service_builds_commit_snapshot_from_staged_diff_without_unstaged_content\b/,
        message: 'missing staged/unstaged boundary regression test',
      },
      {
        regex: /\bgit_service_startchat_snapshot_preserves_no_head_and_non_git_fallback\b/,
        message: 'missing Startchat no-HEAD and non-Git fallback regression test',
      },
      {
        regex: /\bgit_service_time_snapshot_uses_last_commit_timestamp\b/,
        message: 'missing time snapshot regression test',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/function_agents/git-func-agent/ai_service.rs',
    reason:
      'legacy Git function-agent AI service path must remain a compatibility re-export only',
    patterns: [
      {
        regex: /\bCoreCommitAiAnalysisService as AIAnalysisService\b/,
        message: 'missing Git function-agent AI service compatibility re-export',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/function_agents/startchat-func-agent/ai_service.rs',
    reason:
      'legacy Startchat AI service path must remain a compatibility re-export only',
    patterns: [
      {
        regex: /\bCoreWorkStateAiAnalysisService as AIWorkStateService\b/,
        message: 'missing Startchat AI service compatibility re-export',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/function_agents/git-func-agent/commit_generator.rs',
    reason:
      'legacy Git commit generator must delegate to the core product-domain runtime owner',
    patterns: [
      {
        regex: /\bCoreProductDomainRuntime\b/,
        message: 'missing core product-domain runtime owner routing',
      },
      {
        regex: /\bgenerate_function_agent_commit_message\b/,
        message: 'missing Git commit owner method delegation',
      },
    ],
  },
  {
    path: 'src/crates/contracts/product-domains/src/miniapp/builtin.rs',
    reason:
      'product-domains owns built-in MiniApp bundle assets, marker, hash, and seed-decision contracts while core keeps asset seeding IO and recompilation',
    patterns: [
      {
        regex: /id: "builtin-pr-review"/,
        message: 'missing built-in PR Review MiniApp bundle anchor',
      },
      {
        regex: /\bpub const BUILTIN_APPS\b/,
        message: 'missing built-in MiniApp bundle asset owner',
      },
      {
        regex: /\bpub struct BuiltinMiniAppBundle\b/,
        message: 'missing built-in MiniApp bundle contract',
      },
      {
        regex: /\bpub struct BuiltinInstallMarker\b/,
        message: 'missing built-in MiniApp install marker contract',
      },
      {
        regex: /\bpub const BUILTIN_INSTALL_MARKER\b/,
        message: 'missing built-in MiniApp marker filename contract',
      },
      {
        regex: /\bpub fn builtin_content_hash\b/,
        message: 'missing built-in MiniApp content hash helper',
      },
      {
        regex: /\bpub fn should_seed_builtin_app\b/,
        message: 'missing built-in MiniApp seed decision helper',
      },
      {
        regex: /\bpub struct BuiltinSeedArtifacts\b/,
        message: 'missing built-in MiniApp seed artifacts contract',
      },
      {
        regex: /\bpub enum BuiltinSeedCheck\b/,
        message: 'missing built-in MiniApp seed check contract',
      },
      {
        regex: /\bpub enum BuiltinSeedAction\b/,
        message: 'missing built-in MiniApp seed action contract',
      },
      {
        regex: /\bpub fn resolve_builtin_seed_check\b/,
        message: 'missing built-in MiniApp seed check helper',
      },
      {
        regex: /\bpub fn resolve_builtin_seed_action\b/,
        message: 'missing built-in MiniApp seed action helper',
      },
      {
        regex: /\bpub fn serialize_builtin_install_marker\b/,
        message: 'missing built-in MiniApp marker serialization helper',
      },
      {
        regex: /\bpub fn parse_builtin_install_marker\b/,
        message: 'missing built-in MiniApp marker parse helper',
      },
      {
        regex: /\bpub fn builtin_source_files\b/,
        message: 'missing built-in MiniApp source payload helper',
      },
      {
        regex: /\bpub const BUILTIN_PLACEHOLDER_COMPILED_HTML\b/,
        message: 'missing built-in MiniApp placeholder payload contract',
      },
      {
        regex: /\bpub fn build_builtin_package_json\b/,
        message: 'missing built-in MiniApp package payload helper',
      },
      {
        regex: /\bpub fn preserved_builtin_created_at\b/,
        message: 'missing built-in MiniApp created-at preservation helper',
      },
      {
        regex: /\bpub fn build_builtin_seed_meta\b/,
        message: 'missing built-in MiniApp seed meta helper',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/function_agents/startchat-func-agent/work_state_analyzer.rs',
    reason:
      'legacy Startchat work-state analyzer must delegate to the core product-domain runtime owner',
    patterns: [
      {
        regex: /\bCoreProductDomainRuntime\b/,
        message: 'missing core product-domain runtime owner routing',
      },
      {
        regex: /\banalyze_function_agent_work_state\b/,
        message: 'missing Startchat work-state owner method delegation',
      },
    ],
  },
  {
    path: 'src/crates/contracts/product-domains/src/function_agents/ports.rs',
    reason:
      'product-domains owns port-backed function-agent facade orchestration while providers/core keep concrete Git/AI runtime calls',
    patterns: [
      {
        regex: /\bpub struct FunctionAgentRuntimeFacade\b/,
        message: 'missing function-agent runtime facade',
      },
      {
        regex: /\bgenerate_commit_message\b/,
        message: 'missing function-agent commit facade orchestration',
      },
      {
        regex: /\banalyze_work_state\b/,
        message: 'missing function-agent work-state facade orchestration',
      },
      {
        regex: /\bgit_work_state_from_snapshot\b/,
        message: 'missing Startchat Git snapshot projection helper',
      },
      {
        regex: /\bStartchatTimeSnapshot\b/,
        message: 'missing Startchat time snapshot contract',
      },
      {
        regex: /\bstartchat_time_snapshot\b/,
        message: 'missing Startchat time snapshot port',
      },
    ],
  },
  {
    path: 'src/crates/contracts/product-domains/src/function_agents/common.rs',
    reason:
      'product-domains owns function-agent AI response JSON extraction while core keeps concrete AI clients',
    patterns: [
      {
        regex: /\bfn extract_json_from_ai_response\b/,
        message: 'missing function-agent AI response JSON extraction helper',
      },
      {
        regex: /\bfn try_repair_json\b/,
        message: 'missing function-agent AI response JSON repair helper',
      },
    ],
  },
  {
    path: 'src/crates/contracts/product-domains/src/function_agents/startchat_func_agent/utils.rs',
    reason:
      'product-domains owns Startchat function-agent prompt and response policy while core keeps AI calls',
    patterns: [
      {
        regex: /\bpub const WORK_STATE_ANALYSIS_PROMPT\b/,
        message: 'missing product-domain Startchat prompt template',
      },
      {
        regex: /\bpub fn build_work_state_analysis_prompt\b/,
        message: 'missing product-domain Startchat prompt builder',
      },
      {
        regex: /\bpub struct ParsedCompleteAnalysis\b/,
        message: 'missing Startchat complete-analysis parse result contract',
      },
      {
        regex: /\bpub fn parse_complete_analysis_value\b/,
        message: 'missing Startchat complete-analysis value parser',
      },
      {
        regex: /\bpub fn parse_complete_analysis_json\b/,
        message: 'missing Startchat complete-analysis JSON parser',
      },
      {
        regex: /\bpub fn parse_work_state_analysis_response\b/,
        message: 'missing Startchat AI response policy',
      },
      {
        regex: /\bwork_state_ai_response_policy_extracts_json_and_maps_domain_errors\b/,
        message: 'missing Startchat AI response policy regression test',
      },
    ],
  },
  {
    path: 'src/crates/contracts/product-domains/src/function_agents/git_func_agent/utils.rs',
    reason:
      'product-domains owns Git function-agent prompt and response policy while core keeps AI calls',
    patterns: [
      {
        regex: /\bpub const COMMIT_MESSAGE_PROMPT\b/,
        message: 'missing product-domain Git function-agent prompt template',
      },
      {
        regex: /\bpub fn parse_commit_analysis_value\b/,
        message: 'missing Git function-agent commit analysis value parser',
      },
      {
        regex: /\bpub fn parse_commit_analysis_json\b/,
        message: 'missing Git function-agent commit analysis JSON parser',
      },
      {
        regex: /\bpub fn truncate_diff_for_commit_prompt\b/,
        message: 'missing Git function-agent diff truncation helper',
      },
      {
        regex: /\bpub fn prepare_commit_prompt\b/,
        message: 'missing Git function-agent prompt preparation helper',
      },
      {
        regex: /\bpub fn prepare_commit_ai_prompt\b/,
        message: 'missing Git function-agent AI prompt policy',
      },
      {
        regex: /\bpub fn parse_commit_ai_response\b/,
        message: 'missing Git function-agent AI response policy',
      },
      {
        regex: /\bcommit_ai_response_policy_extracts_json_and_maps_domain_errors\b/,
        message: 'missing Git function-agent AI response policy regression test',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/miniapp/runtime_detect.rs',
    reason:
      'core MiniApp runtime detection must be a compatibility facade over product-domain runtime detection',
    patterns: [
      {
        regex: /\bpub use bitfun_product_domains::miniapp::runtime::\{/,
        message: 'missing product-domain MiniApp runtime facade re-export',
      },
      {
        regex: /\bdetect_runtime\b/,
        message: 'missing product-domain detect_runtime facade export',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/miniapp/js_worker_pool.rs',
    reason:
      'core must continue owning MiniApp worker runtime adapter until process/runtime migration is reviewed',
    patterns: [
      {
        regex: /\bplan_install_deps\b/,
        message: 'missing product-domain install-deps plan use',
      },
      {
        regex: /\bworker_is_idle\b/,
        message: 'missing product-domain worker idle policy use',
      },
      {
        regex: /\bworker_pool_at_capacity\b/,
        message: 'missing product-domain worker capacity policy use',
      },
      {
        regex: /\bselect_lru_worker\b/,
        message: 'missing product-domain worker LRU policy use',
      },
      {
        regex: /\bimpl MiniAppRuntimePort for JsWorkerPool\b/,
        message: 'missing MiniApp runtime port adapter owner',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/function_agents/port_adapters.rs',
    reason:
      'core function-agent port adapters must stay thin adapters over integration Git services and core AI services',
    patterns: [
      {
        regex: /\bpub struct CoreFunctionAgentGitAdapter\b/,
        message: 'missing core function-agent Git adapter type',
      },
      {
        regex: /\bimpl FunctionAgentGitPort for CoreFunctionAgentGitAdapter\b/,
        message: 'missing function-agent Git port adapter owner',
      },
      {
        regex: /\bpub struct CoreFunctionAgentAiAdapter\b/,
        message: 'missing core function-agent AI adapter type',
      },
      {
        regex: /\bimpl FunctionAgentAiPort for CoreFunctionAgentAiAdapter\b/,
        message: 'missing function-agent AI port adapter owner',
      },
      {
        regex: /\bFunctionAgentGitService::git_commit_snapshot\b/,
        message: 'missing Git adapter delegation to integration runtime service',
      },
      {
        regex: /\bCoreCommitAiAnalysisService::new_with_agent_config\b/,
        message: 'missing commit AI adapter delegation to concrete runtime service',
      },
      {
        regex: /\bCoreWorkStateAiAnalysisService::new_with_agent_config\b/,
        message: 'missing Startchat AI adapter delegation to concrete runtime service',
      },
      {
        regex: /\bgit_adapter_commit_snapshot_keeps_staged_diff_and_unstaged_count_separate\b/,
        message: 'missing function-agent Git snapshot boundary regression test',
      },
      {
        regex: /\bgit_adapter_startchat_snapshot_preserves_git_state_when_diff_has_no_head\b/,
        message: 'missing Startchat Git diff fallback regression test',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/product_domain_runtime.rs',
    reason:
      'core product-domain runtime owner must centralize concrete MiniApp and function-agent runtime port bindings without moving runtime behavior',
    patterns: [
      {
        regex: /\bpub\(crate\) struct CoreProductDomainRuntime\b/,
        message: 'missing core product-domain runtime owner type',
      },
      {
        regex: /\bfn miniapp_runtime_facade\b/,
        message: 'missing MiniApp runtime facade owner factory',
      },
      {
        regex: /\bfn function_agent_git_adapter\b/,
        message: 'missing function-agent Git adapter owner factory',
      },
      {
        regex: /\bfn function_agent_ai_adapter\b/,
        message: 'missing function-agent AI adapter owner factory',
      },
      {
        regex: /\bfn function_agent_runtime_facade\b/,
        message: 'missing function-agent runtime facade owner factory',
      },
      {
        regex: /\bfn generate_function_agent_commit_message\b/,
        message: 'missing Git function-agent concrete runtime owner entrypoint',
      },
      {
        regex: /\bfn analyze_function_agent_work_state\b/,
        message: 'missing Startchat concrete runtime owner entrypoint',
      },
      {
        regex: /\bCoreFunctionAgentGitAdapter\b/,
        message: 'missing core-owned Git adapter binding',
      },
      {
        regex: /\bCoreFunctionAgentAiAdapter\b/,
        message: 'missing core-owned AI adapter binding',
      },
      {
        regex: /\bMiniAppRuntimeFacade\b/,
        message: 'missing MiniApp product-domain facade binding',
      },
      {
        regex: /\bMiniAppStoragePort\b/,
        message: 'missing MiniApp storage port owner binding',
      },
      {
        regex: /\bFunctionAgentRuntimeFacade\b/,
        message: 'missing function-agent product-domain facade binding',
      },
      {
        regex: /\bFunctionAgentGitPort\b/,
        message: 'missing function-agent Git port owner binding',
      },
      {
        regex: /\bFunctionAgentAiPort\b/,
        message: 'missing function-agent AI port owner binding',
      },
    ],
  },
];
