import { ITransportAdapter, type TransportRequestTiming } from './base';
import { TauriTransportAdapter } from './tauri-adapter';
import {
  SurfaceChangedError,
  getActiveSurfaceScope,
  isSurfaceChangedError,
  surfaceIdForDevice,
  type DeviceSurfaceId,
} from '@/infrastructure/peer-device/deviceSurface';
import { createLogger } from '@/shared/utils/logger';
import { elapsedMs, nowMs } from '@/shared/utils/timing';

const log = createLogger('PeerDeviceTransport');

/**
 * Commands that must always hit the local Tauri host, even in peer mode.
 * Keep aligned with desktop `peer_host_invoke::LOCAL_ONLY_COMMANDS` and CLI
 * `peer_host/deny.rs`. Account + cloud turn APIs stay on the controller;
 * peer history uses HostInvoke restore. See
 * `src/infrastructure/peer-device/README.md`.
 */
const LOCAL_ONLY_COMMANDS = new Set([
  'show_main_window',
  'hide_main_window_after_close_request',
  'quit_app',
  'minimize_to_tray',
  'initialize_tray_after_startup',
  'startup_window_control',
  'toggle_main_window_fullscreen',
  'set_main_window_transient_geometry',
  'get_prevent_sleep_enabled',
  'set_prevent_sleep_enabled',
  'restart_app',
  'check_for_updates',
  'install_update',
  'appearance_market_browse',
  'appearance_market_download_release',
  'appearance_market_get_listing',
  'appearance_market_get_review_submission',
  'appearance_market_list_review_submissions',
  'appearance_market_list_submissions',
  'appearance_market_review_submission',
  'appearance_market_submit_package',
  'appearance_market_withdraw_submission',
  'account_login',
  'account_finalize_login',
  'account_cancel_pending_login',
  'account_logout',
  'account_status',
  'account_get_credential_hint',
  'account_token_expired',
  'account_connect_devices',
  'account_online_devices',
  'account_list_devices',
  'account_delete_device',
  'account_device_rpc',
  'account_delegate_to_paired',
  'account_auto_sync',
  'account_sync_settings',
  'account_fetch_settings',
  'account_sync_session',
  'account_fetch_synced_sessions',
  'account_delete_synced_session',
  'account_export_local_session',
  'account_export_all_sessions',
  'account_import_remote_sessions',
  'account_fetch_session_turns',
  'account_send_session_to_device',
  'account_execute_on_device',
  'peer_host_invoke_complete',
  'peer_control_attach',
  'peer_control_detach',
  'peer_mode_ping',
  'peer_controller_set_active',
  // `computer_use_request_permissions` / `computer_use_open_system_settings`
  // are intentionally NOT local-only: they run on the host that runs the
  // Computer Use Tool, so Desktop Peer B surfaces B's own OS permission prompts
  // and settings panes. CLI Peer refuses them in deny.rs and the UI gates the
  // section on host type. See SessionConfig + peer_host_invoke + cli deny.rs.
  // Detached dispatch uses this controller's SSH credentials and observer index.
  'dispatch_list_targets',
  'dispatch_probe_target',
  'dispatch_install_cli_start',
  'dispatch_install_cli_poll',
  'dispatch_install_cli_cancel',
  'dispatch_provision_target',
  'dispatch_sync_model_config',
  'dispatch_submit',
  'dispatch_status',
  'dispatch_query',
  'dispatch_cancel',
  'dispatch_list_jobs',
  'dispatch_answer',
  'dispatch_append',
  'dispatch_continue',
  'dispatch_sync_result',
  'dispatch_load_transcript',
  'dispatch_save_transcript',
  'remote_connect_get_device_info',
  'remote_connect_get_lan_ip',
  'remote_connect_get_lan_network_info',
  'remote_connect_get_methods',
  'remote_connect_start',
  'remote_connect_stop',
  'remote_connect_stop_bot',
  'remote_connect_status',
  'remote_connect_get_form_state',
  'remote_connect_set_form_state',
  'remote_connect_configure_custom_server',
  'remote_connect_configure_bot',
  'remote_connect_weixin_qr_start',
  'remote_connect_weixin_qr_poll',
  'remote_connect_get_bot_verbose_mode',
  'remote_connect_set_bot_verbose_mode',
  // One-click relay deploy SSHes from the controller, never the peer host
  'relay_deploy_preflight',
  'relay_deploy_install_docker',
  'relay_deploy_start',
  'relay_deploy_poll',
  'relay_deploy_cancel',
  'relay_deploy_register',
  'relay_deploy_verify',
  'speech_list_models',
  'speech_download_model',
  'speech_cancel_model_download',
  'speech_delete_model',
  'speech_verify_model',
  'speech_start_input_session',
  'speech_append_audio_chunk',
  'speech_finish_input_session',
  'speech_cancel_input_session',
  // UI locale is controller app-shell state: it writes the controller's config
  // file, rebuilds THIS machine's macOS menubar/tray, and drives the UI the user
  // is looking at. Routing it to a peer both writes the wrong config and rebuilds
  // the wrong machine's chrome (CLI peer returns unsupported). See PR #2428.
  'i18n_get_current_language',
  'i18n_set_language',
  'i18n_get_supported_languages',
  'i18n_get_config',
  'i18n_set_config',
  // Announcement cards/scheduler are controller app-shell state. get_pending /
  // get_tips trigger the scheduler (mutate app_open_count + persist); seen /
  // dismiss / never-show write the controller's announcement state. CLI peer
  // returns unsupported. Keeping them LOCAL_ONLY also removes them from peer
  // read-retry (see SIDE_EFFECTING_GET_COMMANDS). See scheduler.rs run().
  'get_pending_announcements',
  'get_announcement_tips',
  'mark_announcement_seen',
  'dismiss_announcement',
  'never_show_announcement',
  'trigger_announcement',
  // Companion pets live on the controller's desktop. The import zip is picked by
  // a local dialog on A; its absolute path only exists on A. Peer B cannot read
  // it, and B's returned spritesheetPath is a B-absolute path A's plugin-fs
  // cannot open. CLI peer returns unsupported at dispatch. Keeping these
  // LOCAL_ONLY gates Peer Mode (invariant 10: download destinations stay on the
  // controller). See PR #2428.
  'list_agent_companion_pets',
  'import_agent_companion_pet_package',
  'delete_agent_companion_pet_package',
  // Insights is the controller's own usage report: it reads the controller's
  // session history and writes the HTML to the controller's user_data_dir. In
  // Peer Mode generate_insights would run on B but listenProgress listens on A,
  // openReport opens B's absolute path on A, and the 30s mutation timeout fires
  // while B keeps running. Keep it controller-local so report, progress event
  // and openPath all land on one machine. (Cross-device insights tracking is
  // out of scope; this LOCAL gate is the reviewer-asked fix.) See PR #2428.
  'generate_insights',
  'get_latest_insights',
  'load_insights_report',
  'has_insights_data',
  'cancel_insights_generation',
  // IDE control events drive THIS window's panels (window.dispatchEvent). The
  // listen is local; the result report must use the same transport on both
  // success and error branches so a request never splits across hosts. CLI peer
  // returns unsupported. See PR #2428.
  'report_ide_control_result',
  // Controller app-shell / local-device commands reached by migrating dynamic
  // invoke() sites behind the adapter fence. These operate on the controller's
  // OWN surfaces (embedded webview, DevTools, desktop pet, diagnostics) and a
  // peer host has no implementation for them, so routing to a peer would be a
  // regression. Declared LOCAL_ONLY so api.invoke keeps them on the controller.
  // See PR #2428 (lint fence + dynamic import migration).
  //
  // NOTE: the runtime-owning Browser Control and Computer Use commands are NOT
  // here — they run the agent Tool, so they route to the host that runs the
  // Tool: Desktop Peer bridges them to its own webview (reads the peer's own
  // browser/OS), and CLI Peer refuses them in deny.rs (the UI gates the section
  // on host type). See SessionConfig + peer_host_invoke + cli deny.rs.
  'browser_webview_create',
  'browser_webview_eval',
  'browser_webview_navigate',
  'browser_webview_reload',
  'browser_webview_set_bounds',
  'debug_devtools_available',
  'debug_open_devtools',
  'resize_agent_companion_desktop_pet',
  'show_agent_companion_desktop_pet',
  'hide_agent_companion_desktop_pet',
  'append_flow_chat_diagnostics',
]);

