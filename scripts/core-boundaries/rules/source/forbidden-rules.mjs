// Boundary rules for source ownership, facades, and required owner content.

export const forbiddenContentRules = [
  {
    path: 'src/crates/contracts/core-types/src/ai.rs',
    patterns: [
      {
        regex: /\bresolve_request_url\b/,
        message:
          'core-types may own AI DTOs, but provider URL resolution belongs in adapter or assembly compatibility owners',
      },
      {
        regex: /\b(?:chat\/completions|v1\/messages|streamGenerateContent)\b/,
        message:
          'core-types must not encode provider endpoint paths; keep protocol URL behavior above contracts',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/product_assembly.rs',
    patterns: [
      {
        regex: /\bpub struct CoreRuntimeServicesProvider\b/,
        message:
          'core product_assembly must remain a compatibility facade; move core-specific runtime service providers to product runtime adapters',
      },
      {
        regex: /\bimpl RuntimeServicesProvider for CoreRuntimeServicesProvider\b/,
        message:
          'core product_assembly must not own runtime service provider registration; use the product runtime adapter path',
      },
      {
        regex: /\bCoreSessionStorePort\b/,
        message:
          'core product_assembly must not bind concrete session store adapters directly; use the product runtime adapter path',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/harness.rs',
    patterns: [
      {
        regex: /\bproduct_assembly_plan_for_profile\b/,
        message:
          'core agentic harness facade must not rebuild product assembly plans; use bitfun-product-capabilities harness registry entrypoints',
      },
      {
        regex: /\bfn product_harness_registry_for_profile\b/,
        message:
          'core agentic harness facade must not own profile-scoped harness registry construction',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/deep_review_policy.rs',
    patterns: [
      {
        regex: /\bstatic GLOBAL_DEEP_REVIEW_BUDGET_TRACKER\b/,
        message:
          'core DeepReview policy facade must not re-own runtime budget state; use bitfun-agent-runtime::deep_review',
      },
      {
        regex: /\bstatic GLOBAL_DEEP_REVIEW_QUEUE_CONTROL_TRACKER\b/,
        message:
          'core DeepReview policy facade must not re-own queue control state; use bitfun-agent-runtime::deep_review',
      },
      {
        regex: /\bpub struct DeepReviewExecutionPolicy\b/,
        message:
          'core DeepReview policy facade must not redefine execution policy; use bitfun-agent-runtime::deep_review',
      },
      {
        regex: /\bpub fn record_deep_review_task_budget\b/,
        message:
          'core DeepReview policy facade must not re-own task budget recording; use bitfun-agent-runtime::deep_review',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/deep_review/report.rs',
    patterns: [
      {
        regex: /\bfn fill_deep_review_packet_metadata\b/,
        message:
          'core DeepReview report must not re-own packet metadata enrichment; use bitfun-agent-runtime::deep_review::report',
      },
      {
        regex: /\bfn deep_review_cache_from_completed_reviewers\b/,
        message:
          'core DeepReview report must not re-own cache update logic; use bitfun-agent-runtime::deep_review::report',
      },
      {
        regex: /\bstruct DeepReviewCacheUpdate\b/,
        message:
          'core DeepReview report must not re-own cache update DTO; use bitfun-agent-runtime::deep_review::report',
      },
      {
        regex: /"kind": "concurrency_limited"/,
        message:
          'core DeepReview report must not re-own runtime tracker reliability signal shaping; use bitfun-agent-runtime::deep_review::report',
      },
      {
        regex: /DeepReview runtime diagnostics:/,
        message:
          'core DeepReview report must not re-own runtime diagnostics log formatting; use bitfun-agent-runtime::deep_review::diagnostics',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/deep_review/task_adapter.rs',
    patterns: [
      {
        regex: /\bfn string_for_any_key\b/,
        message:
          'core DeepReview task adapter must not re-own manifest key normalization; use bitfun-agent-runtime::deep_review::task_execution',
      },
      {
        regex: /\bfn deep_review_packet_id_for_cache\b/,
        message:
          'core DeepReview task adapter must not re-own packet id inference; use bitfun-agent-runtime::deep_review::task_execution',
      },
      {
        regex: /\bfn ensure_deep_review_retry_coverage\b/,
        message:
          'core DeepReview task adapter must not re-own retry coverage validation; use bitfun-agent-runtime::deep_review::task_execution',
      },
      {
        regex: /\bfn provider_capacity_queue_wait_seconds_for_attempt\b/,
        message:
          'core DeepReview task adapter must not re-own provider capacity backoff; use bitfun-agent-runtime::deep_review::task_execution',
      },
      {
        regex: /\bclassify_deep_review_capacity_error\b/,
        message:
          'core DeepReview task adapter must not directly classify provider or local capacity errors; use bitfun-agent-runtime::deep_review::task_execution',
      },
      {
        regex: /\bDeepReviewCapacityFailFastReason::DeterministicProviderError\b/,
        message:
          'core DeepReview task adapter must not re-own provider capacity category fallback; use bitfun-agent-runtime::deep_review::task_execution',
      },
      {
        regex: /\bfn provider_capacity_wait_can_wake_on_active_reviewer_release\b/,
        message:
          'core DeepReview task adapter must not re-own provider capacity queue wake policy; use bitfun-agent-runtime::deep_review::task_execution',
      },
      {
        regex: /\bqueue_expired_without_active_reviewer\b/,
        message:
          'core DeepReview task adapter must not re-own reviewer admission queue expiry policy; use bitfun-agent-runtime::deep_review::task_execution',
      },
      {
        regex: /\bstruct QueueWaitTimer\b/,
        message:
          'core DeepReview task adapter must not re-own queue wait timing; use bitfun-agent-runtime::deep_review::task_execution',
      },
      {
        regex: /runtime_task_execution::decide_provider_capacity_queue_step/,
        message:
          'core DeepReview task adapter must use the runtime provider queue state machine instead of direct step decisions',
      },
      {
        regex: /runtime_task_execution::decide_blocked_reviewer_admission_queue_step/,
        message:
          'core DeepReview task adapter must use the runtime reviewer admission state machine instead of direct step decisions',
      },
      {
        regex: /\bcontrol_snapshot\.(?:cancelled|paused|skip_optional)\b/,
        message:
          'core DeepReview task adapter must not re-own queue control decision priority; use bitfun-agent-runtime::deep_review::task_execution',
      },
      {
        regex: /\bfn prompt_with_deep_review_retry_scope\b/,
        message:
          'core DeepReview task adapter must not re-own retry prompt shaping; use bitfun-agent-runtime::deep_review::task_execution',
      },
      {
        regex: /<partial_result status=/,
        message:
          'core DeepReview task adapter must not re-own task completion result presentation; use bitfun-agent-runtime::deep_review::task_execution',
      },
      {
        regex: /completed successfully with result:/,
        message:
          'core DeepReview task adapter must not re-own task completion result presentation; use bitfun-agent-runtime::deep_review::task_execution',
      },
      {
        regex: /Retries used:/,
        message:
          'core DeepReview task adapter must not re-own retry guidance presentation; use bitfun-agent-runtime::deep_review::task_execution',
      },
      {
        regex: /DeepReview automatic retry elapsed guard exceeded/,
        message:
          'core DeepReview task adapter must not re-own auto-retry admission policy; use bitfun-agent-runtime::deep_review::task_execution',
      },
      {
        regex: /cancelled coverage/,
        message:
          'core DeepReview task adapter must not re-own cancelled reviewer presentation; use bitfun-agent-runtime::deep_review::task_execution',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/implementations/task_tool.rs',
    patterns: [
      {
        regex: /\bDeepReviewIncrementalCache\b/,
        message:
          'TaskTool must not directly inspect DeepReview incremental cache internals; use deep_review_task_adapter',
      },
      {
        regex: /deepReviewCache/,
        message:
          'TaskTool must not directly parse DeepReview cache payloads; use deep_review_task_adapter',
      },
      {
        regex: /<partial_result status=/,
        message:
          'TaskTool must not re-own DeepReview task completion result presentation; use deep_review_task_adapter',
      },
      {
        regex: /completed successfully with result:/,
        message:
          'TaskTool must not re-own DeepReview task completion result presentation; use deep_review_task_adapter',
      },
      {
        regex: /Retries used:/,
        message:
          'TaskTool must not re-own DeepReview retry guidance presentation; use deep_review_task_adapter',
      },
      {
        regex: /DeepReview automatic retry elapsed guard exceeded/,
        message:
          'TaskTool must not re-own DeepReview auto-retry admission policy; use deep_review_task_adapter',
      },
      {
        regex: /cancelled coverage/,
        message:
          'TaskTool must not re-own DeepReview cancelled reviewer presentation; use deep_review_task_adapter',
      },
      {
        regex: /\bprovider_capacity_retry_attempts\b/,
        message:
          'TaskTool must not re-own DeepReview provider capacity retry attempts; use deep_review_task_adapter',
      },
      {
        regex: /\bprovider_capacity_queue_elapsed_ms\b/,
        message:
          'TaskTool must not re-own DeepReview provider capacity queue elapsed aggregation; use deep_review_task_adapter',
      },
      {
        regex: /\bDEEP_REVIEW_PROVIDER_CAPACITY_MAX_RETRY_ATTEMPTS\b/,
        message:
          'TaskTool must not re-own DeepReview provider capacity retry limits; use deep_review_task_adapter',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/implementations/code_review_tool.rs',
    patterns: [
      {
        regex: /"kind"\s*:\s*"cache_hit"/,
        message:
          'CodeReviewTool must not re-own DeepReview cache-hit reliability signal shaping; use deep_review_report',
      },
      {
        regex: /"kind"\s*:\s*"cache_miss"/,
        message:
          'CodeReviewTool must not re-own DeepReview cache-miss reliability signal shaping; use deep_review_report',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/agents/prompt_builder/prompt_builder_impl.rs',
    patterns: [
      {
        regex: /\bComputer use \/ `key_chord`\b/,
        message:
          'core prompt builder must not re-own ComputerUse environment guidance; use bitfun-agent-runtime::prompt',
      },
      {
        regex: /\bfn computer_use_key_chord_guidance\b/,
        message:
          'core prompt builder must not re-own prompt environment guidance helpers; use bitfun-agent-runtime::prompt',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/agents/citation_renumber.rs',
    patterns: [
      {
        regex: /\bfn parse_registry_status\b/,
        message:
          'core DeepResearch citation hook must not re-own registry parsing; use bitfun-agent-runtime::deep_research',
      },
      {
        regex: /\bfn renumber_body\b/,
        message:
          'core DeepResearch citation hook must not re-own body renumbering; use bitfun-agent-runtime::deep_research',
      },
      {
        regex: /\bfn renumber_index_section\b/,
        message:
          'core DeepResearch citation hook must not re-own Citation Index rewriting; use bitfun-agent-runtime::deep_research',
      },
      {
        regex: /\bstatic CIT_ID_RE\b/,
        message:
          'core DeepResearch citation hook must not re-own citation regex parsing; use bitfun-agent-runtime::deep_research',
      },
      {
        regex: /\btokio::fs\b/,
        message:
          'core DeepResearch citation hook must not own report filesystem IO; use bitfun-services-integrations::deep_research',
      },
      {
        regex: /\bdisplay_map\.json\b/,
        message:
          'core DeepResearch citation hook must not own display-map sidecar persistence; use bitfun-services-integrations::deep_research',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/miniapp/host_dispatch.rs',
    patterns: [
      {
        regex: /\btokio::fs\b/,
        message:
          'core MiniApp host-dispatch adapter must not own filesystem execution; use bitfun-services-integrations::miniapp::host_dispatch',
      },
      {
        regex: /\bprocess_manager::create_tokio_command\b/,
        message:
          'core MiniApp host-dispatch adapter must not own shell process execution; use bitfun-services-integrations::miniapp::host_dispatch',
      },
      {
        regex: /\breqwest::Client\b/,
        message:
          'core MiniApp host-dispatch adapter must not own net.fetch execution; use bitfun-services-integrations::miniapp::host_dispatch',
      },
      {
        regex: /\basync fn dispatch_fs\b/,
        message:
          'core MiniApp host-dispatch adapter must not re-own fs dispatch helpers',
      },
      {
        regex: /\basync fn dispatch_shell\b/,
        message:
          'core MiniApp host-dispatch adapter must not re-own shell dispatch helpers',
      },
    ],
  },
  {
    path: 'src/crates/services/services-integrations/src/miniapp/host_dispatch.rs',
    patterns: [
      {
        regex: /\bresolve_policy\s*\(/,
        message:
          'services MiniApp host-dispatch must use MiniAppPermissionPolicyRequest for permission path adaptation',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/miniapp/js_worker.rs',
    patterns: [
      {
        regex: /\btokio::process\b/,
        message:
          'core MiniApp JS worker facade must not own worker process types; use bitfun-services-integrations::miniapp::worker',
      },
      {
        regex: /\bprocess_manager::create_tokio_command\b/,
        message:
          'core MiniApp JS worker facade must not spawn worker processes; use bitfun-services-integrations::miniapp::worker',
      },
      {
        regex: /\bPendingResponseMap\b/,
        message:
          'core MiniApp JS worker facade must not own JSON-RPC response routing; use bitfun-services-integrations::miniapp::worker',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/miniapp/js_worker_pool.rs',
    patterns: [
      {
        regex: /\bworker_pool_at_capacity\b/,
        message:
          'core MiniApp worker pool facade must not own pool policy; use bitfun-services-integrations::miniapp::worker_pool',
      },
      {
        regex: /\bselect_lru_worker\b/,
        message:
          'core MiniApp worker pool facade must not own LRU policy; use bitfun-services-integrations::miniapp::worker_pool',
      },
      {
        regex: /\bplan_install_deps\b/,
        message:
          'core MiniApp worker pool facade must not own install-deps planning; use bitfun-services-integrations::miniapp::worker_pool',
      },
      {
        regex: /\bprocess_manager::create_tokio_command\b/,
        message:
          'core MiniApp worker pool facade must not execute install-deps processes; use bitfun-services-integrations::miniapp::worker_pool',
      },
      {
        regex: /\bHashMap<String, WorkerEntry>\b/,
        message:
          'core MiniApp worker pool facade must not own worker pool state; use bitfun-services-integrations::miniapp::worker_pool',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/miniapp/storage.rs',
    patterns: [
      {
        regex: /\btokio::fs\b/,
        message:
          'core MiniApp storage facade must not own filesystem IO; use bitfun-services-integrations::miniapp::storage',
      },
      {
        regex: /\bMiniAppStorageLayout\b/,
        message:
          'core MiniApp storage facade must not own storage layout logic; use bitfun-services-integrations::miniapp::storage',
      },
      {
        regex: /\bbuild_package_json\b/,
        message:
          'core MiniApp storage facade must not own package-json storage assembly; use bitfun-services-integrations::miniapp::storage',
      },
      {
        regex: /\bparse_npm_dependencies\b/,
        message:
          'core MiniApp storage facade must not own package-json dependency parsing; use bitfun-services-integrations::miniapp::storage',
      },
      {
        regex: /\bDRAFTS_CLEANUP_MARKER\b/,
        message:
          'core MiniApp storage facade must not own draft cleanup marker IO; use bitfun-services-integrations::miniapp::storage',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/function_agents/runtime_services.rs',
    patterns: [
      {
        regex: /\bCoreFunctionAgentGitService\b/,
        message:
          'core function-agent runtime services must not re-own Git concrete snapshots; use bitfun-services-integrations::function_agents',
      },
      {
        regex: /\bgit_stdout_lenient\b/,
        message:
          'core function-agent runtime services must not re-own lenient Git process fallback; use bitfun-services-integrations::function_agents',
      },
      {
        regex: /\bGitService::get_status\b/,
        message:
          'core function-agent runtime services must not re-own Git status snapshots; use bitfun-services-integrations::function_agents',
      },
      {
        regex: /\bcreate_command\("git"\)/,
        message:
          'core function-agent runtime services must not spawn Git concrete commands; use bitfun-services-integrations::function_agents',
      },
    ],
  },
  {
    path: 'src/crates/assembly/product-capabilities/src/lib.rs',
    patterns: [
      {
        regex: /\bpub struct HarnessProviderDescriptor\b/,
        message:
          'product-capabilities must not redefine provider-neutral harness descriptors; use bitfun-harness',
      },
      {
        regex: /\bfn build_harness_registry_from_descriptors\b/,
        message:
          'product-capabilities must not own descriptor registry construction; use bitfun-harness',
      },
      {
        regex: /\bpub enum ProductCapabilityBuildError\b/,
        message:
          'product-capabilities must not redefine tool provider group selection errors; use bitfun-tool-packs',
      },
      {
        regex: /\bproduct_tool_provider_group_plan\(\)\b/,
        message:
          'product-capabilities must not scan product tool provider plans locally; use bitfun-tool-packs selector',
      },
      {
        regex: /\bdefault_product_tool_provider_group_plan\b/,
        message:
          'product-capabilities must expose product assembly, not a separate default tool-provider plan shortcut',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/filesystem/service.rs',
    patterns: [
      {
        regex: /\btokio::fs::/,
        message:
          'core filesystem service must not own async local filesystem IO; use bitfun-services-core filesystem primitives',
      },
      {
        regex: /\bstd::fs::/,
        message:
          'core filesystem service must not own sync local filesystem IO; use bitfun-services-core filesystem primitives',
      },
      {
        regex: /\bignore::WalkBuilder\b/,
        message:
          'core filesystem service must not own local file walking/search implementation; use bitfun-services-core filesystem primitives',
      },
      {
        regex: /\bsha2::/,
        message:
          'core filesystem service must not own editor-sync hashing implementation; use bitfun-services-core filesystem primitives',
      },
      {
        regex: /\bbase64::/,
        message:
          'core filesystem service must not own binary file encoding implementation; use bitfun-services-core filesystem primitives',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/search/service.rs',
    patterns: [
      {
        regex: /\bManagedClient\b/,
        message:
          'core workspace-search facade must not own flashgrep daemon clients; use bitfun-services-integrations::workspace_search',
      },
      {
        regex: /\bRepoSession\b/,
        message:
          'core workspace-search facade must not own flashgrep repo sessions; use bitfun-services-integrations::workspace_search',
      },
      {
        regex: /\bwith_scan_fallback\b/,
        message:
          'core workspace-search facade must not own scan fallback policy; use bitfun-services-integrations::workspace_search',
      },
      {
        regex: /\bconvert_hits_to_file_search_results\b/,
        message:
          'core workspace-search facade must not own hit conversion; use bitfun-services-integrations::workspace_search',
      },
      {
        regex: /\bsplit_preview\b/,
        message:
          'core workspace-search facade must not own preview mapping; use bitfun-services-integrations::workspace_search',
      },
      {
        regex: /\bdunce::canonicalize\b/,
        message:
          'core workspace-search facade must not own local search path normalization; use bitfun-services-integrations::workspace_search',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/search/mod.rs',
    patterns: [
      {
        regex: /\bbitfun_services_integrations::workspace_search::flashgrep\b/,
        message:
          'core must not import flashgrep internals; use the remote workspace-search stdio facade instead',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/search/service.rs',
    patterns: [
      {
        regex:
          /\b(?:owner::flashgrep|workspace_search::flashgrep|bitfun_services_integrations::workspace_search::flashgrep)\b/,
        message:
          'core workspace search facade must not depend on flashgrep internals; use stable workspace-search config and DTO APIs',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/search/remote.rs',
    patterns: [
      {
        regex: /\bconst\s+REMOTE_FLASHGREP_INSTALL_DIR\b/,
        message:
          'core remote workspace search must not own remote flashgrep install facts; use bitfun-services-integrations::remote_ssh::workspace_search',
      },
      {
        regex: /\bconst\s+REMOTE_(?:OS|ARCHITECTURE)_PROBES\b/,
        message:
          'core remote workspace search must not own remote probe facts; use bitfun-services-integrations::remote_ssh::workspace_search',
      },
      {
        regex: /\bstruct\s+LocalFlashgrepBundle\b/,
        message:
          'core remote workspace search must not own local remote-search bundle DTOs; use bitfun-services-integrations::remote_ssh::workspace_search',
      },
      {
        regex:
          /\bfn\s+(?:build_remote_scope|normalize_remote_scope_path|remote_flashgrep_install_dir|parse_remote_architecture_output|parse_remote_os_output|local_flashgrep_bundle_for_arch|remote_stdio_search_mode|should_retry_remote_scan_fallback_as_files_with_matches|join_remote_path|shell_escape)\b/,
        message:
          'core remote workspace search must not re-own provider-neutral remote search strategy helpers',
      },
      {
        regex: /\b(?:RemoteStdioRepoSession|RemoteStdioDaemonClient|RemoteSearchContext|REMOTE_STDIO_SESSIONS|REMOTE_SEARCH_CONTEXTS)\b/,
        message:
          'core remote workspace search must not own remote flashgrep session/context lifecycle; use bitfun-services-integrations::remote_ssh::workspace_search',
      },
      {
        regex: /\bfn\s+(?:ensure_remote_search_context|convert_stdio_search_results|remote_stdio_session_key|remote_search_context_key|schedule_remote_stdio_session_release)\b/,
        message:
          'core remote workspace search must not re-own remote search concrete lifecycle helpers',
      },
      {
        regex:
          /\b(?:ProtocolClient|drain_content_length_messages|log_flashgrep_stderr_line_with_context|FLASHGREP_LOG_TARGET)\b/,
        message:
          'core remote workspace search must not depend on flashgrep protocol internals; use RemoteWorkspaceSearchStdioProtocol',
      },
    ],
  },
  {
    path: 'src/crates/services/services-integrations/src/workspace_search/mod.rs',
    patterns: [
      {
        regex: /\bpub\s+mod\s+flashgrep\b/,
        message:
          'workspace_search must not publicly expose flashgrep protocol internals',
      },
    ],
  },
  {
    path: 'src/crates/services/services-integrations/src/remote_ssh/workspace_search/mod.rs',
    patterns: [
      {
        regex:
          /\bpub\s+(?:const|struct|fn)\s+(?:REMOTE_OS_PROBES|REMOTE_ARCHITECTURE_PROBES|LocalFlashgrepBundle|build_remote_scope|remote_flashgrep_install_dir|remote_workspace_search_storage_root|looks_like_linux_workspace_root|parse_remote_architecture_output|parse_remote_os_output|local_flashgrep_bundle_for_arch|remote_stdio_search_mode|should_retry_remote_scan_fallback_as_files_with_matches|join_remote_path|shell_escape)\b/,
        message:
          'remote workspace-search helper APIs must stay crate-internal; expose only reviewed provider/service contracts',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/miniapp/runtime_detect.rs',
    patterns: [
      {
        regex: /\bCoreMiniAppRuntimeProbe\b/,
        message:
          'core MiniApp runtime_detect must remain a compatibility facade; concrete probe owner is product-domains',
      },
      {
        regex: /\bwhich::which\b/,
        message:
          'core MiniApp runtime_detect must not own PATH lookup; use product-domain runtime detection',
      },
      {
        regex: /\bcreate_command\b/,
        message:
          'core MiniApp runtime_detect must not own version process execution; use product-domain runtime detection',
      },
      {
        regex: /\bstd::fs::read_dir\b/,
        message:
          'core MiniApp runtime_detect must not own version-manager directory scanning; use product-domain runtime detection',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/agents/prompt_builder/user_context.rs',
    patterns: [
      {
        regex: /\bpub\s+enum\s+UserContextSection\b/,
        message:
          'core prompt builder must not own user-context section facts; use bitfun-agent-runtime prompt contracts',
      },
      {
        regex: /\bpub\s+struct\s+UserContextPolicy\b/,
        message:
          'core prompt builder must not own user-context policy facts; use bitfun-agent-runtime prompt contracts',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/agents/mod.rs',
    patterns: [
      {
        regex: /\bpub const SHARED_CODING_MODE_PROMPT_TEMPLATE\b/,
        message:
          'core agent mode module must not own shared coding-mode prompt facts; use bitfun-agent-runtime agents',
      },
      {
        regex: /\bpub const SHARED_CODING_MODE_CONFIG_PROFILE_ID\b/,
        message:
          'core agent mode module must not own shared coding-mode config profile facts; use bitfun-agent-runtime agents',
      },
      {
        regex: /\bpub const SHARED_CODING_MODE_IDS\b/,
        message:
          'core agent mode module must not own shared coding-mode membership facts; use bitfun-agent-runtime agents',
      },
      {
        regex: /\bpub fn resolve_mode_config_profile_id\b/,
        message:
          'core agent mode module must not own mode config profile resolution; use bitfun-agent-runtime agents',
      },
      {
        regex: /\bpub fn mode_config_profile_member_mode_ids\b/,
        message:
          'core agent mode module must not own mode config profile membership; use bitfun-agent-runtime agents',
      },
      {
        regex: /\bpub fn mode_config_profile_label\b/,
        message:
          'core agent mode module must not own mode config profile labels; use bitfun-agent-runtime agents',
      },
      {
        regex: /\bpub fn shared_coding_mode_user_context_policy\b/,
        message:
          'core agent mode module must not own shared coding-mode context policy; use bitfun-agent-runtime agents',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/agents/registry/query.rs',
    patterns: [
      {
        regex: /"agentic"\s*=>\s*0[\s\S]*"Cowork"\s*=>\s*1/,
        message:
          'core agent registry query must not own mode presentation order; use bitfun-agent-runtime agents',
      },
      {
        regex: /\bfn subagent_source_rank\b/,
        message:
          'core agent registry query must not own subagent source presentation rank; use bitfun-agent-runtime agents',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/agents/registry/types.rs',
    patterns: [
      {
        regex: /\bpub enum SubAgentSource\b/,
        message:
          'core agent registry must not own subagent source DTOs; use bitfun-agent-runtime agents',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/session/prompt_cache.rs',
    patterns: [
      {
        regex: /\bpub const PROMPT_CACHE_SCHEMA_VERSION\b/,
        message:
          'core prompt cache must not own prompt-cache schema facts; use bitfun-agent-runtime prompt_cache',
      },
      {
        regex: /\bpub struct PromptCachePolicy\b/,
        message:
          'core prompt cache must not own prompt-cache policy; use bitfun-agent-runtime prompt_cache',
      },
      {
        regex: /\bpub struct SessionPromptCache\b/,
        message:
          'core prompt cache must not own prompt-cache DTOs; use bitfun-agent-runtime prompt_cache',
      },
      {
        regex: /\bpub enum PromptCacheScope\b/,
        message:
          'core prompt cache must not own prompt-cache invalidation scope; use bitfun-agent-runtime prompt_cache',
      },
      {
        regex: /\bpub struct SessionPromptCacheStore\b/,
        message:
          'core prompt cache must not own in-memory prompt-cache store; use bitfun-agent-runtime prompt_cache',
      },
      {
        regex: /\bpub enum PromptCacheLookup\b/,
        message:
          'core prompt cache must not own prompt-cache lookup outcomes; use bitfun-agent-runtime prompt_cache',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/agents/prompt_builder/prompt_builder_impl.rs',
    patterns: [
      {
        regex: /\bpub\s+struct\s+ToolListingSections\b/,
        message:
          'core prompt builder must not own tool-listing reminder facts; use bitfun-agent-runtime prompt contracts',
      },
      {
        regex: /\bpub\s+struct\s+PrependedPromptReminders\b/,
        message:
          'core prompt builder must not own prepended-reminder ordering facts; use bitfun-agent-runtime prompt contracts',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/execution/types.rs',
    patterns: [
      {
        regex: /\bpub\s+enum\s+FinishReason\b/,
        message:
          'core execution types must not own finish-reason event facts; use bitfun-agent-runtime events',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/events/types.rs',
    patterns: [
      {
        regex: /SessionState::Idle\s*=>\s*"idle"/,
        message:
          'core event types must not own session-state wire labels; use bitfun-agent-runtime events',
      },
      {
        regex: /SessionState::Processing\s*\{[^}]*\}\s*=>\s*"processing"/,
        message:
          'core event types must not own session-state wire labels; use bitfun-agent-runtime events',
      },
      {
        regex: /SessionState::Error\s*\{[^}]*\}\s*=>\s*"error"/,
        message:
          'core event types must not own session-state wire labels; use bitfun-agent-runtime events',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/coordination/scheduler.rs',
    patterns: [
      {
        regex: /\bconst\s+MAX_QUEUE_DEPTH\b/,
        message:
          'core scheduler must not own dialog queue capacity; use bitfun-agent-runtime scheduler',
      },
      {
        regex: /\bstd::collections::VecDeque\b/,
        message:
          'core scheduler must not own dialog queue storage; use bitfun-agent-runtime scheduler',
      },
      {
        regex: /\bdashmap::DashMap\b/,
        message:
          'core scheduler must not own scheduler state maps; use bitfun-agent-runtime scheduler stores',
      },
      {
        regex: /\bstruct\s+ActiveTurn\b/,
        message:
          'core scheduler must not own active-turn facts; use bitfun-agent-runtime scheduler',
      },
      {
        regex: /\bfn\s+format_agent_session_reply\b/,
        message:
          'core scheduler must not own agent-session reply text assembly; use bitfun-agent-runtime scheduler',
      },
      {
        regex: /automated reply to a previous SessionMessage call/,
        message:
          'core scheduler must not own agent-session reply reminder text; use bitfun-agent-runtime scheduler',
      },
      {
        regex: /RoundInjectionKind::UserSteering/,
        message:
          'core scheduler must not own steering injection construction; use bitfun-agent-runtime scheduler',
      },
      {
        regex: /RoundInjectionTarget::ExactTurn/,
        message:
          'core scheduler must not own steering exact-turn targeting; use bitfun-agent-runtime scheduler',
      },
      {
        regex: /RoundInjectionKind::ThreadGoalObjectiveUpdated/,
        message:
          'core scheduler must not own thread-goal background injection construction; use bitfun-agent-runtime scheduler',
      },
      {
        regex: /RoundInjectionKind::BackgroundResult/,
        message:
          'core scheduler must not own background result injection construction; use bitfun-agent-runtime scheduler',
      },
      {
        regex: /RoundInjectionTarget::CurrentRunningTurn/,
        message:
          'core scheduler must not own current-turn background injection targeting; use bitfun-agent-runtime scheduler',
      },
      {
        regex: /\bfn\s+turn_outcome_kind\s*\(/,
        message:
          'core scheduler must not own turn-outcome event facts; use bitfun-agent-runtime events',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/framework.rs',
    patterns: [
      {
        regex: /\bpub struct DynamicMcpToolInfo\b/,
        message: 'core tool framework must not redefine DynamicMcpToolInfo; use bitfun-agent-tools',
      },
      {
        regex: /\bpub struct DynamicToolInfo\b/,
        message: 'core tool framework must not redefine DynamicToolInfo; use bitfun-agent-tools',
      },
      {
        regex: /\bpub struct ToolRenderOptions\b/,
        message: 'core tool framework must not redefine ToolRenderOptions; use bitfun-agent-tools',
      },
      {
        regex: /\bpub enum ToolPathBackend\b/,
        message: 'core tool framework must not redefine ToolPathBackend; use bitfun-agent-tools',
      },
      {
        regex: /\bpub struct ToolPathResolution\b/,
        message: 'core tool framework must not redefine ToolPathResolution; use bitfun-agent-tools',
      },
      {
        regex: /\bpub struct ToolContextFacts\b/,
        message: 'core tool framework must not redefine ToolContextFacts; use bitfun-agent-tools',
      },
      {
        regex: /\bpub enum ToolWorkspaceKind\b/,
        message: 'core tool framework must not redefine ToolWorkspaceKind; use bitfun-agent-tools',
      },
      {
        regex: /\bpub struct ToolUseContext\b/,
        message:
          'core tool framework must not own ToolUseContext; re-export it from tool_context_runtime',
      },
      {
        regex: /\bcall_with_tool_runtime_hooks\b/,
        message:
          'core tool framework must not wire runtime hooks directly; delegate through tool_context_runtime',
      },
      {
        regex: /\bdeep_review_shared_context_measurement_snapshot\b/,
        message:
          'core tool framework must not own runtime hook regressions; keep them in tool_context_runtime',
      },
      {
        regex: /\bget_global_coordinator\b/,
        message:
          'core tool framework must not own runtime checkpoint coordination; keep it in tool_context_runtime',
      },
      {
        regex: /\bGitService\b/,
        message:
          'core tool framework must not own git-backed checkpoint runtime; keep it in tool_context_runtime',
      },
      {
        regex: /\bget_workspace_runtime_service_arc\b/,
        message:
          'core tool framework must not own workspace runtime lookup; keep it in tool_context_runtime',
      },
      {
        regex: /\bremote_workspace_runtime_root\b/,
        message:
          'core tool framework must not own remote runtime-root lookup; keep it in tool_context_runtime',
      },
      {
        regex: /\bget_path_manager_arc\b/,
        message:
          'core tool framework must not own host runtime-root lookup; keep it in tool_context_runtime',
      },
      {
        regex: /\bpost_call_hooks::record_successful_tool_call\b/,
        message:
          'core tool framework must not own post-call runtime hooks; keep them in tool_context_runtime',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/pipeline/tool_pipeline.rs',
    patterns: [
      {
        regex: /framework::(?:\{[^}]*\bToolUseContext\b[^}]*\}|\bToolUseContext\b)/,
        message:
          'tool pipeline must import ToolUseContext from tool_context_runtime, not the framework re-export',
      },
      {
        regex: /\bfn serialize_result_for_assistant\b/,
        message:
          'core tool pipeline must not own provider-neutral assistant result rendering; use bitfun-agent-tools',
      },
      {
        regex: /\bconst TOOL_ERROR_ARGUMENTS_PREVIEW_BYTES\b/,
        message:
          'core tool pipeline must not own tool error argument preview limits; use bitfun-agent-tools',
      },
      {
        regex: /\bfn truncate_arguments_preview\b/,
        message:
          'core tool pipeline must not own tool error argument preview rendering; use bitfun-agent-tools',
      },
      {
        regex: /\bfn truncate_raw_arguments_preview\b/,
        message:
          'core tool pipeline must not own raw tool argument preview rendering; use bitfun-agent-tools',
      },
      {
        regex: /\bconst USER_STEERING_INTERRUPTED_MESSAGE\b/,
        message:
          'core tool pipeline must not own steering-interrupted result presentation; use bitfun-agent-tools',
      },
      {
        regex: /\bfn build_truncation_recovery_notice\b/,
        message:
          'core tool pipeline must not own truncation recovery notice policy; use bitfun-agent-tools',
      },
      {
        regex: /\bfn is_write_like_tool_name\b/,
        message:
          'core tool pipeline must not own write-like truncation classification; use bitfun-agent-tools',
      },
      {
        regex: /\bstruct\s+ToolBatch\b/,
        message:
          'core tool pipeline must not own portable batching DTOs; use tool-runtime::pipeline',
      },
      {
        regex: /\bfn\s+partition_tool_batches\b/,
        message:
          'core tool pipeline must not own portable batching strategy; use tool-runtime::pipeline',
      },
      {
        regex: /Duration::from_millis\(100\s*\*\s*attempts/,
        message:
          'core tool pipeline must not own retry backoff policy; use tool-runtime::pipeline',
      },
      {
        regex: /\blet\s+mut\s+cancelled_count\b/,
        message:
          'core tool pipeline must not own dialog-turn cancellation summary policy; use tool-runtime::pipeline',
      },
      {
        regex: /ToolConfirmationOutcome::(?:Rejected|ChannelClosed|Timeout)/,
        message:
          'core tool pipeline must not own confirmation wait-result mapping; use bitfun-agent-runtime',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/pipeline/state_manager.rs',
    patterns: [
      {
        regex: /\bstats\.(?:queued|waiting|running|streaming|awaiting_confirmation|completed|failed|cancelled)\s*\+=/,
        message:
          'core tool state manager must not own provider-neutral state counting; use tool-runtime::pipeline',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/tool_context_runtime.rs',
    patterns: [
      {
        regex: /remote_workspace_git_metadata_unavailable|workspace_unavailable|git_status_unavailable:/,
        message:
          'core tool context must not own light-checkpoint summary policy; use bitfun-agent-runtime::checkpoint',
      },
      {
        regex: /format!\(\s*"staged=\{\}, unstaged=\{\}, untracked=\{\}"/,
        message:
          'core tool context must not own local git dirty-state checkpoint formatting; use bitfun-agent-runtime::checkpoint',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/agents/definitions/custom/subagent.rs',
    patterns: [
      {
        regex: /\bFrontMatterMarkdown\b/,
        message:
          'core custom subagent facade must not own markdown front-matter IO; use bitfun-agent-runtime',
      },
      {
        regex: /\bserde_yaml::Mapping\b/,
        message:
          'core custom subagent facade must not own markdown metadata serialization; use bitfun-agent-runtime',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/agents/registry/custom.rs',
    patterns: [
      {
        regex: /\bCustomSubagentLoader\b/,
        message:
          'core custom subagent registry must not restore the old loader owner; use bitfun-agent-runtime discovery report',
      },
      {
        regex: /\bread_dir\b/,
        message:
          'core custom subagent registry must not own directory scanning; use bitfun-agent-runtime discovery report',
      },
      {
        regex: /\.extension\(\)\.is_some_and\(\|ext\|\s*ext\s*==\s*"md"\)/,
        message:
          'core custom subagent registry must not own markdown file discovery; use bitfun-agent-runtime discovery report',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/subagent_runtime/mod.rs',
    patterns: [
      {
        regex: /\bstruct\s+DelegationPolicy\b/,
        message:
          'core subagent runtime must not redefine DelegationPolicy; use bitfun-runtime-ports',
      },
      {
        regex: /\benum\s+SubagentContextMode\b/,
        message:
          'core subagent runtime must not redefine SubagentContextMode; use bitfun-runtime-ports',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/coordination/coordinator.rs',
    patterns: [
      {
        regex: /\benum\s+DialogTriggerSource\b/,
        message:
          'core coordinator must not redefine DialogTriggerSource; use bitfun-runtime-ports',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/coordination/scheduler.rs',
    patterns: [
      {
        regex: /\benum\s+DialogQueuePriority\b/,
        message:
          'core scheduler must not redefine DialogQueuePriority; use bitfun-runtime-ports',
      },
      {
        regex: /\bstruct\s+DialogSubmissionPolicy\b/,
        message:
          'core scheduler must not redefine DialogSubmissionPolicy; use bitfun-runtime-ports',
      },
      {
        regex: /\benum\s+DialogSubmitOutcome\b/,
        message:
          'core scheduler must not redefine DialogSubmitOutcome; use bitfun-runtime-ports',
      },
      {
        regex: /\bstruct\s+AgentSessionReplyRoute\b/,
        message:
          'core scheduler must not redefine AgentSessionReplyRoute; use bitfun-runtime-ports',
      },
      {
        regex: /\benum\s+DialogSteerOutcome\b/,
        message:
          'core scheduler must not redefine DialogSteerOutcome; use bitfun-runtime-ports',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/round_preempt.rs',
    patterns: [
      {
        regex: /\btrait\s+DialogRoundPreemptSource\b/,
        message:
          'core round preempt runtime must not redefine DialogRoundPreemptSource; use bitfun-runtime-ports',
      },
      {
        regex: /\bstruct\s+RoundInjection\b/,
        message:
          'core round preempt runtime must not redefine RoundInjection; use bitfun-runtime-ports',
      },
      {
        regex: /\btrait\s+DialogRoundInjectionSource\b/,
        message:
          'core round preempt runtime must not redefine DialogRoundInjectionSource; use bitfun-runtime-ports',
      },
      {
        regex: /\benum\s+RoundInjectionKind\b/,
        message:
          'core round preempt runtime must not redefine RoundInjectionKind; use bitfun-runtime-ports',
      },
      {
        regex: /\benum\s+RoundInjectionTarget\b/,
        message:
          'core round preempt runtime must not redefine RoundInjectionTarget; use bitfun-runtime-ports',
      },
      {
        regex: /\bpub\s+struct\s+SessionRoundInjectionBuffer\b/,
        message:
          'core round preempt runtime must not own round injection buffer; use bitfun-agent-runtime',
      },
      {
        regex: /\bpub\s+struct\s+SessionRoundYieldFlags\b/,
        message:
          'core round preempt runtime must not own round yield flags; use bitfun-agent-runtime',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/goal_mode/mod.rs',
    patterns: [
      {
        regex: /\bconst\s+GOAL_MODE_METADATA_KEY\b/,
        message: 'core goal mode types must not redefine GOAL_MODE_METADATA_KEY; use bitfun-runtime-ports',
      },
      {
        regex: /\bconst\s+MAX_GOAL_CONTINUATIONS\b/,
        message: 'core goal mode types must not redefine MAX_GOAL_CONTINUATIONS; use bitfun-runtime-ports',
      },
      {
        regex: /\bconst\s+MAX_CONTEXT_SUMMARY_CHARS\b/,
        message: 'core goal mode types must not redefine MAX_CONTEXT_SUMMARY_CHARS; use bitfun-runtime-ports',
      },
      {
        regex: /\bstruct\s+ThreadGoal\b/,
        message: 'core goal mode types must not redefine ThreadGoal; use bitfun-runtime-ports',
      },
      {
        regex: /\benum\s+ThreadGoalStatus\b/,
        message: 'core goal mode types must not redefine ThreadGoalStatus; use bitfun-runtime-ports',
      },
      {
        regex: /\bstruct\s+GoalGenerationResult\b/,
        message: 'core goal mode types must not redefine GoalGenerationResult; use bitfun-runtime-ports',
      },
      {
        regex: /\bstruct\s+ThreadGoalToolResponse\b/,
        message: 'core goal mode types must not redefine ThreadGoalToolResponse; use bitfun-runtime-ports',
      },
      {
        regex: /\bstruct\s+GoalActivationResult\b/,
        message: 'core goal mode types must not redefine GoalActivationResult; use bitfun-runtime-ports',
      },
      {
        regex: /\bstruct\s+GoalContinuationPlan\b/,
        message: 'core goal mode types must not redefine GoalContinuationPlan; use bitfun-runtime-ports',
      },
      {
        regex: /\bpub\s+struct\s+ThreadGoalRuntime\b/,
        message:
          'core goal mode must not own thread goal runtime accounting; use bitfun-agent-runtime',
      },
      {
        regex: /\bfn\s+build_thread_goal_continuation_plan\b/,
        message:
          'core goal mode must not own thread goal continuation planning; use bitfun-agent-runtime',
      },
      {
        regex: /\bfn\s+goal_tool_response\b/,
        message:
          'core goal mode must not own thread goal tool response assembly; use bitfun-agent-runtime',
      },
      {
        regex: /\bfn\s+billable_tokens_from_counts\b/,
        message:
          'core goal mode must not own thread goal token accounting policy; use bitfun-agent-runtime',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/core/message.rs',
    patterns: [
      {
        regex: /\bstruct\s+CompressionContract\b/,
        message: 'core message model must not redefine CompressionContract; use bitfun-runtime-ports',
      },
      {
        regex: /\bstruct\s+CompressionContractItem\b/,
        message: 'core message model must not redefine CompressionContractItem; use bitfun-runtime-ports',
      },
      {
        regex: /\bfn\s+render_contract_items\b/,
        message: 'core message model must not own compression contract rendering; use bitfun-runtime-ports',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/workspace/manager.rs',
    patterns: [
      {
        regex: /\bstruct\s+RelatedPath\b/,
        message: 'core workspace manager must not redefine RelatedPath; use bitfun-runtime-ports',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/file_read_state_runtime.rs',
    patterns: [
      {
        regex: /framework::(?:\{[^}]*\bToolUseContext\b[^}]*\}|\bToolUseContext\b)/,
        message:
          'file read-state runtime must import ToolUseContext from tool_context_runtime, not the framework re-export',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/tool_result_storage.rs',
    patterns: [
      {
        regex: /framework::(?:\{[^}]*\bToolUseContext\b[^}]*\}|\bToolUseContext\b)/,
        message:
          'tool-result storage runtime must import ToolUseContext from tool_context_runtime, not the framework re-export',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/post_call_hooks.rs',
    patterns: [
      {
        regex: /framework::(?:\{[^}]*\bToolUseContext\b[^}]*\}|\bToolUseContext\b)/,
        message:
          'post-call hooks must import ToolUseContext from tool_context_runtime, not the framework re-export',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/tool_adapter.rs',
    patterns: [
      {
        regex: /framework::(?:\{[^}]*\bToolUseContext\b[^}]*\}|\bToolUseContext\b)/,
        message:
          'tool adapter must import ToolUseContext from tool_context_runtime, not the framework re-export',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/product_runtime.rs',
    patterns: [
      {
        regex: /framework::(?:\{[^}]*\bToolUseContext\b[^}]*\}|\bToolUseContext\b)/,
        message:
          'product tool runtime must import ToolUseContext from tool_context_runtime, not the framework re-export',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/product_runtime/catalog.rs',
    patterns: [
      {
        regex: /framework::(?:\{[^}]*\bToolUseContext\b[^}]*\}|\bToolUseContext\b)/,
        message:
          'product tool runtime catalog must import ToolUseContext from tool_context_runtime, not the framework re-export',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/product_runtime/get_tool_spec_tool.rs',
    patterns: [
      {
        regex: /framework::(?:\{[^}]*\bToolUseContext\b[^}]*\}|\bToolUseContext\b)/,
        message:
          'GetToolSpec adapter must import ToolUseContext from tool_context_runtime, not the framework re-export',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/manifest_resolver.rs',
    patterns: [
      {
        regex: /framework::(?:\{[^}]*\bToolUseContext\b[^}]*\}|\bToolUseContext\b)/,
        message:
          'manifest resolver must import ToolUseContext from tool_context_runtime, not the framework re-export',
      },
      {
        regex: /\bContextualToolManifest\b/,
        message:
          'manifest resolver must stay a compatibility facade; contextual manifest conversion belongs in product_runtime/catalog',
      },
      {
        regex: /\bContextualVisibleTools\b/,
        message:
          'manifest resolver must stay a compatibility facade; contextual visible-tool conversion belongs in product_runtime/catalog',
      },
      {
        regex: /\bToolManifestDefinition\b/,
        message:
          'manifest resolver must not own manifest DTO conversion; use product_runtime/catalog',
      },
      {
        regex: /\bfn\s+to_core_tool_definition\b/,
        message:
          'manifest resolver must not own ToolDefinition conversion; use product_runtime/catalog',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/workspace.rs',
    patterns: [
      {
        regex: /\bpub\s+trait\s+WorkspaceFileSystem\b/,
        message:
          'workspace file-system contract must be owned by bitfun-runtime-ports; keep only concrete adapters or re-exports in core',
      },
      {
        regex: /\bpub\s+trait\s+WorkspaceShell\b/,
        message:
          'workspace shell contract must be owned by bitfun-runtime-ports; keep only concrete adapters or re-exports in core',
      },
      {
        regex: /\bpub\s+struct\s+WorkspaceServices\b/,
        message:
          'workspace service bundle contract must be owned by bitfun-runtime-ports; keep only concrete adapters or re-exports in core',
      },
      {
        regex: /\bpub\s+struct\s+WorkspaceCommandOptions\b/,
        message:
          'workspace command contract must be owned by bitfun-runtime-ports; keep only concrete adapters or re-exports in core',
      },
      {
        regex: /\bpub\s+struct\s+WorkspaceCommandResult\b/,
        message:
          'workspace command result contract must be owned by bitfun-runtime-ports; keep only concrete adapters or re-exports in core',
      },
      {
        regex: /\bpub\s+struct\s+WorkspaceDirEntry\b/,
        message:
          'workspace directory entry contract must be owned by bitfun-runtime-ports; keep only concrete adapters or re-exports in core',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/execution/execution_engine.rs',
    patterns: [
      {
        regex: /\bGetToolSpecLoadObservation\b/,
        message:
          'execution engine must not own collapsed-tool unlock observation details; use product_runtime unlock state owner',
      },
      {
        regex: /\bcollect_loaded_collapsed_tool_names\b/,
        message:
          'execution engine must not call generic collapsed-tool collector directly; use product_runtime unlock state owner',
      },
      {
        regex: /\bfn\s+collect_unlocked_collapsed_tools\b/,
        message:
          'execution engine must not own collapsed-tool unlock collection; use product_runtime unlock state owner',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/miniapp/manager.rs',
    patterns: [
      {
        regex: /\bbuild_runtime_state\b/,
        message:
          'core MiniApp manager must not build runtime state directly; use product-domain lifecycle helpers',
      },
      {
        regex: /\bbuild_source_revision\b/,
        message:
          'core MiniApp manager must not build source revisions directly; use product-domain lifecycle helpers',
      },
      {
        regex: /\bbuild_deps_revision\b/,
        message:
          'core MiniApp manager must not build dependency revisions directly; use product-domain lifecycle helpers',
      },
      {
        regex: /\bapp\.version\s*\+=\s*1\b/,
        message:
          'core MiniApp manager must not own version increments for lifecycle transitions; use product-domain lifecycle helpers',
      },
      {
        regex: /\bapp\.runtime\s*=/,
        message:
          'core MiniApp manager must not own runtime-state replacement for lifecycle transitions; use product-domain lifecycle helpers',
      },
      {
        regex: /\bbuild_created_app\b/,
        message:
          'core MiniApp manager must not own create workflow assembly; use MiniAppRuntimeFacade',
      },
      {
        regex: /\bapply_update_patch\b/,
        message:
          'core MiniApp manager must not own update workflow assembly; use MiniAppRuntimeFacade',
      },
      {
        regex: /\bprepare_draft_app\b/,
        message:
          'core MiniApp manager must not own draft creation workflow assembly; use MiniAppRuntimeFacade',
      },
      {
        regex: /\bapply_draft_source_sync_result\b/,
        message:
          'core MiniApp manager must not own draft source-sync workflow assembly; use MiniAppRuntimeFacade',
      },
      {
        regex: /\bapply_draft_permission_update_result\b/,
        message:
          'core MiniApp manager must not own draft permission workflow assembly; use MiniAppRuntimeFacade',
      },
      {
        regex: /\bapply_draft_to_active\b/,
        message:
          'core MiniApp manager must not own apply-draft workflow assembly; use MiniAppRuntimeFacade',
      },
      {
        regex: /\bapply_draft_customization_metadata\b/,
        message:
          'core MiniApp manager must not own draft customization workflow assembly; use MiniAppRuntimeFacade',
      },
      {
        regex: /\bmark_builtin_update_available_metadata\b/,
        message:
          'core MiniApp manager must not own built-in update workflow assembly; use MiniAppRuntimeFacade',
      },
      {
        regex: /\bdecline_builtin_update_metadata\b/,
        message:
          'core MiniApp manager must not own built-in update decline workflow assembly; use MiniAppRuntimeFacade',
      },
      {
        regex: /\bprepare_imported_meta\b/,
        message:
          'core MiniApp manager must not own import metadata rehome planning; use product-domain import bundle plan',
      },
      {
        regex: /\bbuild_import_fallbacks\b/,
        message:
          'core MiniApp manager must not own import fallback planning; use product-domain import bundle plan',
      },
      {
        regex: /\bbuild_import_bundle_plan\b/,
        message:
          'core MiniApp manager must not own import bundle planning; use MiniAppRuntimeFacade',
      },
      {
        regex: /\bread_import_meta_json\b/,
        message:
          'core MiniApp manager must not own import metadata IO; use MiniAppRuntimeFacade import ports',
      },
      {
        regex: /\bwrite_import_bundle\b/,
        message:
          'core MiniApp manager must not own import bundle IO; use MiniAppRuntimeFacade import ports',
      },
      {
        regex: /\bworkspace_dir_string\b/,
        message:
          'core MiniApp manager must not own compile workspace path adaptation; use MiniAppCompileRequest',
      },
      {
        regex: /\bresolve_policy\s*\(/,
        message:
          'core MiniApp manager must not own permission policy path adaptation; use MiniAppPermissionPolicyRequest',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/restrictions.rs',
    patterns: [
      {
        regex: /\bpub enum ToolPathOperation\b/,
        message: 'core tool restrictions must not redefine ToolPathOperation; use bitfun-agent-tools',
      },
      {
        regex: /\bpub struct ToolPathPolicy\b/,
        message: 'core tool restrictions must not redefine ToolPathPolicy; use bitfun-agent-tools',
      },
      {
        regex: /\bpub struct ToolRuntimeRestrictions\b/,
        message:
          'core tool restrictions must not redefine ToolRuntimeRestrictions; use bitfun-agent-tools',
      },
      {
        regex: /\bfn\s+normalize_absolute_posix_path\b/,
        message:
          'core tool restrictions must not redefine remote POSIX path normalization; use bitfun-agent-tools',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/workspace_paths.rs',
    patterns: [
      {
        regex: /\bpub const BITFUN_RUNTIME_URI_PREFIX\b/,
        message:
          'core workspace path facade must not redefine the runtime URI prefix; use bitfun-agent-tools',
      },
      {
        regex: /\bpub struct ParsedBitFunRuntimeUri\b/,
        message:
          'core workspace path facade must not redefine ParsedBitFunRuntimeUri; use bitfun-agent-tools',
      },
      {
        regex: /\bfn\s+posix_normalize_components\b/,
        message:
          'core workspace path facade must not redefine remote POSIX path normalization; use bitfun-agent-tools',
      },
      {
        regex: /Component::ParentDir/,
        message:
          'core workspace path facade must not redefine host path normalization; use bitfun-agent-tools',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/registry.rs',
    patterns: [
      {
        regex: /\bstruct DynamicToolMetadata\b/,
        message:
          'core tool registry must not own dynamic tool metadata storage; use bitfun-agent-tools ToolRegistry',
      },
      {
        regex: /\btools\s*:\s*IndexMap\b/,
        message:
          'core tool registry must not own the generic tool map; use bitfun-agent-tools ToolRegistry',
      },
      {
        regex: /\bdynamic_tools\s*:\s*IndexMap\b/,
        message:
          'core tool registry must not own the dynamic tool map; use bitfun-agent-tools ToolRegistry',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/file_read_state_runtime.rs',
    patterns: [
      {
        regex: /\bnormalize_string\b/,
        message:
          'core file read-state runtime must delegate pure freshness normalization to bitfun-agent-tools',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/tool_result_storage.rs',
    patterns: [
      {
        regex: /\bfn\s+generate_preview\b/,
        message:
          'core tool result storage must delegate pure preview generation to bitfun-agent-tools',
      },
      {
        regex: /\bfn\s+build_persisted_output_message\b/,
        message:
          'core tool result storage must delegate persisted-output rendering to bitfun-agent-tools',
      },
      {
        regex: /\bfn\s+select_candidates_to_persist\b/,
        message:
          'core tool result storage must delegate round-budget selection to bitfun-agent-tools',
      },
      {
        regex: /\bstruct\s+ToolResultStoragePolicy\b/,
        message:
          'core tool result storage must use the provider-neutral storage policy from bitfun-agent-tools',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/mcp/server/process.rs',
    patterns: [
      {
        regex: /\bpub enum MCPServerType\b/,
        message: 'core MCP server process runtime must not redefine MCPServerType; use the integrations contract',
      },
      {
        regex: /\bpub enum MCPServerStatus\b/,
        message: 'core MCP server process runtime must not redefine MCPServerStatus; use the integrations contract',
      },
      {
        regex: /\bfn is_auth_error\b/,
        message: 'core MCP server process runtime must not own auth error classification; use the integrations helper',
      },
      {
        regex: /\bconst AUTHORIZATION_KEYS\b/,
        message: 'core MCP server process runtime must not own remote authorization key constants; use the integrations helper',
      },
      {
        regex: /\bcontains_key\("Authorization"\)/,
        message: 'core MCP server process runtime must not inline legacy authorization header fallback; use the integrations helper',
      },
      {
        regex: /\bprocess_manager::create_tokio_command\b/,
        message: 'core MCP server process facade must not spawn MCP child processes; use the integrations owner crate',
      },
      {
        regex: /\bMCPTransport::start_receive_loop\b/,
        message: 'core MCP server process facade must not own stdio receive lifecycle; use the integrations owner crate',
      },
      {
        regex: /\bMCPConnection::new_remote\b/,
        message: 'core MCP server process facade must not own remote transport lifecycle; use the integrations owner crate',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/mcp/server/manager/mod.rs',
    patterns: [
      {
        regex: /\benum ListChangedKind\b/,
        message: 'core MCP server manager must not own list-changed classification; use the integrations helper',
      },
      {
        regex: /\bresource_catalog_cache\b/,
        message: 'core MCP server manager must not own resource catalog cache state; use the integrations owner crate',
      },
      {
        regex: /\bprompt_catalog_cache\b/,
        message: 'core MCP server manager must not own prompt catalog cache state; use the integrations owner crate',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/mcp/server/manager/reconnect.rs',
    patterns: [
      {
        regex: /\bfn compute_backoff_delay\b/,
        message: 'core MCP reconnect runtime must not own backoff policy math; use the integrations helper',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/mcp/server/manager/interaction.rs',
    patterns: [
      {
        regex: /\bfn detect_list_changed_kind\b/,
        message: 'core MCP interaction runtime must not own list-changed classification; use the integrations helper',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/mcp/adapter/tool.rs',
    patterns: [
      {
        regex: /\bfn behavior_hints\b/,
        message: 'core MCP tool adapter must not own dynamic tool behavior hint rendering; use the integrations helper',
      },
      {
        regex: /\bfn truncate_for_assistant\b/,
        message: 'core MCP tool adapter must not own result truncation rendering; use the integrations helper',
      },
      {
        regex: /\bMCPToolResultContent\b/,
        message: 'core MCP tool adapter must not own MCP result content rendering; use the integrations helper',
      },
      {
        regex: /Tool '\{\}' from MCP server/,
        message: 'core MCP tool adapter must not own dynamic descriptor text; use the integrations helper',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/mcp/adapter/context.rs',
    patterns: [
      {
        regex: /\bpub struct ContextEnhancerConfig\b/,
        message: 'core MCP context provider must not own enhancer config; use the integrations helper',
      },
      {
        regex: /\bpub struct ContextEnhancer\b/,
        message: 'core MCP context provider must not own resource selection logic; use the integrations helper',
      },
      {
        regex: /\bpartial_cmp\b/,
        message: 'core MCP context provider must not own resource ranking logic; use the integrations helper',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/function_agents/git-func-agent/commit_generator.rs',
    patterns: [
      {
        regex: /\bGitService::get_status\b/,
        message:
          'Git function-agent commit generator must use CoreProductDomainRuntime for Git adapter wiring',
      },
      {
        regex: /\bAIAnalysisService::new_with_agent_config\b/,
        message:
          'Git function-agent commit generator must use CoreProductDomainRuntime for AI adapter wiring',
      },
      {
        regex: /\bto_string_lossy\b/,
        message:
          'Git function-agent commit generator must preserve PathBuf paths when routing through the facade',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/function_agents/startchat-func-agent/work_state_analyzer.rs',
    patterns: [
      {
        regex: /\bAIWorkStateService::new_with_agent_config\b/,
        message:
          'Startchat work-state analyzer must use CoreProductDomainRuntime for AI adapter wiring',
      },
      {
        regex: /\bcreate_command\("git"\)/,
        message:
          'Startchat work-state analyzer must use CoreProductDomainRuntime for Git adapter wiring',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/mcp/server/config.rs',
    patterns: [
      {
        regex: /\bpub enum MCPServerTransport\b/,
        message: 'core MCP server config facade must not redefine MCPServerTransport; use the integrations contract',
      },
      {
        regex: /\bpub struct MCPServerOAuthConfig\b/,
        message: 'core MCP server config facade must not redefine OAuth config; use the integrations contract',
      },
      {
        regex: /\bpub struct MCPServerXaaConfig\b/,
        message: 'core MCP server config facade must not redefine XAA config; use the integrations contract',
      },
      {
        regex: /\bpub struct MCPServerConfig\b/,
        message: 'core MCP server config facade must not redefine server config; use the integrations contract',
      },
      {
        regex: /\bfn default_true\b/,
        message: 'core MCP server config facade must not redefine config serde defaults; use the integrations contract',
      },
      {
        regex: /\bpub fn resolved_transport\b/,
        message: 'core MCP server config facade must not redefine transport defaults; use the integrations contract',
      },
      {
        regex: /\bpub fn validate\b/,
        message: 'core MCP server config facade must not redefine config validation; use the integrations contract',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/mcp/config/cursor_format.rs',
    patterns: [
      {
        regex: /\bfn parse_source\b/,
        message: 'core MCP cursor facade must not redefine source parsing; use the integrations contract',
      },
      {
        regex: /\bfn parse_transport\b/,
        message: 'core MCP cursor facade must not redefine transport parsing; use the integrations contract',
      },
      {
        regex: /\bfn parse_legacy_type\b/,
        message: 'core MCP cursor facade must not redefine legacy type parsing; use the integrations contract',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/mcp/config/json_config.rs',
    patterns: [
      {
        regex: /\bfn normalize_source\b/,
        message: 'core MCP JSON config facade must not redefine source normalization; use the integrations helper',
      },
      {
        regex: /\bfn normalize_transport\b/,
        message: 'core MCP JSON config facade must not redefine transport normalization; use the integrations helper',
      },
      {
        regex: /\bfn normalize_legacy_type\b/,
        message: 'core MCP JSON config facade must not redefine legacy type normalization; use the integrations helper',
      },
      {
        regex: /\bconfig_value\.get\("mcpServers"\)\.is_none\(\)/,
        message: 'core MCP JSON config facade must not inline save validation; use the integrations helper',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/mcp/config/service.rs',
    patterns: [
      {
        regex: /\bconst AUTHORIZATION_KEYS\b/,
        message: 'core MCP config service facade must not own authorization key constants; use the integrations helper',
      },
      {
        regex: /\bfn config_signature\b/,
        message: 'core MCP config service facade must not own merge signatures; use the integrations helper',
      },
      {
        regex: /\bfn precedence\b/,
        message: 'core MCP config service facade must not own merge precedence; use the integrations helper',
      },
      {
        regex: /\bfn config_authorization_from_map\b/,
        message: 'core MCP config service facade must not own authorization extraction; use the integrations helper',
      },
      {
        regex: /\bBTreeMap\b/,
        message: 'core MCP config service facade must not rebuild stable merge signatures; use the integrations helper',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/mcp/auth.rs',
    patterns: [
      {
        regex: /\bstruct VaultFile\b/,
        message: 'core MCP auth facade must not own OAuth vault storage; use the integrations owner crate',
      },
      {
        regex: /\bconst NONCE_LEN\b/,
        message: 'core MCP auth facade must not own OAuth vault encryption; use the integrations owner crate',
      },
      {
        regex: /\bfn encrypt_value\b/,
        message: 'core MCP auth facade must not own OAuth vault encryption; use the integrations owner crate',
      },
      {
        regex: /\bfn decrypt_value\b/,
        message: 'core MCP auth facade must not own OAuth vault encryption; use the integrations owner crate',
      },
      {
        regex: /\bAuthorizationManager::new\b/,
        message: 'core MCP auth facade must not assemble OAuth authorization manager internals; use the integrations owner crate',
      },
      {
        regex: /\bOAuthState::new\b/,
        message: 'core MCP auth facade must not assemble OAuth authorization state internals; use the integrations owner crate',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/mcp/protocol/transport_remote.rs',
    patterns: [
      {
        regex: /\bfn normalize_authorization_value\b/,
        message: 'core MCP remote transport must not redefine authorization normalization; use the integrations helper',
      },
      {
        regex: /starts_with\("bearer "\)/,
        message: 'core MCP remote transport must not inline bearer normalization; use the integrations helper',
      },
      {
        regex: /\bfn build_client_info\b/,
        message: 'core MCP remote transport must not own client capability construction; use the integrations helper',
      },
      {
        regex: /\bClientCapabilities::builder\b/,
        message: 'core MCP remote transport must not inline client capability construction; use the integrations helper',
      },
      {
        regex: /\bfn map_(?:rmcp_)?initialize_result\b/,
        message: 'core MCP remote transport must not own rmcp initialize mapping; use the integrations helper',
      },
      {
        regex: /\bfn map_(?:rmcp_)?tool\b/,
        message: 'core MCP remote transport must not own rmcp tool mapping; use the integrations helper',
      },
      {
        regex: /\bfn map_(?:rmcp_)?resource\b/,
        message: 'core MCP remote transport must not own rmcp resource mapping; use the integrations helper',
      },
      {
        regex: /\bfn map_(?:rmcp_)?resource_content\b/,
        message: 'core MCP remote transport must not own rmcp resource content mapping; use the integrations helper',
      },
      {
        regex: /\bfn map_(?:rmcp_)?prompt\b/,
        message: 'core MCP remote transport must not own rmcp prompt mapping; use the integrations helper',
      },
      {
        regex: /\bfn map_(?:rmcp_)?prompt_message\b/,
        message: 'core MCP remote transport must not own rmcp prompt message mapping; use the integrations helper',
      },
      {
        regex: /\bfn map_(?:rmcp_)?tool_result\b/,
        message: 'core MCP remote transport must not own rmcp tool result mapping; use the integrations helper',
      },
      {
        regex: /\bfn map_(?:rmcp_)?content_block\b/,
        message: 'core MCP remote transport must not own rmcp content block mapping; use the integrations helper',
      },
      {
        regex: /\bfn map_(?:rmcp_)?icons\b/,
        message: 'core MCP remote transport must not own rmcp icon mapping; use the integrations helper',
      },
      {
        regex: /\bfn map_(?:rmcp_)?annotations\b/,
        message: 'core MCP remote transport must not own rmcp annotation mapping; use the integrations helper',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/mcp/protocol/jsonrpc.rs',
    patterns: [
      {
        regex: /\bfn serialize_params\b/,
        message: 'core MCP jsonrpc facade must not redefine request parameter serialization; use the integrations contract',
      },
      {
        regex: /\bpub fn create_initialize_request\b/,
        message: 'core MCP jsonrpc facade must not redefine initialize request builders; use the integrations contract',
      },
      {
        regex: /\bpub fn create_resources_list_request\b/,
        message: 'core MCP jsonrpc facade must not redefine resources/list request builders; use the integrations contract',
      },
      {
        regex: /\bpub fn create_resources_read_request\b/,
        message: 'core MCP jsonrpc facade must not redefine resources/read request builders; use the integrations contract',
      },
      {
        regex: /\bpub fn create_prompts_list_request\b/,
        message: 'core MCP jsonrpc facade must not redefine prompts/list request builders; use the integrations contract',
      },
      {
        regex: /\bpub fn create_prompts_get_request\b/,
        message: 'core MCP jsonrpc facade must not redefine prompts/get request builders; use the integrations contract',
      },
      {
        regex: /\bpub fn create_tools_list_request\b/,
        message: 'core MCP jsonrpc facade must not redefine tools/list request builders; use the integrations contract',
      },
      {
        regex: /\bpub fn create_tools_call_request\b/,
        message: 'core MCP jsonrpc facade must not redefine tools/call request builders; use the integrations contract',
      },
      {
        regex: /\bpub fn create_ping_request\b/,
        message: 'core MCP jsonrpc facade must not redefine ping request builders; use the integrations contract',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/remote_ssh/workspace_state.rs',
    patterns: [
      {
        regex: /\bpub const LOCAL_WORKSPACE_SSH_HOST\b/,
        message: 'core remote SSH workspace runtime must not redefine LOCAL_WORKSPACE_SSH_HOST; use the integrations contract',
      },
      {
        regex: /\bpub fn normalize_remote_workspace_path\b/,
        message: 'core remote SSH workspace runtime must not redefine remote path normalization; use the integrations contract',
      },
      {
        regex: /\bpub fn sanitize_ssh_connection_id_for_local_dir\b/,
        message: 'core remote SSH workspace runtime must not redefine SSH connection id sanitization; use the integrations contract',
      },
      {
        regex: /\bpub fn sanitize_remote_mirror_path_component\b/,
        message: 'core remote SSH workspace runtime must not redefine remote mirror path sanitization; use the integrations contract',
      },
      {
        regex: /\bpub fn sanitize_ssh_hostname_for_mirror\b/,
        message: 'core remote SSH workspace runtime must not redefine SSH hostname mirror sanitization; use the integrations contract',
      },
      {
        regex: /\bpub fn remote_root_to_mirror_subpath\b/,
        message: 'core remote SSH workspace runtime must not redefine remote mirror subpath mapping; use the integrations contract',
      },
      {
        regex: /\bpub fn workspace_logical_key\b/,
        message: 'core remote SSH workspace runtime must not redefine workspace logical keys; use the integrations contract',
      },
      {
        regex: /\bpub fn local_workspace_stable_storage_id\b/,
        message: 'core remote SSH workspace runtime must not redefine local workspace stable ids; use the integrations contract',
      },
      {
        regex: /\bpub fn remote_workspace_stable_id\b/,
        message: 'core remote SSH workspace runtime must not redefine remote workspace stable ids; use the integrations contract',
      },
      {
        regex: /\bpub fn unresolved_remote_session_storage_key\b/,
        message: 'core remote SSH workspace runtime must not redefine unresolved session keys; use the integrations contract',
      },
      {
        regex: /\bstruct RegisteredRemoteWorkspace\b/,
        message: 'core remote SSH workspace runtime must not own workspace registrations; use the integrations registry',
      },
      {
        regex: /\bpub struct RemoteWorkspaceEntry\b/,
        message: 'core remote SSH workspace runtime must not redefine workspace entries; use the integrations registry',
      },
      {
        regex: /\bpub struct RemoteWorkspaceState\b/,
        message: 'core remote SSH workspace runtime must not redefine legacy workspace state; use the integrations registry',
      },
      {
        regex: /\bregistration_matches_path\b/,
        message: 'core remote SSH workspace runtime must not own path-to-registration matching; use the integrations registry',
      },
      {
        regex: /\bdunce::canonicalize\b/,
        message: 'core remote SSH workspace runtime must not own local root canonicalization; use the integrations path helper',
      },
      {
        regex: /\bfn path_buf_to_stable_local_root_string\b/,
        message: 'core remote SSH workspace runtime must not own local root string normalization; use the integrations path helper',
      },
      {
        regex: /join\("_unresolved"\)/,
        message: 'core remote SSH workspace runtime must not own unresolved session path layout; use the integrations path helper',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/remote_connect/remote_server.rs',
    patterns: [
      {
        regex: /\bpub\(crate\) struct CoreRemoteDialogRuntimeHost\b/,
        message:
          'remote_server must not own concrete remote dialog runtime host; keep it in service_agent_runtime',
      },
      {
        regex: /\bpub\(crate\) struct CoreRemoteCancelRuntimeHost\b/,
        message:
          'remote_server must not own concrete remote cancel runtime host; keep it in service_agent_runtime',
      },
      {
        regex: /\bpub\(crate\) struct CoreRemoteWorkspaceFileRuntimeHost\b/,
        message:
          'remote_server must not own concrete remote workspace file runtime host; keep it in service_agent_runtime',
      },
      {
        regex: /\bstruct CoreRemoteSessionTrackerHost\b/,
        message:
          'remote_server must not own concrete remote tracker host; keep it in service_agent_runtime',
      },
      {
        regex: /\basync fn resolve_session_model_id\b/,
        message:
          'remote_server must not own remote session model resolution; keep it in service_agent_runtime',
      },
      {
        regex: /\basync fn load_remote_model_catalog\b/,
        message:
          'remote_server must not own remote model catalog loading; keep it in service_agent_runtime',
      },
      {
        regex: /\bget_global_config_service\b/,
        message:
          'remote_server must not own remote model config access; route it through service_agent_runtime',
      },
      {
        regex: /\bfn compress_data_url_for_mobile\b/,
        message:
          'remote_server must not own remote chat thumbnail compression; keep it in service_agent_runtime',
      },
      {
        regex: /\bfn turns_to_chat_messages\b/,
        message:
          'remote_server must not own persisted turn to remote chat conversion; keep it in service_agent_runtime',
      },
      {
        regex: /\basync fn load_chat_messages_from_conversation_persistence\b/,
        message:
          'remote_server must not own remote chat history persistence loading; keep it in service_agent_runtime',
      },
      {
        regex: /\bfn strip_user_input_tags\b/,
        message:
          'remote_server must not own remote user input display cleanup; keep it in service_agent_runtime',
      },
      {
        regex: /\bpub struct ImageAttachment\b/,
        message: 'core remote-connect server must not redefine image attachment wire DTOs; use the integrations contract',
      },
      {
        regex: /\bpub struct ChatImageAttachment\b/,
        message: 'core remote-connect server must not redefine chat image wire DTOs; use the integrations contract',
      },
      {
        regex: /\bpub struct ChatMessage\b/,
        message: 'core remote-connect server must not redefine chat message wire DTOs; use the integrations contract',
      },
      {
        regex: /\bpub struct ChatMessageItem\b/,
        message: 'core remote-connect server must not redefine chat message item DTOs; use the integrations contract',
      },
      {
        regex: /\bpub struct RemoteToolStatus\b/,
        message: 'core remote-connect server must not redefine remote tool status DTOs; use the integrations contract',
      },
      {
        regex: /\bpub struct ActiveTurnSnapshot\b/,
        message: 'core remote-connect server must not redefine active turn snapshot DTOs; use the integrations contract',
      },
      {
        regex: /\bpub struct SessionInfo\b/,
        message: 'core remote-connect server must not redefine session info DTOs; use the integrations contract',
      },
      {
        regex: /\bpub struct RemoteDefaultModelsConfig\b/,
        message: 'core remote-connect server must not redefine remote model default DTOs; use the integrations contract',
      },
      {
        regex: /\bpub struct RemoteModelConfig\b/,
        message: 'core remote-connect server must not redefine remote model DTOs; use the integrations contract',
      },
      {
        regex: /\bpub struct RemoteModelCatalog\b/,
        message: 'core remote-connect server must not redefine remote model catalog DTOs; use the integrations contract',
      },
      {
        regex: /\bpub struct RemoteModelCatalogPollDelta\b/,
        message: 'core remote-connect server must not redefine remote model catalog poll delta; use the integrations contract',
      },
      {
        regex: /\bpub enum RemoteCommand\b/,
        message: 'core remote-connect server must not redefine remote command wire DTOs; use the integrations contract',
      },
      {
        regex: /\bpub enum RemoteResponse\b/,
        message: 'core remote-connect server must not redefine remote response wire DTOs; use the integrations contract',
      },
      {
        regex: /\bstruct TrackerState\b/,
        message: 'core remote-connect server must not own tracker state; use the integrations tracker',
      },
      {
        regex: /\bpub enum TrackerEvent\b/,
        message: 'core remote-connect server must not redefine tracker events; use the integrations tracker',
      },
      {
        regex: /\bpub struct RemoteSessionStateTracker\b/,
        message: 'core remote-connect server must not own tracker state; use the integrations tracker',
      },
      {
        regex: /\bDashMap\b/,
        message: 'core remote-connect server must not own tracker storage; use the integrations registry',
      },
      {
        regex: /\bfn make_slim_params\b/,
        message: 'core remote-connect server must not own remote tool preview slimming; use the integrations helper',
      },
      {
        regex: /\bmatch mobile_type\b/,
        message: 'core remote-connect server must not own remote agent type alias mapping; use the integrations helper',
      },
      {
        regex: /\bfn resolve_remote_cancel_decision\b/,
        message: 'core remote-connect server must not own cancel decision policy; use the integrations helper',
      },
      {
        regex: /\benum RemoteCancelDecision\b/,
        message: 'core remote-connect server must not own cancel decision types; use the integrations contract',
      },
      {
        regex: /\bstruct RemoteCancelTaskRequest\b/,
        message: 'core remote-connect server must not own cancel task request contracts; use the integrations contract',
      },
      {
        regex: /\btrait RemoteCancelRuntimeHost\b/,
        message: 'core remote-connect server must not own cancel runtime host contracts; use the integrations contract',
      },
      {
        regex: /\bfn cancel_remote_task\b/,
        message: 'core remote-connect server must not own cancel orchestration; use the integrations helper',
      },
      {
        regex: /\bfn remote_session_restore_target\b/,
        message: 'core remote-connect server must not own restore-target policy; use the integrations helper',
      },
      {
        regex: /\bfn resolve_remote_execution_image_contexts\b/,
        message: 'core remote-connect server must not own image-context preference policy; use the integrations helper',
      },
      {
        regex: /\btrait RemoteImageContextAdapter\b/,
        message: 'core remote-connect server must not own image-context adapter contracts; use the integrations contract',
      },
      {
        regex: /\bconst MAX_SIZE\b/,
        message: 'core remote-connect server must not own remote file max-read policy; use the integrations helper',
      },
      {
        regex: /\bconst MAX_CHUNK\b/,
        message: 'core remote-connect server must not own remote file chunk policy; use the integrations helper',
      },
      {
        regex: /unwrap_or\("file"\)/,
        message: 'core remote-connect server must not own remote file display-name fallback; use the integrations helper',
      },
      {
        regex: /\bresolve_workspace_path\b/,
        message: 'core remote-connect server must not own workspace file path resolution; use the integrations helper',
      },
      {
        regex: /\bdetect_mime_type\b/,
        message: 'core remote-connect server must not own workspace file MIME detection; use the integrations helper',
      },
      {
        regex: /\bread_workspace_file\b/,
        message: 'core remote-connect server must not own workspace file read helpers; use the integrations helper',
      },
      {
        regex: /\bfn read_remote_workspace_file\b/,
        message: 'core remote-connect server must not redefine remote workspace full-file readers; use the integrations helper',
      },
      {
        regex: /\bfn read_remote_workspace_file_chunk\b/,
        message: 'core remote-connect server must not redefine remote workspace chunk readers; use the integrations helper',
      },
      {
        regex: /\bfn read_remote_workspace_file_info\b/,
        message: 'core remote-connect server must not redefine remote workspace file-info readers; use the integrations helper',
      },
      {
        regex: /\bfn remote_file_content_response\b/,
        message: 'core remote-connect server must not own remote file content response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_file_chunk_response\b/,
        message: 'core remote-connect server must not own remote file chunk response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_file_info_response\b/,
        message: 'core remote-connect server must not own remote file-info response assembly; use the integrations helper',
      },
      {
        regex: /\bfn handle_remote_workspace_file_command\b/,
        message: 'core remote-connect server must not own remote file command orchestration; use the integrations helper',
      },
      {
        regex: /general_purpose::STANDARD\.encode/,
        message: 'core remote-connect server must not own remote response base64 wrapping; use the integrations helper',
      },
      {
        regex: /\bfn remote_dialog_submit_response\b/,
        message: 'core remote-connect server must not own remote dialog response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_task_cancel_response\b/,
        message: 'core remote-connect server must not own remote cancel response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_interaction_accepted_response\b/,
        message: 'core remote-connect server must not own remote interaction response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_answer_question_response\b/,
        message: 'core remote-connect server must not own remote answer response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_workspace_info_response\b/,
        message: 'core remote-connect server must not own workspace-info response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_recent_workspaces_response\b/,
        message: 'core remote-connect server must not own recent-workspaces response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_assistant_list_response\b/,
        message: 'core remote-connect server must not own assistant-list response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_workspace_updated_response\b/,
        message: 'core remote-connect server must not own workspace-updated response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_assistant_updated_response\b/,
        message: 'core remote-connect server must not own assistant-updated response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_session_info\b/,
        message: 'core remote-connect server must not own session response facts assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_session_list_response\b/,
        message: 'core remote-connect server must not own session-list response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_initial_sync_response\b/,
        message: 'core remote-connect server must not own initial-sync response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_session_created_response\b/,
        message: 'core remote-connect server must not own session-created response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_session_model_updated_response\b/,
        message: 'core remote-connect server must not own session-model response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_messages_response\b/,
        message: 'core remote-connect server must not own remote messages response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_session_deleted_response\b/,
        message: 'core remote-connect server must not own session-deleted response assembly; use the integrations helper',
      },
      {
        regex: /\bfn should_send_remote_model_catalog\b/,
        message: 'core remote-connect server must not own poll model-catalog policy; use the integrations helper',
      },
      {
        regex: /\bfn remote_model_catalog_poll_delta\b/,
        message: 'core remote-connect server must not own poll model-catalog delta policy; use the integrations helper',
      },
      {
        regex: /\bfn remote_no_change_poll_response\b/,
        message: 'core remote-connect server must not own no-change poll response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_snapshot_poll_response\b/,
        message: 'core remote-connect server must not own streaming poll response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_persisted_poll_response\b/,
        message: 'core remote-connect server must not own persisted poll response assembly; use the integrations helper',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/remote_connect/bot/mod.rs',
    patterns: [
      {
        regex: /\bfn strip_workspace_path_prefix\b/,
        message: 'core remote-connect bot facade must not own workspace path prefix stripping; use the integrations helper',
      },
      {
        regex: /\bfn is_absolute_workspace_path\b/,
        message: 'core remote-connect bot facade must not own workspace path absolute detection; use the integrations helper',
      },
      {
        regex: /\bmatch ext\.as_str\(\)/,
        message: 'core remote-connect bot facade must not own workspace file MIME mapping; use the integrations helper',
      },
      {
        regex: /\btokio::fs::read\(&abs_path\)/,
        message: 'core remote-connect bot facade must not own workspace file reads; use the integrations helper',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/service/announcement/state_store.rs',
    patterns: [
      {
        regex: /\btokio::fs\b/,
        message: 'core announcement state store facade must not own filesystem persistence; use the integrations state store',
      },
      {
        regex: /\bserde_json::to_string_pretty\b/,
        message: 'core announcement state store facade must not own state serialization; use the integrations state store',
      },
      {
        regex: /\bserde_json::from_str\b/,
        message: 'core announcement state store facade must not own state deserialization; use the integrations state store',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/implementations/bash_tool.rs',
    reason:
      'BashTool must stay as terminal/session/checkpoint glue and must not re-own reusable shell execution helpers',
    patterns: [
      {
        regex: /\bconst\s+MAX_OUTPUT_LENGTH\b/,
        message: 'Bash output rendering budget is owned by tool-runtime::shell',
      },
      {
        regex: /\bconst\s+BANNED_COMMANDS\b/,
        message: 'Bash banned-command policy is owned by tool-runtime::shell',
      },
      {
        regex: /\bfn\s+detect_osascript_keystroke_non_ascii\b/,
        message: 'Bash osascript keystroke guard is owned by tool-runtime::shell',
      },
      {
        regex: /\bfn\s+detect_osascript_im_app\b/,
        message: 'Bash IM AppleScript guard is owned by tool-runtime::shell',
      },
      {
        regex: /\bfn\s+truncate_output_preserving_tail\b/,
        message: 'Bash output truncation is owned by tool-runtime::shell',
      },
      {
        regex: /\bfn\s+command_for_working_directory\b/,
        message: 'Bash working-directory command wrapping is owned by tool-runtime::shell',
      },
      {
        regex: /\bfn\s+render_result\b/,
        message: 'Bash local result rendering is owned by tool-runtime::shell',
      },
      {
        regex: /\bfn\s+render_remote_result\b/,
        message: 'Bash remote result rendering is owned by tool-runtime::shell',
      },
      {
        regex: /\bfn\s+format_background_command_delivery_text\b/,
        message: 'Bash background-result delivery text is owned by tool-runtime::shell',
      },
      {
        regex: /\bfn\s+format_background_command_error_text\b/,
        message: 'Bash background-result error text is owned by tool-runtime::shell',
      },
    ],
  },
];

export const forbiddenContentUnderRules = [
  {
    path: 'src/crates/assembly/core/src',
    reason:
      'core must use runtime-ports as the owner path for portable subagent contracts',
    patterns: [
      {
        regex:
          /crate::agentic::subagent_runtime(?:::|\s*::|::\{)(?:[^;\n]*\b(?:DelegationPolicy|SubagentContextMode)\b)/,
        message:
          'DelegationPolicy and SubagentContextMode must be imported from bitfun-runtime-ports, not the core compatibility re-export',
      },
    ],
  },
  {
    path: 'src/crates/contracts/product-domains/src',
    reason:
      'product-domains must not own IO/process/Git/AI/platform runtime behavior without an approved port/provider migration',
    patterns: [
      {
        regex: /\bCommand::new\(/,
        allowPaths: ['src/crates/contracts/product-domains/src/miniapp/runtime.rs'],
        message:
          'product-domains must not spawn processes outside the reviewed MiniApp runtime detector owner',
      },
      {
        regex: /\bprocess_manager::/,
        message:
          'product-domains must not use the core process manager; keep process execution in core/adapters',
      },
      {
        regex: /\btokio::process::/,
        message: 'product-domains must not own async process execution',
      },
      {
        regex: /\btokio::fs::/,
        message:
          'product-domains must not own async storage IO; storage runtime belongs in services-integrations',
      },
      {
        regex: /\bGitService::/,
        message: 'product-domains must not call concrete Git services; use reviewed ports/adapters',
      },
      {
        regex: /\b(?:AIService|AiService)::/,
        message: 'product-domains must not call concrete AI services; use reviewed ports/adapters',
      },
      {
        regex: /\breqwest::/,
        message: 'product-domains must not own network clients',
      },
      {
        regex: /\bgit2::/,
        message: 'product-domains must not own libgit2 runtime integration',
      },
      {
        regex: /\brmcp::/,
        message: 'product-domains must not own MCP runtime integration',
      },
      {
        regex: /\btauri::/,
        message: 'product-domains must not depend on Tauri platform APIs',
      },
      {
        regex: /\bAppHandle\b/,
        message: 'product-domains must not own desktop platform handles',
      },
      {
        regex: /\bstd::net::/,
        message: 'product-domains must not own network sockets',
      },
      {
        regex: /\b(?:TcpStream|UdpSocket)\b/,
        message: 'product-domains must not own network sockets',
      },
    ],
  },
  {
    path: 'src/crates/execution/tool-contracts/src',
    reason:
      'agent-tools may own pure tool manifest contracts, but not product manifest runtime or GetToolSpec execution without an approved provider migration',
    patterns: [
      {
        regex: /\bGetToolSpecTool\b/,
        message: 'GetToolSpec implementation stays in core product tool runtime',
      },
      {
        regex: /\bmanifest_resolver\b/,
        message: 'tool manifest resolution stays in core product tool runtime',
      },
      {
        regex: /\bunlocked_collapsed_tools\b/,
        message: 'collapsed-tool unlock state stays in core ToolUseContext/runtime',
      },
      {
        regex: /\bToolUseContext\b/,
        message: 'ToolUseContext stays in core until a portable context port is reviewed',
      },
    ],
  },
  {
    path: 'src/crates/execution/tool-provider-groups/src',
    reason:
      'tool-packs may own provider group plans, but not product tool manifest/exposure or GetToolSpec runtime',
    patterns: [
      {
        regex: /\bGetToolSpecTool\b/,
        message: 'GetToolSpec implementation stays in core product tool runtime',
      },
      {
        regex: /\bGET_TOOL_SPEC_TOOL_NAME\b/,
        message: 'GetToolSpec manifest insertion stays in core product tool runtime',
      },
      {
        regex: /\bmanifest_resolver\b/,
        message: 'tool manifest resolution stays in core product tool runtime',
      },
      {
        regex: /\bunlocked_collapsed_tools\b/,
        message: 'collapsed-tool unlock state stays in core ToolUseContext/runtime',
      },
      {
        regex: /\bToolExposure\b/,
        message: 'expanded/collapsed exposure policy stays in core until provider migration',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/implementations',
    reason:
      'GetToolSpec concrete adapter belongs in the product tool runtime owner, not the generic concrete-tool implementations module',
    patterns: [
      {
        regex: /\bpub(?:\(crate\))? struct GetToolSpecTool\b/,
        message: 'move GetToolSpecTool into core product_runtime owner',
      },
    ],
  },
  {
    path: 'src/crates/assembly/core/src/agentic/tools/pipeline',
    reason:
      'core pipeline must delegate deterministic tool execution admission policy to bitfun-agent-tools',
    patterns: [
      {
        regex: /\bvalidate_tool_allowed_by_list\s*\(/,
        message: 'allowed-list admission must stay behind validate_tool_execution_admission',
      },
      {
        regex: /\bruntime_tool_restrictions\s*\.\s*ensure_tool_allowed\s*\(/,
        message: 'runtime-restriction admission must stay behind validate_tool_execution_admission',
      },
      {
        regex: /\bvalidate_collapsed_tool_usage\s*\(/,
        message: 'collapsed-tool admission must stay behind validate_tool_execution_admission',
      },
    ],
  },
];
