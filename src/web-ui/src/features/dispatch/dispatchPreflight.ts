export const DISPATCH_PROTOCOL_VERSION = 4;

/**
 * Capabilities every dispatch target must advertise.
 *
 * The Git-worktree entries are not feature-detected extras: there is no
 * snapshot delivery left to fall back to, so a target missing any of them
 * cannot run a dispatch at all.
 */
export const BASE_DISPATCH_CAPABILITIES = [
  'persistent_jobs',
  'cursor_events',
  'detached_worker',
  'frontend_event_projection',
  'workspace_serialization',
  'workspace_git_worktree',
  'workspace_git_bundle_upload',
  'workspace_git_sync',
  'dispatch_worker_cli_profile',
  'per_turn_options',
  'session_query',
  'inline_attachments',
] as const;