/**
 * Session / workspace / chat / config path — must not wait behind git/SSH/editor
 * noise. Concurrency is capped (2); demoting `get_config` / modes / agent
 * profile to low starves peer hydrate (missing keys). See peer-device README.
 * Allowlist so new background commands default to normal/low.
 */
const HIGH_PRIORITY_COMMANDS = new Set([
  'restore_session_view',
  'load_session_event_backfill',
  'restore_session_with_turns',
  'restore_session',
  'load_session_turn_window',
  'load_session_turns',
  'list_persisted_sessions',
  'list_persisted_sessions_page',
  'list_persisted_sessions_count',
  'get_session_lineage',
  'get_session_thread_goal',
  'touch_session_activity',
  'create_session',
  'delete_session',
  'rename_session',
  'archive_session',
  'initialize_workspace_startup_state',
  'get_opened_workspaces',
  'get_recent_workspaces',
  'get_current_workspace',
  'worktree_list_projects',
  'open_workspace',
  'get_workspace_info',
  'reload_config',
  'get_config',
  'get_configs',
  'get_available_modes',
  'get_agent_profile_config',
  'start_dialog_turn',
  'cancel_dialog_turn',
  // Per-tool interrupt (Terminal cards) is interactive and time-sensitive: a
  // long-running shell command keeps producing side effects until the cancel
  // reaches the host. It must take the reserved high-priority slot, not queue
  // behind normal reads/mutations. See PR #2428 review #4.
  'cancel_tool',
  'rollback_session_to_turn',
  'list_pending_permission_requests',
  'subscribe_permission_requests',
  'respond_permission',
  'respond_permission_batch',
  'list_project_permission_grants',
  'remove_project_permission_grant',
  'clear_project_permission_grants',
  'list_project_permission_audit',
  'get_project_permission_rules',
  'save_project_permission_rules',
  // Interactive directory picking / browsing on the peer
  'get_directory_children',
  'get_directory_children_paginated',
  'list_files',
  'check_path_exists',
  'create_directory',
  'get_system_info',
]);

