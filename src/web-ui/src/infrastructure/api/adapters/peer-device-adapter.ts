import { ITransportAdapter, type TransportRequestTiming } from './base';
import { TauriTransportAdapter } from './tauri-adapter';
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
  'restart_app',
  'check_for_updates',
  'install_update',
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
  'computer_use_request_permissions',
  'computer_use_open_system_settings',
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
]);

/**
 * Session / workspace / chat / config path — must not wait behind git/SSH/editor
 * noise. Concurrency is capped (2); demoting `get_config` / modes / agent
 * profile to low starves peer hydrate (missing keys). See peer-device README.
 * Allowlist so new background commands default to normal/low.
 */
const HIGH_PRIORITY_COMMANDS = new Set([
  'restore_session_view',
  'restore_session_with_turns',
  'restore_session',
  'load_session_turns',
  'list_persisted_sessions',
  'list_persisted_sessions_page',
  'list_persisted_sessions_count',
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
  'open_workspace',
  'get_workspace_info',
  'reload_config',
  'get_config',
  'get_configs',
  'get_available_modes',
  'get_agent_profile_config',
  'start_dialog_turn',
  'cancel_dialog_turn',
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
  'restore_session_with_turns',
  'load_session_turns',
  'list_persisted_sessions',
  'list_persisted_sessions_page',
  'list_persisted_sessions_count',
  'get_session_thread_goal',
  'get_opened_workspaces',
  'get_recent_workspaces',
  'get_current_workspace',
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
]);

export function isPeerLocalOnlyCommand(command: string): boolean {
  return LOCAL_ONLY_COMMANDS.has(command);
}

export function isPeerRetryableReadCommand(command: string): boolean {
  return RETRYABLE_READ_COMMANDS.has(command) ||
    command.startsWith('read_') ||
    command.startsWith('list_') ||
    command.startsWith('get_');
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
export const PEER_READ_MAX_RETRIES = 2;
export const PEER_RETRY_BASE_DELAY_MS = 500;

type DeviceRpcFn = (
  targetDeviceId: string,
  commandJson: string,
  timeoutMs?: number,
) => Promise<string>;

export interface PeerDeviceTransportHooks {
  /** Fired only for transport/RPC layer failures, not product command errors. */
  onHostInvokeTransportFailure?: (error: unknown, meta?: { action: string; priority: PeerInvokePriority }) => void;
  onHostInvokeSuccess?: () => void;
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

interface QueuedPeerRequest {
  priority: PeerInvokePriority;
  enqueuedAt: number;
  run: () => Promise<void>;
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
  private readonly local = new TauriTransportAdapter();
  private connected = false;
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

  constructor(
    private readonly targetDeviceId: string,
    private readonly deviceRpc: DeviceRpcFn,
    private readonly hooks: PeerDeviceTransportHooks = {},
    private readonly maxConcurrent: number = PEER_HOST_INVOKE_MAX_CONCURRENT,
  ) {}

  getTargetDeviceId(): string {
    return this.targetDeviceId;
  }

  async connect(): Promise<void> {
    await this.local.connect();
    this.connected = true;
  }

  async request<T>(action: string, params?: any, timing?: TransportRequestTiming): Promise<T> {
    const transportStartedAt = nowMs();
    if (!this.connected) {
      await this.connect();
    }

    if (isPeerLocalOnlyCommand(action)) {
      return this.local.request<T>(action, params, timing);
    }

    const priority = peerInvokePriorityFor(action);
    return this.enqueue(priority, () => this.invokeOnPeer<T>(action, params, timing, transportStartedAt));
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
    if (!this.connected) {
      await this.connect();
    }
    return this.enqueue(priority, () => this.invokePeerCommand<T>(command, priority));
  }

  listen<T>(event: string, callback: (data: T) => void): () => void {
    return this.local.listen<T>(event, callback);
  }

  async waitForListenerRegistrations?(): Promise<void> {
    await this.local.waitForListenerRegistrations?.();
  }

  async disconnect(): Promise<void> {
    await this.local.disconnect();
    this.connected = false;
    for (const priority of ['high', 'normal', 'low'] as const) {
      this.queues[priority].length = 0;
      this.activeByPriority[priority] = 0;
    }
    this.activeCount = 0;
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

  private enqueue<T>(priority: PeerInvokePriority, task: () => Promise<T>): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      this.queues[priority].push({
        priority,
        enqueuedAt: nowMs(),
        run: async () => {
          try {
            resolve(await task());
          } catch (error) {
            reject(error);
          }
        },
      });
      this.pump();
    });
  }

  private pump(): void {
    while (this.activeCount < this.maxConcurrent) {
      const next = this.dequeueNext();
      if (!next) {
        return;
      }
      this.activeCount += 1;
      this.activeByPriority[next.priority] += 1;
      void next.run().finally(() => {
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
        retryable,
      );
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
      log.error('Peer direct command transport failed', { action, error });
      this.hooks.onHostInvokeTransportFailure?.(error, { action, priority });
      throw error;
    }
  }

  private async invokeOnPeer<T>(
    action: string,
    params: unknown,
    timing: TransportRequestTiming | undefined,
    transportStartedAt: number,
  ): Promise<T> {
    const invokeStartedAt = nowMs();
    const priority = peerInvokePriorityFor(action);
    const commandJson = JSON.stringify({
      cmd: 'host_invoke',
      command: action,
      args: params === undefined ? {} : params,
    });

    try {
      const raw = await this.invokeDeviceRpc(
        action,
        commandJson,
        isPeerRetryableReadCommand(action),
      );
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
      log.error('Peer HostInvoke transport failed', { action, error });
      this.hooks.onHostInvokeTransportFailure?.(error, { action, priority });
      throw error;
    }
  }

  private async invokeDeviceRpc(
    action: string,
    commandJson: string,
    retryable: boolean,
  ): Promise<string> {
    const timeoutMs = retryable
      ? PEER_READ_REQUEST_TIMEOUT_MS
      : PEER_MUTATION_REQUEST_TIMEOUT_MS;
    const maxRetries = retryable ? PEER_READ_MAX_RETRIES : 0;

    for (let attempt = 0; ; attempt += 1) {
      try {
        return await this.withTimeout(
          this.deviceRpc(this.targetDeviceId, commandJson, timeoutMs),
          action,
          timeoutMs,
        );
      } catch (error) {
        if (attempt >= maxRetries) {
          throw error;
        }
        const delayMs = PEER_RETRY_BASE_DELAY_MS * (2 ** attempt);
        log.warn('Retrying read-only Peer request', {
          action,
          attempt: attempt + 1,
          maxRetries,
          delayMs,
          error,
        });
        await new Promise(resolve => setTimeout(resolve, delayMs));
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
