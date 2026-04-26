/**
 * SSH Remote Feature - API Service
 */

import type {
  SSHConnectionConfig,
  SSHConnectionResult,
  SavedConnection,
  RemoteFileEntry,
  RemoteTreeNode,
  RemoteWorkspace,
  SSHConfigLookupResult,
  SSHConfigEntry,
  ServerInfo,
} from './types';

// API adapter for Tauri/Server Mode compatibility
import { api } from '@/infrastructure/api/service-api/ApiClient';

export const sshApi = {
  // === Connection Management ===

  /**
   * List all saved SSH connections
   */
  async listSavedConnections(): Promise<SavedConnection[]> {
    return api.invoke<SavedConnection[]>('ssh_list_saved_connections', {});
  },

  /**
   * Save SSH connection configuration
   */
  async saveConnection(config: SSHConnectionConfig): Promise<void> {
    return api.invoke('ssh_save_connection', { config });
  },

  /**
   * Delete saved SSH connection
   */
  async deleteConnection(connectionId: string): Promise<void> {
    return api.invoke('ssh_delete_connection', { connectionId });
  },

  /**
   * Whether a password is stored in the local vault for this saved connection (password auth auto-reconnect).
   */
  async hasStoredPassword(connectionId: string): Promise<boolean> {
    return api.invoke<boolean>('ssh_has_stored_password', { connectionId });
  },

  /**
   * Connect to remote SSH server
   */
  async connect(config: SSHConnectionConfig): Promise<SSHConnectionResult> {
    return api.invoke<SSHConnectionResult>('ssh_connect', { config });
  },

  /**
   * Disconnect from SSH server
   */
  async disconnect(connectionId: string): Promise<void> {
    return api.invoke('ssh_disconnect', { connectionId });
  },

  /**
   * Disconnect all SSH connections
   */
  async disconnectAll(): Promise<void> {
    return api.invoke('ssh_disconnect_all', {});
  },

  /**
   * Check if connected to SSH server
   */
  async isConnected(connectionId: string): Promise<boolean> {
    return api.invoke<boolean>('ssh_is_connected', { connectionId });
  },

  /**
   * Server info for an active connection; may probe `echo ~` / `$HOME` if `homeDir` was missing.
   */
  async getServerInfo(connectionId: string): Promise<ServerInfo | null> {
    return api.invoke<ServerInfo | null>('ssh_get_server_info', { connectionId });
  },

  /**
   * Get SSH config for a host from ~/.ssh/config
   */
  async getSSHConfig(host: string): Promise<SSHConfigLookupResult> {
    return api.invoke<SSHConfigLookupResult>('ssh_get_config', { host });
  },

  /**
   * List all hosts from ~/.ssh/config
   */
  async listSSHConfigHosts(): Promise<SSHConfigEntry[]> {
    return api.invoke<SSHConfigEntry[]>('ssh_list_config_hosts', {});
  },

  // === Remote File Operations ===

  /**
   * Read file content from remote server
   */
  async readFile(connectionId: string, path: string): Promise<string> {
    return api.invoke<string>('remote_read_file', { connectionId, path });
  },

  /**
   * Write content to remote file
   */
  async writeFile(connectionId: string, path: string, content: string): Promise<void> {
    return api.invoke('remote_write_file', { connectionId, path, content });
  },

  /**
   * Check if remote path exists
   */
  async exists(connectionId: string, path: string): Promise<boolean> {
    return api.invoke<boolean>('remote_exists', { connectionId, path });
  },

  /**
   * List directory contents
   */
  async readDir(connectionId: string, path: string): Promise<RemoteFileEntry[]> {
    return api.invoke<RemoteFileEntry[]>('remote_read_dir', { connectionId, path });
  },

  /**
   * Get remote file tree
   */
  async getTree(
    connectionId: string,
    path: string,
    depth?: number
  ): Promise<RemoteTreeNode> {
    return api.invoke<RemoteTreeNode>('remote_get_tree', { connectionId, path, depth });
  },

  /**
   * Create remote directory
   */
  async createDir(
    connectionId: string,
    path: string,
    recursive: boolean
  ): Promise<void> {
    return api.invoke('remote_create_dir', { connectionId, path, recursive });
  },

  /**
   * Remove remote file or directory
   */
  async remove(
    connectionId: string,
    path: string,
    recursive: boolean
  ): Promise<void> {
    return api.invoke('remote_remove', { connectionId, path, recursive });
  },

  /**
   * Rename/move remote file or directory
   */
  async rename(
    connectionId: string,
    oldPath: string,
    newPath: string
  ): Promise<void> {
    return api.invoke('remote_rename', { connectionId, oldPath, newPath });
  },

  /**
   * Download a remote file to a local filesystem path (desktop; binary-safe).
   */
  async downloadToLocalPath(
    connectionId: string,
    remotePath: string,
    localPath: string
  ): Promise<void> {
    return api.invoke('remote_download_to_local_path', {
      connectionId,
      remotePath,
      localPath,
    });
  },

  /**
   * Upload a local file to a remote path (desktop; binary-safe).
   */
  async uploadFromLocalPath(
    connectionId: string,
    localPath: string,
    remotePath: string
  ): Promise<void> {
    return api.invoke('remote_upload_from_local_path', {
      connectionId,
      localPath,
      remotePath,
    });
  },

  /**
   * Execute command on remote server
   */
  async execute(
    connectionId: string,
    command: string
  ): Promise<[string, string, number]> {
    return api.invoke<[string, string, number]>('remote_execute', { connectionId, command });
  },

  // === Remote Workspace ===

  /**
   * Open remote workspace
   */
  async openWorkspace(connectionId: string, remotePath: string): Promise<void> {
    return api.invoke('remote_open_workspace', { connectionId, remotePath });
  },

  /**
   * Close remote workspace
   */
  async closeWorkspace(): Promise<void> {
    return api.invoke('remote_close_workspace', {});
  },

  /**
   * Remove one persisted remote workspace restore entry without requiring it to be active.
   */
  async removeWorkspace(connectionId: string, remotePath: string): Promise<void> {
    return api.invoke('remote_remove_workspace', { connectionId, remotePath });
  },

  /**
   * Get current remote workspace info
   */
  async getWorkspaceInfo(): Promise<RemoteWorkspace | null> {
    return api.invoke<RemoteWorkspace | null>('remote_get_workspace_info', {});
  },
};