const RETRYABLE_READ_COMMANDS = new Set([
  'initialize_workspace_startup_state',
  'restore_session_view',
  'load_session_event_backfill',
  'restore_session_with_turns',
  'load_session_turn_window',
  'load_session_turns',
  'list_persisted_sessions',
  'list_persisted_sessions_page',
  'list_persisted_sessions_count',
  'get_session_lineage',
  'get_session_thread_goal',
  'get_opened_workspaces',
  'get_recent_workspaces',
  'get_current_workspace',
  'worktree_list_projects',
  'get_workspace_info',
  'get_config',
  'get_configs',
  'get_available_modes',
  'get_agent_profile_config',
  'list_pending_permission_requests',
  'list_project_permission_grants',
  'list_project_permission_audit',
  'get_project_permission_rules',
  'get_directory_children',
  'get_directory_children_paginated',
  'list_files',
  'check_path_exists',
  'get_system_info',
  // Browser Control / Computer Use status reads. These are pure reads with no
  // side effects (the launch / restart / CDP-enable / permission-prompt /
  // open-settings commands stay mutation below). They start with `browser_`
  // / `computer_`, not `get_`/`read_`/`list_`, so the prefix rules in
  // isPeerRetryableReadCommand miss them — list them explicitly so a Desktop
  // Peer settings page survives a brief relay blip with a read retry instead of
  // a 30 s mutation timeout. See PR #2428 round 5 #3.
  'browser_control_get_status',
  'browser_control_list_browsers',
  'computer_use_get_status',
]);

const RETRYABLE_IDEMPOTENT_MUTATION_COMMANDS = new Set([
  'start_dialog_turn',
  'start_acp_dialog_turn',
]);

export function isPeerLocalOnlyCommand(command: string): boolean {
  return LOCAL_ONLY_COMMANDS.has(command);
}

/**
 * `get_*` commands that run the announcement scheduler (mutate app_open_count +
 * persist state). They are NOT side-effect-free reads and must never be
 * auto-retried by the peer read path, where a retry would multiply the side
 * effect. They are also LOCAL_ONLY, but this guard keeps the contract explicit
 * if ownership ever moves back to the peer. See scheduler.rs run().
 */
const SIDE_EFFECTING_GET_COMMANDS = new Set([
  'get_pending_announcements',
  'get_announcement_tips',
]);

export function isPeerRetryableReadCommand(command: string): boolean {
  if (SIDE_EFFECTING_GET_COMMANDS.has(command)) {
    return false;
  }
  return RETRYABLE_READ_COMMANDS.has(command) ||
    command.startsWith('read_') ||
    command.startsWith('list_') ||
    command.startsWith('get_');
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === 'object' && value !== null
    ? value as Record<string, unknown>
    : null;
}

/**
 * Mutations are retryable only when the peer can deduplicate the same logical
 * submission. Dialog turns carry a controller-generated turnId, which the
 * Peer Host bridge uses as an idempotency key.
 */
export function isPeerRetryableIdempotentMutation(
  command: string,
  params: unknown,
): boolean {
  if (!RETRYABLE_IDEMPOTENT_MUTATION_COMMANDS.has(command)) {
    return false;
  }
  const request = asRecord(asRecord(params)?.request);
  return typeof request?.sessionId === 'string' &&
    request.sessionId.trim().length > 0 &&
    typeof request.turnId === 'string' &&
    request.turnId.trim().length > 0;
}

