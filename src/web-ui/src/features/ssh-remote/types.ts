/**
 * SSH Remote Feature - Types
 */

export interface SSHConnectionConfig {
  id: string;
  name: string;
  host: string;
  port: number;
  username: string;
  auth: SSHAuthMethod;
  defaultWorkspace?: string;
}

export type SSHAuthMethod =
  | { type: 'Password'; password: string }
  | { type: 'PrivateKey'; keyPath: string; passphrase?: string };

export type SavedAuthType =
  | { type: 'Password' }
  | { type: 'PrivateKey'; keyPath: string };

export interface SavedConnection {
  id: string;
  name: string;
  host: string;
  port: number;
  username: string;
  authType: SavedAuthType;
  defaultWorkspace?: string;
  lastConnected?: number;
}

export interface SSHConnectionResult {
  success: boolean;
  connectionId?: string;
  error?: string;
  serverInfo?: ServerInfo;
}

export interface ServerInfo {
  osType: string;
  hostname: string;
  homeDir: string;
}

export interface RemoteFileEntry {
  name: string;
  path: string;
  isDir: boolean;
  isFile: boolean;
  isSymlink: boolean;
  size?: number;
  modified?: number;
}

export interface RemoteTreeNode {
  name: string;
  path: string;
  isDir: boolean;
  children?: RemoteTreeNode[];
}

export interface RemoteWorkspace {
  connectionId: string;
  connectionName: string;
  remotePath: string;
  /** SSH `host` from connection profile; required for correct local session mirror paths. */
  sshHost?: string;
}

export interface SSHConfigEntry {
  host: string;
  hostname?: string;
  port?: number;
  user?: string;
  identityFile?: string;
  agent?: boolean;
}

export interface SSHConfigLookupResult {
  found: boolean;
  config?: SSHConfigEntry;
}
