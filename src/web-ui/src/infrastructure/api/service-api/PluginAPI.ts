import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';

export interface PluginInfo {
  id: string;
  name: string;
  version?: string | null;
  description?: string | null;
  path: string;
  enabled: boolean;
  hasMcpConfig: boolean;
  mcpServerCount: number;
}

export interface ImportMcpServersResult {
  added: number;
  skipped: number;
  overwritten: number;
}

export class PluginAPI {
  async listPlugins(): Promise<PluginInfo[]> {
    try {
      return await api.invoke('list_plugins');
    } catch (error) {
      throw createTauriCommandError('list_plugins', error);
    }
  }

  async installPlugin(sourcePath: string): Promise<PluginInfo> {
    try {
      return await api.invoke('install_plugin', { sourcePath });
    } catch (error) {
      throw createTauriCommandError('install_plugin', error, { sourcePath });
    }
  }

  async uninstallPlugin(pluginId: string): Promise<string> {
    try {
      return await api.invoke('uninstall_plugin', { pluginId });
    } catch (error) {
      throw createTauriCommandError('uninstall_plugin', error, { pluginId });
    }
  }

  async setPluginEnabled(pluginId: string, enabled: boolean): Promise<string> {
    try {
      return await api.invoke('set_plugin_enabled', { pluginId, enabled });
    } catch (error) {
      throw createTauriCommandError('set_plugin_enabled', error, { pluginId, enabled });
    }
  }

  async importPluginMcpServers(pluginId: string, overwriteExisting: boolean): Promise<ImportMcpServersResult> {
    try {
      return await api.invoke('import_plugin_mcp_servers', { pluginId, overwriteExisting });
    } catch (error) {
      throw createTauriCommandError('import_plugin_mcp_servers', error, { pluginId, overwriteExisting });
    }
  }
}

export const pluginAPI = new PluginAPI();