export type PeerInvokePriority = 'high' | 'normal' | 'low';

const LOW_PRIORITY_EXACT = new Set([
  'get_file_metadata',
  'read_file_content',
  'get_file_editor_sync_hash',
  'get_file_tree',
  'explorer_get_children',
  'start_file_watch',
  'stop_file_watch',
  'get_watched_paths',
  'load_canvas_artifact',
  'load_canvas_state',
  'search_get_repo_status',
  'search_build_index',
  'search_rebuild_index',
  'list_background_command_activities',
  'read_background_command_output',
  'get_health_status',
  'notify_cron_host_ready',
  'list_miniapps',
  'miniapp_worker_list_running',
]);

export function peerInvokePriorityFor(command: string): PeerInvokePriority {
  if (HIGH_PRIORITY_COMMANDS.has(command) || command.startsWith('terminal_')) {
    return 'high';
  }
  if (
    command.startsWith('git_') ||
    command.startsWith('ssh_') ||
    command.startsWith('lsp_') ||
    command.startsWith('search_') ||
    command.startsWith('explorer_') ||
    command.startsWith('miniapp_') ||
    LOW_PRIORITY_EXACT.has(command)
  ) {
    return 'low';
  }
  return 'normal';
}

/** Max in-flight HostInvoke RPCs per controller. */
export const PEER_HOST_INVOKE_MAX_CONCURRENT = 4;
export const PEER_READ_REQUEST_TIMEOUT_MS = 10_000;
export const PEER_MUTATION_REQUEST_TIMEOUT_MS = 30_000;
export const PEER_READ_MAX_RETRIES = 4;
export const PEER_IDEMPOTENT_MUTATION_MAX_RETRIES = 4;
export const PEER_RETRY_BASE_DELAY_MS = 500;

interface PeerRpcPolicy {
  timeoutMs: number;
  maxRetries: number;
  retryKind: 'read' | 'idempotent-mutation' | 'none';
}

const READ_RPC_POLICY: PeerRpcPolicy = {
  timeoutMs: PEER_READ_REQUEST_TIMEOUT_MS,
  maxRetries: PEER_READ_MAX_RETRIES,
  retryKind: 'read',
};

const MUTATION_RPC_POLICY: PeerRpcPolicy = {
  timeoutMs: PEER_MUTATION_REQUEST_TIMEOUT_MS,
  maxRetries: 0,
  retryKind: 'none',
};

const IDEMPOTENT_MUTATION_RPC_POLICY: PeerRpcPolicy = {
  timeoutMs: PEER_MUTATION_REQUEST_TIMEOUT_MS,
  maxRetries: PEER_IDEMPOTENT_MUTATION_MAX_RETRIES,
  retryKind: 'idempotent-mutation',
};

type DeviceRpcFn = (
  targetDeviceId: string,
  commandJson: string,
  timeoutMs?: number,
) => Promise<string>;

export interface PeerDeviceTransportHooks {
  /** Fired only for transport/RPC layer failures, not product command errors. */
  onHostInvokeTransportFailure?: (error: unknown, meta?: { action: string; priority: PeerInvokePriority }) => void;
  onHostInvokeSuccess?: () => void;
  /**
   * Enables replay of stable dialog submissions only when the target host
   * advertises matching execution-side deduplication.
   */
  supportsIdempotentDialogSubmit?: boolean;
  /** Enables targeted rollback only when the target host owns the transaction. */
  supportsTargetedSessionRollback?: boolean;
  /** Enables host-local usage statistics only when the target implements it. */
  supportsTokenUsageStatistics?: boolean;
}

interface HostInvokeResultEnvelope {
  resp?: string;
  ok?: boolean;
  value?: unknown;
  error?: string;
  message?: string;
}

export interface PeerDeviceCommandResponse {
  resp?: string;
  message?: string;
}

/** Product-level HostInvoke failure (peer executed the command and returned ok:false). */
export class PeerProductCommandError extends Error {
  readonly isPeerProductError = true;

  constructor(message: string) {
    super(message);
    this.name = 'PeerProductCommandError';
  }
}

class PeerRpcTimeoutError extends Error {
  constructor(action: string, timeoutMs: number) {
    super(`Peer request '${action}' timed out after ${timeoutMs}ms`);
    this.name = 'PeerRpcTimeoutError';
  }
}

