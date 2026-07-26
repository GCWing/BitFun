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
  proxyJump?: string;
  container?: ContainerWorkspaceConfig;
  options?: SSHConnectionOptions;
}

export type ContainerAccess = 'sshd' | 'docker-exec' | 'auto';

export interface SSHConnectionOptions {
  connectTimeoutSecs: number;
  authTimeoutSecs: number;
  authAttempts: number;
  connectAttempts: number;
}

export interface ContainerWorkspaceConfig {
  name: string;
  access: ContainerAccess;
  local: boolean;
  dockerPath: string;
  shell: string;
  user?: string;
  interactive: boolean;
}

export type SSHAuthMethod =
  | { type: 'Password'; password: string }
  | { type: 'PrivateKey'; keyPath: string; passphrase?: string; certificatePath?: string }
  | { type: 'Agent'; keyFingerprint?: string; fallbackKeyPath?: string }
  | { type: 'KeyboardInteractive'; responses: string[] };

export type SavedAuthType =
  | { type: 'Password' }
  | { type: 'PrivateKey'; keyPath: string; certificatePath?: string }
  | { type: 'Agent'; keyFingerprint?: string; fallbackKeyPath?: string }
  | { type: 'KeyboardInteractive' };

export interface SavedConnection {
  id: string;
  name: string;
  host: string;
  port: number;
  username: string;
  authType: SavedAuthType;
  defaultWorkspace?: string;
  lastConnected?: number;
  proxyJump?: string;
  container?: ContainerWorkspaceConfig;
  options?: SSHConnectionOptions;
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
  certificateFile?: string;
  agent?: boolean;
  proxyJump?: string;
}

export interface SSHConfigLookupResult {
  found: boolean;
  config?: SSHConfigEntry;
}

export interface DockerContainerInfo {
  id: string;
  name: string;
  image: string;
  status: string;
  state: string;
}

export interface ConnectionTestStage {
  id: string;
  label: string;
  success: boolean;
  error?: string;
}

export interface ConnectionTestReport {
  success: boolean;
  stages: ConnectionTestStage[];
  serverInfo?: ServerInfo;
  resolvedContainerAccess?: ContainerAccess;
}
