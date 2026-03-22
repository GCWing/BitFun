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

export interface LanNetworkInfo {
  local_ip: string;
  gateway_ip: string | null;
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

class RemoteConnectAPIService {
  private adapter = getTransportAdapter();

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

  async startConnection(method: string, customServerUrl?: string): Promise<ConnectionResult> {
    try {
      return await this.adapter.request<ConnectionResult>('remote_connect_start', {
        request: { method, custom_server_url: customServerUrl ?? null },
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
}

export const remoteConnectAPI = new RemoteConnectAPIService();