/**
 * Raised when a request is abandoned because its adapter was torn down.
 *
 * It extends `SurfaceChangedError` so the one boundary that already unwinds
 * stale-surface work handles disposal too, rather than every call site growing
 * a second guard. What matters most is that it settles the promise at all:
 * queued entries used to be dropped without settling, so every caller awaiting
 * one (session list, session history) hung on a spinner forever.
 */
export class PeerTransportClosedError extends SurfaceChangedError {
  constructor(deviceId: string, epoch: number, action?: string) {
    super(surfaceIdForDevice(deviceId), epoch, action);
    this.name = 'PeerTransportClosedError';
    this.message = action
      ? `Peer transport for '${deviceId}' closed while '${action}' was pending`
      : `Peer transport for '${deviceId}' closed`;
  }
}

interface QueuedPeerRequest {
  priority: PeerInvokePriority;
  action: string;
  enqueuedAt: number;
  run: () => Promise<void>;
  /** Settle the caller without running (or awaiting) the request. */
  cancel: (error: Error) => void;
}

/**
 * Routes product invokes to a peer device via account Device RPC HostInvoke,
 * while keeping account / window / remote-connect commands on the local host.
 * Event listen stays local — peer events are re-emitted onto this machine.
 * Failures never fall back to the local product data plane.
 *
 * HostInvoke calls are priority-queued with a small concurrency limit so
 * session hydrate is not starved by background git/SSH/editor RPCs.
 */
export class PeerDeviceTransportAdapter implements ITransportAdapter {
  /** Which device surface this adapter's product traffic belongs to. */
  readonly surfaceId: DeviceSurfaceId;

  private readonly local = new TauriTransportAdapter();
  private connected = false;
  private disposed = false;
  /**
   * Whether this adapter is the transport the window currently renders.
   * Permissive until the registry says otherwise: the guard exists to catch
   * work outliving a switch *away*, not to gate an adapter nobody rendered yet.
   */
  private renderedTransport = true;
  /** Bumped on every bind/unbind, so a request can detect it outlived one. */
  private surfaceBindingEpoch = 0;
  private activeCount = 0;
  private readonly activeByPriority: Record<PeerInvokePriority, number> = {
    high: 0,
    normal: 0,
    low: 0,
  };
  private readonly queues: Record<PeerInvokePriority, QueuedPeerRequest[]> = {
    high: [],
    normal: [],
    low: [],
  };
  /** Every request whose caller promise has not settled — queued or in flight. */
  private readonly pending = new Set<QueuedPeerRequest>();

  constructor(
    private readonly targetDeviceId: string,
    private readonly deviceRpc: DeviceRpcFn,
    private hooks: PeerDeviceTransportHooks = {},
    private readonly maxConcurrent: number = PEER_HOST_INVOKE_MAX_CONCURRENT,
  ) {
    this.surfaceId = surfaceIdForDevice(targetDeviceId);
  }

  /**
   * Search progress is not part of the negotiated Peer DeviceEvent contract.
   * Use the response-based search command so older peers remain compatible and
   * results never fall back to the controller's filesystem.
   */
  supportsSearchStreamEvents(): boolean {
    return false;
  }

  getTargetDeviceId(): string {
    return this.targetDeviceId;
  }

  /**
   * Adopt the capabilities of the host that answered the latest handshake.
   * They belong to that host, not to the device id: a peer that restarted on
   * an older build during a reconnect must not keep the previous contract.
   */
  setHostCapabilities(
    capabilities: Pick<
      PeerDeviceTransportHooks,
      | 'supportsIdempotentDialogSubmit'
      | 'supportsTargetedSessionRollback'
      | 'supportsTokenUsageStatistics'
    >,
  ): void {
    this.hooks = { ...this.hooks, ...capabilities };
  }

  /**
   * Registry hook: this adapter has started or stopped being the transport the
   * window renders. A request issued under an earlier binding fails as a
   * surface change when it lands, so slow work — a session-list read still
   * working through its retry budget, say — cannot resolve into the surface
   * that replaced it.
   */
  markRenderedTransport(rendered: boolean, forceNewBinding = false): void {
    if (rendered === this.renderedTransport && !forceNewBinding) {
      return;
    }
    this.renderedTransport = rendered;
    this.surfaceBindingEpoch += 1;
    const error = new SurfaceChangedError(
      this.surfaceId,
      getActiveSurfaceScope().epoch,
      'change peer transport binding',
    );
    // Settle callers immediately at the activation boundary. Work already on
    // the wire may finish on its owning host, but nobody should wait through a
    // timeout/retry budget before learning that its rendered surface is gone.
    for (const entry of Array.from(this.pending)) {
      entry.cancel(error);
    }
    for (const priority of ['high', 'normal', 'low'] as const) {
      for (const entry of this.queues[priority]) {
        this.pending.delete(entry);
      }
      this.queues[priority].length = 0;
    }
  }

