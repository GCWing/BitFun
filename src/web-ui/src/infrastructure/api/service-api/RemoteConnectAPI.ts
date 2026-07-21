/**
 * Remote Connect API — calls Tauri commands for remote connection management.
 */

import { getTransportAdapter } from '../adapters';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('RemoteConnectAPI');

export interface DeviceInfo {
  device_id: string;
  device_name: string;
  mac_address: string;
}

export interface ConnectionMethodInfo {
  id: string;
  name: string;
  available: boolean;
  description: string;
}

export interface ConnectionResult {
  method: string;
  qr_data: string | null;
  qr_svg: string | null;
  qr_url: string | null;
  bot_pairing_code: string | null;
  bot_link: string | null;
  pairing_state: string;
}

export interface RemoteConnectStatus {
  is_connected: boolean;
  pairing_state: string;
  active_method: string | null;
  peer_device_name: string | null;
  peer_user_id: string | null;
  bot_connected: string | null;
  bot_verbose_mode: boolean;
}

export interface LanNetworkInterface {
  interface_name: string;
  ip: string;
  gateway_ip: string | null;
}

export interface LanNetworkInfo {
  local_ip: string;
  gateway_ip: string | null;
  available_ips: LanNetworkInterface[];
}

export interface RemoteConnectFormState {
  custom_server_url: string;
  telegram_bot_token: string;
  feishu_app_id: string;
  feishu_app_secret: string;
  weixin_ilink_token?: string;
  weixin_base_url?: string;
  weixin_bot_account_id?: string;
}

export interface WeixinQrStartResponse {
  session_key: string;
  qr_image_url: string;
  message: string;
}

export type WeixinQrPollStatus =
  | 'wait'
  | 'scanned'
  | 'confirmed'
  | 'expired'
  | 'error';

export interface WeixinQrPollResponse {
  status: WeixinQrPollStatus;
  message: string;
  qr_image_url: string | null;
  ilink_token: string | null;
  bot_account_id: string | null;
  base_url: string | null;
}

export interface AccountLoginResult {
  token: string;
  user_id: string;
  has_cloud_settings: boolean;
}

export interface AccountHint {
  username: string;
  relay_url: string;
}

export interface AutoSyncResult {
  settings_synced: boolean;
  sessions_exported: number;
  sessions_imported: number;
}

export interface AccountStatus {
  logged_in: boolean;
  user_id: string | null;
}

export interface OnlineDeviceInfo {
  device_id: string;
  device_name: string;
}

export interface AccountDeviceInfo {
  device_id: string;
  device_name: string;
  online: boolean;
  last_seen_at: number | null;
}

export interface SyncedSession {
  session_id: string;
  session_json: string;
}

class RemoteConnectAPIService {
  private get adapter() {
    return getTransportAdapter();
  }

  async getDeviceInfo(): Promise<DeviceInfo> {
    try {
      return await this.adapter.request<DeviceInfo>('remote_connect_get_device_info');
    } catch (e) {
      log.error('getDeviceInfo failed', e);
      throw e;
    }
  }

  async getLanIp(): Promise<string | null> {
    try {
      return await this.adapter.request<string>('remote_connect_get_lan_ip');
    } catch (e) {
      log.warn('getLanIp failed', e);
      return null;
    }
  }

  async getLanNetworkInfo(): Promise<LanNetworkInfo | null> {
    try {
      return await this.adapter.request<LanNetworkInfo>('remote_connect_get_lan_network_info');
    } catch (e) {
      log.warn('getLanNetworkInfo failed', e);
      return null;
    }
  }

  async getConnectionMethods(): Promise<ConnectionMethodInfo[]> {
    try {
      return await this.adapter.request<ConnectionMethodInfo[]>('remote_connect_get_methods');
    } catch (e) {
      log.error('getConnectionMethods failed', e);
      throw e;
    }
  }

  async startConnection(method: string, customServerUrl?: string, lanIp?: string): Promise<ConnectionResult> {
    try {
      return await this.adapter.request<ConnectionResult>('remote_connect_start', {
        request: { method, custom_server_url: customServerUrl ?? null, lan_ip: lanIp ?? null },
      });
    } catch (e) {
      log.error('startConnection failed', e);
      throw e;
    }
  }

  async stopConnection(): Promise<void> {
    try {
      await this.adapter.request<void>('remote_connect_stop');
    } catch (e) {
      log.error('stopConnection failed', e);
      throw e;
    }
  }

  async getStatus(): Promise<RemoteConnectStatus> {
    try {
      return await this.adapter.request<RemoteConnectStatus>('remote_connect_status');
    } catch (e) {
      log.error('getStatus failed', e);
      throw e;
    }
  }

  async getFormState(): Promise<RemoteConnectFormState> {
    try {
      return await this.adapter.request<RemoteConnectFormState>('remote_connect_get_form_state');
    } catch (e) {
      log.error('getFormState failed', e);
      throw e;
    }
  }

