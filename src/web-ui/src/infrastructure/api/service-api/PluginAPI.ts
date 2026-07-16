import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';

export interface PluginStatusView {
  pluginId: string;
  name: string;
  version: string | null;
  source: string;
  scope: string;
  trustLevel: string;
  enabled: boolean;
  skillCount: number;
  diagnostics: string[];
}

export interface PluginStatusResponse {
  pluginsEnabled: boolean;
  plugins: PluginStatusView[];
  workspacePath?: string;
}

export interface SetPluginTrustRequest {
  pluginId: string;
  trusted: boolean;
}

export class PluginAPI {
  /**
   * Returns the current plugin discovery status for a workspace.
   */
  async getPluginStatus(workspacePath?: string): Promise<PluginStatusResponse> {
    try {
      return await api.invoke('get_plugin_status', {
        request: workspacePath ? { workspacePath } : {},
      });
    } catch (error) {
      throw createTauriCommandError('get_plugin_status', error, { workspacePath });
    }
  }

  /**
   * Enables or disables the plugin system globally.
   */
  async setPluginsEnabled(enabled: boolean): Promise<PluginStatusResponse> {
    try {
      return await api.invoke('set_plugins_enabled', {
        request: { enabled },
      });
    } catch (error) {
      throw createTauriCommandError('set_plugins_enabled', error, { enabled });
    }
  }

  /**
   * Sets trust for a specific plugin.
   */
  async setPluginTrust(pluginId: string, trusted: boolean): Promise<PluginStatusView> {
    try {
      return await api.invoke('set_plugin_trust', {
        request: { pluginId, trusted } as SetPluginTrustRequest,
      });
    } catch (error) {
      throw createTauriCommandError('set_plugin_trust', error, { pluginId, trusted });
    }
  }

  /**
   * Refreshes plugin discovery and returns updated status.
   */
  async refreshPlugins(workspacePath?: string): Promise<PluginStatusResponse> {
    try {
      return await api.invoke('refresh_plugins', {
        request: workspacePath ? { workspacePath } : {},
      });
    } catch (error) {
      throw createTauriCommandError('refresh_plugins', error, { workspacePath });
    }
  }
}

export const pluginAPI = new PluginAPI();