  isDisposed(): boolean {
    return this.disposed;
  }

  async connect(): Promise<void> {
    this.assertUsable('connect');
    await this.local.connect();
    this.connected = true;
  }

  async request<T>(action: string, params?: any, timing?: TransportRequestTiming): Promise<T> {
    const transportStartedAt = nowMs();
    this.assertUsable(action);
    if (!this.connected) {
      await this.connect();
    }

    if (isPeerLocalOnlyCommand(action)) {
      // Local-only commands run on this machine by definition, so they are not
      // bound to the rendered surface and stay valid on a retired adapter.
      return this.local.request<T>(action, params, timing);
    }

    this.assertRenderedTransport(action);

    if (
      action === 'rollback_session_to_turn' &&
      this.hooks.supportsTargetedSessionRollback !== true
    ) {
      throw new PeerProductCommandError(
        'The connected Peer host does not support targeted Session rollback',
      );
    }

    if (
      action === 'get_token_usage_statistics' &&
      this.hooks.supportsTokenUsageStatistics !== true
    ) {
      throw new PeerProductCommandError(
        'token_usage_statistics_unsupported: The connected Peer host does not support usage statistics',
      );
    }

    const issuedBindingEpoch = this.surfaceBindingEpoch;
    const priority = peerInvokePriorityFor(action);
    return this.enqueue(
      priority,
      action,
      () => this.invokeOnPeer<T>(action, params, timing, transportStartedAt, issuedBindingEpoch),
    );
  }

  /**
   * Send an existing RemoteCommand envelope directly to the peer. This is for
   * split-endpoint operations such as file download, where the peer reads the
   * source but the controller owns the destination path and local write.
   */
  async requestPeerCommand<T extends PeerDeviceCommandResponse>(
    command: Record<string, unknown>,
    priority: PeerInvokePriority = 'normal',
  ): Promise<T> {
    const action = typeof command.cmd === 'string' ? command.cmd : 'unknown';
    this.assertUsable(action);
    if (!this.connected) {
      await this.connect();
    }
    this.assertRenderedTransport(action);
    const issuedBindingEpoch = this.surfaceBindingEpoch;
    return this.enqueue(
      priority,
      action,
      () => this.invokePeerCommand<T>(command, priority, issuedBindingEpoch),
    );
  }

  listen<T>(event: string, callback: (data: T) => void): () => void {
    return this.local.listen<T>(event, callback);
  }

  async waitForListenerRegistrations?(): Promise<void> {
    await this.local.waitForListenerRegistrations?.();
  }

  /**
   * Dispose this adapter. Terminal on purpose: an adapter dies with its peer
   * link, and a late request must not silently re-open a data plane to a device
   * we have detached from.
   *
   * Every unsettled caller is rejected. Queued entries used to be discarded
   * with `queue.length = 0`, which never settled their promises — that is what
   * left session-list and history loads spinning forever after a teardown.
   */
  async disconnect(): Promise<void> {
    this.disposed = true;
    this.connected = false;
    for (const priority of ['high', 'normal', 'low'] as const) {
      this.queues[priority].length = 0;
    }
    for (const entry of Array.from(this.pending)) {
      entry.cancel(this.closedError(entry.action));
    }
    this.pending.clear();
    // Concurrency counters are deliberately left alone: requests already handed
    // to the transport still settle and decrement themselves. Zeroing them here
    // made each late settle decrement a freshly reset count, so the limiter
    // afterwards believed it had free slots it did not have.
    await this.local.disconnect();
  }

  isConnected(): boolean {
    return this.connected && this.local.isConnected();
  }

  /** Test helper: current queued depths by priority. */
  getQueueDepthsForTest(): Record<PeerInvokePriority, number> {
    return {
      high: this.queues.high.length,
      normal: this.queues.normal.length,
      low: this.queues.low.length,
    };
  }

  /** Test helper: requests currently occupying a concurrency slot. */
  getActiveCountsForTest(): Record<PeerInvokePriority | 'total', number> {
    return {
      total: this.activeCount,
      high: this.activeByPriority.high,
      normal: this.activeByPriority.normal,
      low: this.activeByPriority.low,
    };
  }

  private closedError(action?: string): PeerTransportClosedError {
    return new PeerTransportClosedError(
      this.targetDeviceId,
      getActiveSurfaceScope().epoch,
      action,
    );
  }