  async setFormState(formState: RemoteConnectFormState): Promise<void> {
    try {
      await this.adapter.request<void>('remote_connect_set_form_state', { request: formState });
    } catch (e) {
      log.error('setFormState failed', e);
      throw e;
    }
  }

  async stopBot(): Promise<void> {
    try {
      await this.adapter.request<void>('remote_connect_stop_bot');
    } catch (e) {
      log.error('stopBot failed', e);
      throw e;
    }
  }

  async configureCustomServer(url: string): Promise<void> {
    try {
      await this.adapter.request<void>('remote_connect_configure_custom_server', { url });
    } catch (e) {
      log.error('configureCustomServer failed', e);
      throw e;
    }
  }

  async configureBot(params: {
    botType: string;
    appId?: string;
    appSecret?: string;
    botToken?: string;
    weixinIlinkToken?: string;
    weixinBaseUrl?: string;
    weixinBotAccountId?: string;
  }): Promise<void> {
    try {
      await this.adapter.request<void>('remote_connect_configure_bot', {
        request: {
          bot_type: params.botType,
          app_id: params.appId ?? null,
          app_secret: params.appSecret ?? null,
          bot_token: params.botToken ?? null,
          weixin_ilink_token: params.weixinIlinkToken ?? null,
          weixin_base_url: params.weixinBaseUrl ?? null,
          weixin_bot_account_id: params.weixinBotAccountId ?? null,
        },
      });
    } catch (e) {
      log.error('configureBot failed', e);
      throw e;
    }
  }

  async weixinQrStart(baseUrl?: string | null): Promise<WeixinQrStartResponse> {
    return await this.adapter.request<WeixinQrStartResponse>('remote_connect_weixin_qr_start', {
      request: { base_url: baseUrl ?? null },
    });
  }

  async weixinQrPoll(sessionKey: string, baseUrl?: string | null): Promise<WeixinQrPollResponse> {
    return await this.adapter.request<WeixinQrPollResponse>('remote_connect_weixin_qr_poll', {
      request: { session_key: sessionKey, base_url: baseUrl ?? null },
    });
  }

  async getBotVerboseMode(): Promise<boolean> {
    try {
      return await this.adapter.request<boolean>('remote_connect_get_bot_verbose_mode');
    } catch (e) {
      log.error('getBotVerboseMode failed', e);
      return false;
    }
  }

  async setBotVerboseMode(verbose: boolean): Promise<void> {
    try {
      await this.adapter.request<void>('remote_connect_set_bot_verbose_mode', { verbose });
    } catch (e) {
      log.error('setBotVerboseMode failed', e);
      throw e;
    }
  }

  async accountLogin(relayUrl: string, username: string, password: string): Promise<AccountLoginResult> {
    try {
      return await this.adapter.request<AccountLoginResult>('account_login', {
        request: { relay_url: relayUrl, username, password },
      });
    } catch (e) {
      log.error('accountLogin failed', e);
      throw e;
    }
  }

  /**
   * Persist an in-memory login after the user accepts the cloud/local settings
   * choice. Without this, a process kill during the choice dialog must not
   * restore a logged-in session.
   */
  async accountFinalizeLogin(): Promise<void> {
    try {
      await this.adapter.request<void>('account_finalize_login');
    } catch (e) {
      log.error('accountFinalizeLogin failed', e);
      throw e;
    }
  }

  async accountStatus(): Promise<AccountStatus> {
    try {
      return await this.adapter.request<AccountStatus>('account_status');
    } catch (e) {
      log.warn('accountStatus failed', e);
      return { logged_in: false, user_id: null };
    }
  }

  async accountGetCredentialHint(): Promise<AccountHint | null> {
    try {
      return await this.adapter.request<AccountHint | null>('account_get_credential_hint');
    } catch (e) {
      log.warn('accountGetCredentialHint failed', e);
      return null;
    }
  }

  async accountTokenExpired(): Promise<boolean> {
    try {
      return await this.adapter.request<boolean>('account_token_expired');
    } catch (e) {
      log.warn('accountTokenExpired failed', e);
      return false;
    }
  }

  async accountLogout(): Promise<void> {
    try {
      await this.adapter.request<void>('account_logout');
    } catch (e) {
      log.error('accountLogout failed', e);
      throw e;
    }
  }

  // ── P2: Device routing ──────────────────────────────────────────────────

  async accountConnectDevices(): Promise<OnlineDeviceInfo[]> {
    try {
      return await this.adapter.request<OnlineDeviceInfo[]>('account_connect_devices');
    } catch (e) {
      log.error('accountConnectDevices failed', e);
      throw e;
    }
  }

  async accountOnlineDevices(): Promise<OnlineDeviceInfo[]> {
    try {
      return await this.adapter.request<OnlineDeviceInfo[]>('account_online_devices');
    } catch (e) {
      log.warn('accountOnlineDevices failed', e);
      return [];
    }
  }

  async accountSendSessionToDevice(
    targetDeviceId: string,
    sessionId: string,
    sessionJson: string,
  ): Promise<void> {
    try {
      await this.adapter.request<void>('account_send_session_to_device', {
        targetDeviceId,
        sessionId,
        sessionJson,
      });
    } catch (e) {
      log.error('accountSendSessionToDevice failed', e);
      throw e;
    }
  }

