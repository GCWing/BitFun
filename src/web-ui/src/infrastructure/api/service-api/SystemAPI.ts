 

import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';
import { openUrl } from '@tauri-apps/plugin-opener';
import { disable as autostartDisable, enable as autostartEnable, isEnabled as autostartIsEnabled } from '@tauri-apps/plugin-autostart';
import { createLogger } from '@/shared/utils/logger';


const log = createLogger('SystemAPI');

/** Matches `check_for_updates` / `CheckForUpdatesResponse` from desktop `system_api.rs` (camelCase). */
export interface CheckForUpdatesResponse {
  updateAvailable: boolean;
  currentVersion: string;
  latestVersion: string | null;
  releaseNotes: string | null;
  releaseDate: string | null;
}

export class SystemAPI {
   
  async getSystemInfo(): Promise<any> {
    try {
      return await api.invoke('get_system_info', { 
        request: {} 
      });
    } catch (error) {
      throw createTauriCommandError('get_system_info', error);
    }
  }

   
  async getAppVersion(): Promise<string> {
    try {
      return await api.invoke('get_app_version', { 
        request: {} 
      });
    } catch (error) {
      throw createTauriCommandError('get_app_version', error);
    }
  }

   
  async checkForUpdates(): Promise<CheckForUpdatesResponse> {
    try {
      return await api.invoke('check_for_updates', { 
        request: {} 
      });
    } catch (error) {
      throw createTauriCommandError('check_for_updates', error);
    }
  }

  /** Desktop only: download and install update after user confirms (calls updater again). */
  async installUpdate(): Promise<void> {
    try {
      await api.invoke('install_update', {
        request: {}
      });
    } catch (error) {
      throw createTauriCommandError('install_update', error);
    }
  }

  /** Desktop only: restart the app after an update has been installed. */
  async restartApp(): Promise<void> {
    try {
      await api.invoke('restart_app', {
        request: {}
      });
    } catch (error) {
      throw createTauriCommandError('restart_app', error);
    }
  }

   
  async openExternal(url: string): Promise<void> {
    try {
      await openUrl(url);
    } catch (error) {
      log.error('Failed to open external URL', { url, error });
      throw new Error(`Failed to open external URL: ${error}`);
    }
  }

   
  async showInFolder(path: string): Promise<void> {
    try {
      await api.invoke('show_in_folder', { 
        request: { path } 
      });
    } catch (error) {
      throw createTauriCommandError('show_in_folder', error, { path });
    }
  }

   
  async checkPathExists(path: string): Promise<boolean> {
    try {
      return await api.invoke('check_path_exists', { 
        request: { path } 
      });
    } catch (error) {
      throw createTauriCommandError('check_path_exists', error, { path });
    }
  }

   
  async getClipboard(): Promise<string> {
    try {
      return await api.invoke('get_clipboard', { 
        request: {} 
      });
    } catch (error) {
      throw createTauriCommandError('get_clipboard', error);
    }
  }

   
  async setClipboard(text: string): Promise<void> {
    try {
      await api.invoke('set_clipboard', { 
        request: { text } 
      });
    } catch (error) {
      throw createTauriCommandError('set_clipboard', error, { text });
    }
  }

   
  async checkCommandExists(command: string): Promise<{ exists: boolean; path: string | null }> {
    try {
      return await api.invoke('check_command_exists', { command });
    } catch (error) {
      throw createTauriCommandError('check_command_exists', error, { command });
    }
  }

   
  async checkCommandsExist(commands: string[]): Promise<Array<[string, { exists: boolean; path: string | null }]>> {
    try {
      return await api.invoke('check_commands_exist', { commands });
    } catch (error) {
      throw createTauriCommandError('check_commands_exist', error, { commands });
    }
  }

  async setMacosEditMenuMode(mode: 'system' | 'renderer'): Promise<void> {
    try {
      await api.invoke('set_macos_edit_menu_mode', {
        request: { mode }
      });
    } catch (error) {
      throw createTauriCommandError('set_macos_edit_menu_mode', error, { mode });
    }
  }

  /** Desktop only: whether the app is registered to launch at OS login. */
  async getLaunchAtLoginEnabled(): Promise<boolean> {
    if (typeof window === 'undefined' || !('__TAURI__' in window)) {
      return false;
    }
    try {
      return await autostartIsEnabled();
    } catch (error) {
      log.error('Failed to read launch-at-login state', error);
      throw createTauriCommandError('autostart_is_enabled', error);
    }
  }

  /** Desktop only: send an OS-level desktop notification. */
  async sendSystemNotification(title: string, body?: string): Promise<void> {
    if (typeof window === 'undefined' || !('__TAURI__' in window)) {
      return;
    }
    try {
      await api.invoke('send_system_notification', {
        request: { title, body: body ?? null }
      });
    } catch (error) {
      log.warn('Failed to send system notification', { title, error });
    }
  }

  /** Desktop only: register or unregister launch at OS login. */
  async setLaunchAtLoginEnabled(enabled: boolean): Promise<void> {
    if (typeof window === 'undefined' || !('__TAURI__' in window)) {
      return;
    }
    try {
      if (enabled) {
        await autostartEnable();
      } else {
        await autostartDisable();
      }
    } catch (error) {
      log.error('Failed to set launch-at-login', { enabled, error });
      throw createTauriCommandError('autostart_set', error, { enabled });
    }
  }
}


export const systemAPI = new SystemAPI();
