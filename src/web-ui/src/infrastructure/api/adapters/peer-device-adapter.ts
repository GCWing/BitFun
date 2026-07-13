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

export function isPeerLocalOnlyCommand(command: string): boolean {
  return LOCAL_ONLY_COMMANDS.has(command);
}

type DeviceRpcFn = (targetDeviceId: string, commandJson: string) => Promise<string>;

interface HostInvokeResultEnvelope {
  resp?: string;
  ok?: boolean;
  value?: unknown;
  error?: string;
  message?: string;
}

/**
 * Routes product invokes to a peer device via account Device RPC HostInvoke,
 * while keeping account / window / remote-connect commands on the local host.
 * Event listen stays local — peer events are re-emitted onto this machine.
 */
export class PeerDeviceTransportAdapter implements ITransportAdapter {
  private readonly local = new TauriTransportAdapter();
  private connected = false;

  constructor(
    private readonly targetDeviceId: string,
    private readonly deviceRpc: DeviceRpcFn,
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

    const invokeStartedAt = nowMs();
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
          throw new Error(envelope.error || `Peer command '${action}' failed`);
        }
        return envelope.value as T;
      }
      throw new Error(`Unexpected peer RPC response for '${action}': ${envelope.resp || 'unknown'}`);
    } catch (error) {
      log.error('Peer HostInvoke failed', { action, error });
      throw error;
    }
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
  }

  isConnected(): boolean {
    return this.connected && this.local.isConnected();
  }
}
