 

import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';
import { openUrl } from '@tauri-apps/plugin-opener';
import { createLogger } from '@/shared/utils/logger';


const log = createLogger('SystemAPI');

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

   
  async checkForUpdates(): Promise<any> {
    try {
      return await api.invoke('check_for_updates', { 
        request: {} 
      });
    } catch (error) {
      throw createTauriCommandError('check_for_updates', error);
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

  async getCoworkWorkspacePath(): Promise<string> {
    try {
      return await api.invoke('get_cowork_workspace_path');
    } catch (error) {
      throw createTauriCommandError('get_cowork_workspace_path', error);
    }
  }

  async ensureCoworkSessionDirs(sessionId: string): Promise<{ artifactsPath: string; tmpPath: string }> {
    try {
      return await api.invoke('ensure_cowork_session_dirs', {
        request: {
          sessionId,
        }
      });
    } catch (error) {
      throw createTauriCommandError('ensure_cowork_session_dirs', error, { sessionId });
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
}


export const systemAPI = new SystemAPI();