  // ── P4: Session / settings sync ─────────────────────────────────────────

  async accountSyncSession(sessionId: string, sessionJson: string): Promise<void> {
    try {
      await this.adapter.request<void>('account_sync_session', {
        sessionId,
        sessionJson,
      });
    } catch (e) {
      log.error('accountSyncSession failed', e);
      throw e;
    }
  }

  async accountFetchSyncedSessions(): Promise<SyncedSession[]> {
    try {
      return await this.adapter.request<SyncedSession[]>('account_fetch_synced_sessions');
    } catch (e) {
      log.error('accountFetchSyncedSessions failed', e);
      throw e;
    }
  }

  async accountDeleteSyncedSession(sessionId: string): Promise<void> {
    try {
      await this.adapter.request<void>('account_delete_synced_session', {
        sessionId,
      });
    } catch (e) {
      log.error('accountDeleteSyncedSession failed', e);
      throw e;
    }
  }

  async accountSyncSettings(settingsJson: string): Promise<void> {
    try {
      await this.adapter.request<void>('account_sync_settings', {
        settingsJson,
      });
    } catch (e) {
      log.error('accountSyncSettings failed', e);
      throw e;
    }
  }

  async accountFetchSettings(): Promise<string | null> {
    try {
      return await this.adapter.request<string | null>('account_fetch_settings');
    } catch (e) {
      log.error('accountFetchSettings failed', e);
      return null;
    }
  }

  // ── High-level session sync ───────────────────────────────────────────────

  async accountExportLocalSession(
    sessionId: string,
    workspacePath: string,
  ): Promise<void> {
    try {
      await this.adapter.request<void>('account_export_local_session', {
        sessionId,
        workspacePath,
      });
    } catch (e) {
      log.error('accountExportLocalSession failed', e);
      throw e;
    }
  }

  async accountExportAllSessions(workspacePath: string): Promise<number> {
    try {
      return await this.adapter.request<number>('account_export_all_sessions', {
        workspacePath,
      });
    } catch (e) {
      log.error('accountExportAllSessions failed', e);
      throw e;
    }
  }

  async accountImportRemoteSessions(workspacePath: string): Promise<string[]> {
    try {
      return await this.adapter.request<string[]>('account_import_remote_sessions', {
        workspacePath,
      });
    } catch (e) {
      log.error('accountImportRemoteSessions failed', e);
      throw e;
    }
  }

  /** Complete or resume a relay-imported session's lazy turn import. */
  async accountFetchSessionTurns(sessionId: string, workspacePath: string): Promise<boolean> {
    try {
      return await this.adapter.request<boolean>('account_fetch_session_turns', {
        sessionId,
        workspacePath,
      });
    } catch (e) {
      log.error('accountFetchSessionTurns failed', e);
      throw e;
    }
  }

  async accountExecuteOnDevice(
    targetDeviceId: string,
    content: string,
    sessionId?: string,
    agentType?: string,
    workspacePath?: string,
  ): Promise<void> {
    try {
      await this.adapter.request<void>('account_execute_on_device', {
        targetDeviceId,
        sessionId: sessionId ?? null,
        content,
        agentType: agentType ?? null,
        workspacePath: workspacePath ?? null,
      });
    } catch (e) {
      log.error('accountExecuteOnDevice failed', e);
      throw e;
    }
  }

  async accountAutoSync(
    isFirstLogin: boolean,
    workspacePath: string,
    configJson: string,
  ): Promise<AutoSyncResult> {
    try {
      return await this.adapter.request<AutoSyncResult>('account_auto_sync', {
        isFirstLogin,
        workspacePath,
        configJson,
      });
    } catch (e) {
      log.error('accountAutoSync failed', e);
      throw e;
    }
  }

  async accountListDevices(): Promise<AccountDeviceInfo[]> {
    try {
      return await this.adapter.request<AccountDeviceInfo[]>('account_list_devices');
    } catch (e) {
      log.error('accountListDevices failed', e);
      throw e;
    }
  }

  async accountDeleteDevice(targetDeviceId: string): Promise<void> {
    try {
      await this.adapter.request<void>('account_delete_device', { targetDeviceId });
    } catch (e) {
      log.error('accountDeleteDevice failed', e);
      throw e;
    }
  }

  async accountDeviceRpc(
    targetDeviceId: string,
    commandJson: string,
  ): Promise<string> {
    try {
      return await this.adapter.request<string>('account_device_rpc', {
        targetDeviceId,
        commandJson,
      });
    } catch (e) {
      log.error('accountDeviceRpc failed', e);
      throw e;
    }
  }

  async accountDelegateToPaired(correlationId: string): Promise<string> {
    try {
      return await this.adapter.request<string>('account_delegate_to_paired', {
        correlationId,
      });
    } catch (e) {
      log.warn('accountDelegateToPaired failed', e);
      throw e;
    }
  }
}

export const remoteConnectAPI = new RemoteConnectAPIService();
