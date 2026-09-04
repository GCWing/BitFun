export const DISPATCH_PROTOCOL_VERSION = 6;

/**
 * Capabilities every dispatch target must advertise.
 *
 * Mirrors `dispatch_required_target_capabilities()` in
 * `src/crates/services/services-core/src/dispatch_contract.rs`; the contract
 * test pins the two sets equal in both directions.
 *
 * The Git-worktree entries are not feature-detected extras: there is no
 * snapshot delivery left to fall back to, so a target missing any of them
 * cannot run a dispatch at all.
 */
export const BASE_DISPATCH_CAPABILITIES = [
  'product_identity',
  'persistent_jobs',
  'cursor_events',
  'detached_worker',
  'frontend_event_projection',
  'append_message',
  'event_log_completeness',
  'approval_auto',
  'approval_reject_and_report',
  'approval_remote',
  'workspace_serialization',
  'workspace_git_worktree',
  'workspace_git_bundle_upload',
  'workspace_git_sync',
  'dispatch_worker_cli_profile',
  'per_turn_options',
  'session_query',
  'inline_attachments',
  'reasoning_presets',
] as const;