  private assertUsable(action: string): void {
    if (this.disposed) {
      throw this.closedError(action);
    }
  }

  private assertRenderedTransport(action: string): void {
    if (!this.renderedTransport) {
      throw new SurfaceChangedError(
        this.surfaceId,
        getActiveSurfaceScope().epoch,
        action,
      );
    }
  }

  private enqueue<T>(
    priority: PeerInvokePriority,
    action: string,
    task: () => Promise<T>,
  ): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      let settled = false;
      const settle = (outcome: () => void): void => {
        if (settled) {
          return;
        }
        settled = true;
        outcome();
      };
      const entry: QueuedPeerRequest = {
        priority,
        action,
        enqueuedAt: nowMs(),
        run: async () => {
          try {
            const value = await task();
            settle(() => resolve(value));
          } catch (error) {
            settle(() => reject(error));
          }
        },
        cancel: (error: Error) => settle(() => reject(error)),
      };
      this.pending.add(entry);
      this.queues[priority].push(entry);
      this.pump();
    });
  }

  private pump(): void {
    if (this.disposed) {
      return;
    }
    while (this.activeCount < this.maxConcurrent) {
      const next = this.dequeueNext();
      if (!next) {
        return;
      }
      this.activeCount += 1;
      this.activeByPriority[next.priority] += 1;
      void next.run().finally(() => {
        this.pending.delete(next);
        this.activeCount = Math.max(0, this.activeCount - 1);
        this.activeByPriority[next.priority] = Math.max(
          0,
          this.activeByPriority[next.priority] - 1,
        );
        this.pump();
      });
    }
  }

  private dequeueNext(): QueuedPeerRequest | undefined {
    // Prefer high, then normal. Allow low only when nothing higher is waiting,
    // so background git/SSH cannot monopolize slots after a hydrate burst.
    if (this.queues.high.length > 0) {
      return this.queues.high.shift();
    }
    const nonHighConcurrencyLimit =
      this.maxConcurrent > 1 ? this.maxConcurrent - 1 : 1;
    const activeNonHigh = this.activeByPriority.normal + this.activeByPriority.low;
    if (
      this.queues.normal.length > 0 &&
      activeNonHigh < nonHighConcurrencyLimit
    ) {
      return this.queues.normal.shift();
    }
    // Keep one transport slot available for future interactive work. Without
    // this, two slow background RPCs can make terminal input appear frozen.
    const lowConcurrencyLimit = this.maxConcurrent > 1 ? this.maxConcurrent - 1 : 1;
    if (
      this.queues.low.length > 0 &&
      this.activeByPriority.low < lowConcurrencyLimit
    ) {
      return this.queues.low.shift();
    }
    return undefined;
  }

  private async invokePeerCommand<T extends PeerDeviceCommandResponse>(
    command: Record<string, unknown>,
    priority: PeerInvokePriority,
    issuedBindingEpoch: number,
  ): Promise<T> {
    const action = typeof command.cmd === 'string' ? command.cmd : 'unknown';
    try {
      const retryable =
        action === 'get_file_info' ||
        action === 'read_file_chunk' ||
        action.startsWith('get_') ||
        action.startsWith('read_') ||
        action.startsWith('list_');
      const raw = await this.invokeDeviceRpc(
        action,
        JSON.stringify(command),
        retryable ? READ_RPC_POLICY : MUTATION_RPC_POLICY,
        issuedBindingEpoch,
      );
      this.assertBindingUnchanged(issuedBindingEpoch, action);
      const envelope = JSON.parse(raw) as T;
      if (envelope.resp === 'error') {
        throw new PeerProductCommandError(
          envelope.message || `Peer command '${action}' failed`,
        );
      }
      if (!envelope.resp) {
        throw new Error(`Unexpected peer RPC response for '${action}'`);
      }
      this.hooks.onHostInvokeSuccess?.();
      return envelope;
    } catch (error) {
      if (error instanceof PeerProductCommandError) {
        log.warn('Peer product command failed', { action, error });
        throw error;
      }
      if (isSurfaceChangedError(error)) {
        throw error;
      }
      log.error('Peer direct command transport failed', { action, error });
      this.hooks.onHostInvokeTransportFailure?.(error, { action, priority });
      throw error;
    }
  }

  /**
   * A request that outlived its surface binding must not resolve: its answer
   * describes a device this window has stopped rendering, and feeding it to the
   * current surface is how a peer's session list ended up drawn over the local
   * one. The peer-side effect of a mutation still stands — the caller's own
   * generation guard decides what to do about that.
   */
  private assertBindingUnchanged(issuedBindingEpoch: number, action: string): void {
    if (issuedBindingEpoch !== this.surfaceBindingEpoch) {
      throw new SurfaceChangedError(
        this.surfaceId,
        getActiveSurfaceScope().epoch,
        action,
      );
    }
  }

  private async invokeOnPeer<T>(
    action: string,
    params: unknown,
    timing: TransportRequestTiming | undefined,
    transportStartedAt: number,
    issuedBindingEpoch: number,
  ): Promise<T> {
    const invokeStartedAt = nowMs();
    const priority = peerInvokePriorityFor(action);
    const commandJson = JSON.stringify({
      cmd: 'host_invoke',
      command: action,
      args: params === undefined ? {} : params,
    });
    const rpcPolicy = isPeerRetryableReadCommand(action)
      ? READ_RPC_POLICY
      : this.hooks.supportsIdempotentDialogSubmit === true &&
          isPeerRetryableIdempotentMutation(action, params)
        ? IDEMPOTENT_MUTATION_RPC_POLICY
        : MUTATION_RPC_POLICY;

    try {
      const raw = await this.invokeDeviceRpc(
        action,
        commandJson,
        rpcPolicy,
        issuedBindingEpoch,
      );
      this.assertBindingUnchanged(issuedBindingEpoch, action);
      const envelope = JSON.parse(raw) as HostInvokeResultEnvelope;
      if (timing) {
        timing.invokeDurationMs = elapsedMs(invokeStartedAt);
        timing.transportDurationMs = elapsedMs(transportStartedAt);
      }

      if (envelope.resp === 'error') {
        throw new Error(envelope.message || 'Peer HostInvoke failed');
      }
      if (envelope.resp === 'host_invoke_result') {
        if (!envelope.ok) {
          // Product failure on the peer — do not count as transport loss.
          throw new PeerProductCommandError(
            envelope.error || `Peer command '${action}' failed`,
          );
        }
        this.hooks.onHostInvokeSuccess?.();
        return envelope.value as T;
      }
      throw new Error(
        `Unexpected peer RPC response for '${action}': ${envelope.resp || 'unknown'}`,
      );
    } catch (error) {
      if (error instanceof PeerProductCommandError) {
        log.warn('Peer product command failed', { action, error });
        throw error;
      }
      // A surface change is control flow, not a weak link: reporting it as a
      // transport failure would mark a healthy peer degraded on every switch.
      if (isSurfaceChangedError(error)) {
        throw error;
      }
      log.error('Peer HostInvoke transport failed', { action, error });
      this.hooks.onHostInvokeTransportFailure?.(error, { action, priority });
      throw error;
    }
  }

  private async invokeDeviceRpc(
    action: string,
    commandJson: string,
    policy: PeerRpcPolicy,
    issuedBindingEpoch: number,
  ): Promise<string> {
    for (let attempt = 0; ; attempt += 1) {
      this.assertBindingUnchanged(issuedBindingEpoch, action);
      try {
        const result = await this.withTimeout(
          this.deviceRpc(this.targetDeviceId, commandJson, policy.timeoutMs),
          action,
          policy.timeoutMs,
        );
        this.assertBindingUnchanged(issuedBindingEpoch, action);
        return result;
      } catch (error) {
        if (isSurfaceChangedError(error)) {
          throw error;
        }
        // A retired binding is cancellation, not a weak relay. In particular,
        // do not keep replaying a read after another device is rendered.
        this.assertBindingUnchanged(issuedBindingEpoch, action);
        if (attempt >= policy.maxRetries) {
          throw error;
        }
        const delayMs = PEER_RETRY_BASE_DELAY_MS * (2 ** attempt);
        log.warn('Retrying recoverable Peer request', {
          action,
          retryKind: policy.retryKind,
          attempt: attempt + 1,
          maxRetries: policy.maxRetries,
          delayMs,
          error,
        });
        await new Promise(resolve => setTimeout(resolve, delayMs));
        this.assertBindingUnchanged(issuedBindingEpoch, action);
      }
    }
  }

  private async withTimeout<T>(
    request: Promise<T>,
    action: string,
    timeoutMs: number,
  ): Promise<T> {
    let timeout: ReturnType<typeof setTimeout> | undefined;
    try {
      return await Promise.race([
        request,
        new Promise<T>((_resolve, reject) => {
          timeout = setTimeout(() => {
            reject(new PeerRpcTimeoutError(action, timeoutMs));
          }, timeoutMs);
        }),
      ]);
    } finally {
      if (timeout !== undefined) {
        clearTimeout(timeout);
      }
    }
  }
}
