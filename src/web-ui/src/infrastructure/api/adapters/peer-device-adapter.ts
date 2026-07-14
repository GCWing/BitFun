import { ITransportAdapter, type TransportRequestTiming } from './base';
import { TauriTransportAdapter } from './tauri-adapter';
import { createLogger } from '@/shared/utils/logger';
import { elapsedMs, nowMs } from '@/shared/utils/timing';

const log = createLogger('PeerDeviceTransport');

/** Commands that must always hit the local Tauri host, even in peer mode. */
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
]);

/**
 * Session / workspace / chat path — must not wait behind git/SSH/editor noise.
 * Kept as an allowlist so new background commands default to normal/low.
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
  'start_dialog_turn',
  'cancel_dialog_turn',
  'confirm_tool_execution',
  'reject_tool_execution',
  // Interactive directory picking / browsing on the peer
  'get_directory_children',
  'get_directory_children_paginated',
  'list_files',
  'check_path_exists',
  'create_directory',
  'get_system_info',
]);

export function isPeerLocalOnlyCommand(command: string): boolean {
  return LOCAL_ONLY_COMMANDS.has(command);
}

export type PeerInvokePriority = 'high' | 'normal' | 'low';

const LOW_PRIORITY_EXACT = new Set([
  'get_file_metadata',
  'read_file_content',
  'get_file_editor_sync_hash',
  'get_config',
  'get_configs',
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
  'get_available_modes',
  'get_agent_profile_config',
  'list_miniapps',
  'miniapp_worker_list_running',
]);

export function peerInvokePriorityFor(command: string): PeerInvokePriority {
  if (HIGH_PRIORITY_COMMANDS.has(command)) {
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

/** Max in-flight HostInvoke RPCs per controller. Keep low to avoid relay 504 pile-ups. */
export const PEER_HOST_INVOKE_MAX_CONCURRENT = 2;

type DeviceRpcFn = (targetDeviceId: string, commandJson: string) => Promise<string>;

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

/** Product-level HostInvoke failure (peer executed the command and returned ok:false). */
export class PeerProductCommandError extends Error {
  readonly isPeerProductError = true;

  constructor(message: string) {
    super(message);
    this.name = 'PeerProductCommandError';
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
      void next.run().finally(() => {
        this.activeCount -= 1;
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
    if (this.queues.normal.length > 0) {
      return this.queues.normal.shift();
    }
    if (this.queues.low.length > 0) {
      return this.queues.low.shift();
    }
    return undefined;
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
      const raw = await this.deviceRpc(this.targetDeviceId, commandJson);
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
}
