/**
 * SSH Remote Feature - React Context Provider
 */

import React, { createContext, useContext, useState, useCallback, useEffect, useRef } from 'react';
import { createLogger } from '@/shared/utils/logger';
import { workspaceManager } from '@/infrastructure/services/business/workspaceManager';
import { WorkspaceKind } from '@/shared/types/global-state';
import type { SSHConnectionConfig, RemoteWorkspace } from './types';
import { sshApi } from './sshApi';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import { normalizeRemoteWorkspacePath } from '@/shared/utils/pathUtils';

const log = createLogger('SSHRemoteProvider');

export type ConnectionStatus = 'disconnected' | 'connecting' | 'connected' | 'error';

interface SSHContextValue {
  // Connection state
  status: ConnectionStatus;
  isConnected: boolean;
  isConnecting: boolean;
  connectionId: string | null;
  connectionConfig: SSHConnectionConfig | null;
  remoteWorkspace: RemoteWorkspace | null;
  connectionError: string | null;

  // Per-workspace connection statuses (keyed by connectionId)
  workspaceStatuses: Record<string, ConnectionStatus>;

  // UI state
  showConnectionDialog: boolean;
  showFileBrowser: boolean;
  error: string | null;

  // Actions
  connect: (connectionId: string, config: SSHConnectionConfig) => Promise<void>;
  disconnect: () => Promise<void>;
  openWorkspace: (path: string) => Promise<void>;
  closeWorkspace: () => Promise<void>;

  // UI actions
  setShowConnectionDialog: (show: boolean) => void;
  setShowFileBrowser: (show: boolean) => void;
  clearError: () => void;
}

export const SSHContext = createContext<SSHContextValue | null>(null);

export const useSSHRemoteContext = () => {
  const context = useContext(SSHContext);
  if (!context) {
    throw new Error('useSSHRemoteContext must be used within SSHRemoteProvider');
  }
  return context;
};

interface SSHRemoteProviderProps {
  children: React.ReactNode;
}

export const SSHRemoteProvider: React.FC<SSHRemoteProviderProps> = ({ children }) => {
  const [status, setStatus] = useState<ConnectionStatus>('disconnected');
  const [isConnected, setIsConnected] = useState(false);
  const [isConnecting, setIsConnecting] = useState(false);
  const [connectionId, setConnectionId] = useState<string | null>(null);
  const [connectionConfig, setConnectionConfig] = useState<SSHConnectionConfig | null>(null);
  const [remoteWorkspace, setRemoteWorkspace] = useState<RemoteWorkspace | null>(null);
  const [showConnectionDialog, setShowConnectionDialog] = useState(false);
  const [showFileBrowser, setShowFileBrowser] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [connectionError, setConnectionError] = useState<string | null>(null);
  // Per-workspace connection statuses (keyed by connectionId)
  const [workspaceStatuses, setWorkspaceStatuses] = useState<Record<string, ConnectionStatus>>({});
  const heartbeatInterval = useRef<number | null>(null);

  const setWorkspaceStatus = useCallback((connId: string, st: ConnectionStatus) => {
    setWorkspaceStatuses(prev => ({ ...prev, [connId]: st }));
  }, []);

  // Wait for workspace manager to finish loading, then check remote workspaces
  useEffect(() => {
    const state = workspaceManager.getState();
    if (!state.loading) {
      // Already loaded — kick off immediately
      void checkRemoteWorkspace();
      return;
    }
    // Wait for loading to complete
    const unsubscribe = workspaceManager.addEventListener(event => {
      if (event.type === 'workspace:loading' && !event.loading) {
        unsubscribe();
        void checkRemoteWorkspace();
      }
    });
    return unsubscribe;
    // checkRemoteWorkspace is defined below but stable (no deps change it)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Cleanup heartbeat on unmount
  useEffect(() => {
    return () => {
      if (heartbeatInterval.current) {
        clearInterval(heartbeatInterval.current);
      }
    };
  }, []);

  // Try to reconnect a single remote workspace with retries.
  // Returns the reconnected workspace info on success, false on failure.
  // Waits RETRY_WAIT_MS between each attempt (fixed, not exponential).
  const RETRY_WAIT_MS = 10_000;

  const tryReconnectWithRetry = async (
    workspace: RemoteWorkspace,
    maxRetries: number,
    timeoutMs: number
  ): Promise<false | { workspace: RemoteWorkspace; connectionId: string }> => {
    log.info('tryReconnectWithRetry: starting', { workspace, maxRetries, timeoutMs });

    const savedConnections = await sshApi.listSavedConnections();
    const savedConn = savedConnections.find(c => c.id === workspace.connectionId);

    if (!savedConn) {
      log.warn('No saved connection found for workspace', { connectionId: workspace.connectionId });
      return false;
    }

    // Determine auth method from tagged enum
    let authMethod: SSHConnectionConfig['auth'] | null = null;
    if (savedConn.authType.type === 'PrivateKey') {
      authMethod = { type: 'PrivateKey', keyPath: savedConn.authType.keyPath };
    } else if (savedConn.authType.type === 'Agent') {
      authMethod = { type: 'Agent' };
    } else {
      // Password auth cannot auto-reconnect because BitFun intentionally does not
      // persist passwords. The user must reconnect manually after restarting the app.
      log.warn('Skipping auto-reconnect: password auth requires user input', { connectionId: workspace.connectionId });
      return false;
    }

    const reconnectConfig: SSHConnectionConfig = {
      id: savedConn.id,
      name: savedConn.name,
      host: savedConn.host,
      port: savedConn.port,
      username: savedConn.username,
      auth: authMethod,
    };

    for (let attempt = 1; attempt <= maxRetries; attempt++) {
      try {
        log.info(`Attempting to reconnect (${attempt}/${maxRetries})`, {
          connectionId: workspace.connectionId,
          host: reconnectConfig.host,
        });

        const connectWithTimeout = async (): Promise<{ connectionId: string }> => {
          const result = await sshApi.connect(reconnectConfig);
          if (!result.success || !result.connectionId) {
            throw new Error(result.error || 'Connection failed');
          }
          return { connectionId: result.connectionId };
        };

        const timeoutPromise = new Promise<never>((_, reject) => {
          setTimeout(() => reject(new Error('Connection timeout')), timeoutMs);
        });

        const result = await Promise.race([connectWithTimeout(), timeoutPromise]);

        // Successfully connected — open the workspace in SSH state manager
        await sshApi.openWorkspace(result.connectionId, workspace.remotePath);
        const reconnectedWorkspace: RemoteWorkspace = {
          connectionId: result.connectionId,
          connectionName: savedConn.name,
          remotePath: workspace.remotePath,
        };

        log.info('Successfully reconnected to remote workspace', {
          originalConnectionId: workspace.connectionId,
          newConnectionId: result.connectionId,
        });
        return { workspace: reconnectedWorkspace, connectionId: result.connectionId };
      } catch (err) {
        log.warn(`Reconnect attempt ${attempt}/${maxRetries} failed`, { connectionId: workspace.connectionId, error: err });
        if (attempt < maxRetries) {
          // Fixed 10-second wait between retries
          await new Promise(resolve => setTimeout(resolve, RETRY_WAIT_MS));
        }
      }
    }

    return false;
  };

  const checkRemoteWorkspace = async () => {
    try {
      // ── Collect all remote workspaces to reconnect ──────────────────────
      const allWorkspaces = Array.from(workspaceManager.getState().openedWorkspaces.values());
      const openedRemote = allWorkspaces.filter(
        ws => ws.workspaceKind === WorkspaceKind.Remote && ws.connectionId
      );

      // Also check legacy single-workspace persisted in app_state
      let legacyWorkspace: RemoteWorkspace | null = null;
      try {
        legacyWorkspace = await sshApi.getWorkspaceInfo();
      } catch {
        // Ignore
      }

      // Key by connection + path so two servers at the same remote path stay distinct.
      const remoteWorkspaceDedupKey = (cid: string, rp: string) => `${cid}\n${rp}`;
      const toReconnect = new Map<string, RemoteWorkspace>();

      for (const ws of openedRemote) {
        if (!ws.connectionId) continue;
        const rp = normalizeRemoteWorkspacePath(ws.rootPath);
        toReconnect.set(remoteWorkspaceDedupKey(ws.connectionId, rp), {
          connectionId: ws.connectionId,
          connectionName: ws.connectionName || 'Remote',
          remotePath: rp,
        });
      }

      // Add legacy workspace if it isn't already covered
      if (legacyWorkspace?.connectionId) {
        const leg = normalizeRemoteWorkspacePath(legacyWorkspace.remotePath);
        const k = remoteWorkspaceDedupKey(legacyWorkspace.connectionId, leg);
        if (!toReconnect.has(k)) {
          toReconnect.set(k, { ...legacyWorkspace, remotePath: leg });
        }
      }

      if (toReconnect.size === 0) {
        log.info('checkRemoteWorkspace: no remote workspaces to reconnect');
        return;
      }

      log.info(`checkRemoteWorkspace: found ${toReconnect.size} remote workspace(s)`);

      // Mark all as 'connecting' immediately so the UI shows the pending state
      const initialStatuses: Record<string, ConnectionStatus> = {};
      for (const [, ws] of toReconnect) {
        initialStatuses[ws.connectionId] = 'connecting';
      }
      setWorkspaceStatuses(prev => ({ ...prev, ...initialStatuses }));

      // ── Process each workspace ──────────────────────────────────────────
      for (const [, workspace] of toReconnect) {
        const isAlreadyOpened = openedRemote.some(
          ws =>
            normalizeRemoteWorkspacePath(ws.rootPath) ===
            normalizeRemoteWorkspacePath(workspace.remotePath)
        );

        // Check if SSH is already live
        const alreadyConnected = await sshApi.isConnected(workspace.connectionId).catch(() => false);

        if (alreadyConnected) {
          log.info('Remote workspace already connected', { connectionId: workspace.connectionId });
          // Register with SSH state manager (idempotent)
          await sshApi.openWorkspace(workspace.connectionId, workspace.remotePath).catch(() => {});
          setWorkspaceStatus(workspace.connectionId, 'connected');
          setIsConnected(true);
          setConnectionId(workspace.connectionId);
          setRemoteWorkspace(workspace);
          startHeartbeat(workspace.connectionId);

          if (!isAlreadyOpened) {
            await workspaceManager.openRemoteWorkspace(workspace).catch(() => {});
          }
          // Re-initialize sessions now that the workspace is registered in the state manager
          void flowChatStore.initializeFromDisk(workspace.remotePath).catch(() => {});
          continue;
        }

        // Not connected — attempt auto-reconnect
        log.info('Remote workspace disconnected, attempting auto-reconnect', {
          connectionId: workspace.connectionId,
          remotePath: workspace.remotePath,
        });

        const result = await tryReconnectWithRetry(workspace, 5, 5000);

        if (result !== false) {
          log.info('Reconnection successful', { newConnectionId: result.connectionId });
          setWorkspaceStatus(result.workspace.connectionId, 'connected');
          setIsConnected(true);
          setConnectionId(result.connectionId);
          setRemoteWorkspace(result.workspace);
          startHeartbeat(result.connectionId);

          if (!isAlreadyOpened) {
            await workspaceManager.openRemoteWorkspace(result.workspace).catch(() => {});
          }
          // Re-initialize sessions now that the workspace is registered in the state manager
          void flowChatStore.initializeFromDisk(result.workspace.remotePath).catch(() => {});
        } else {
          // Reconnection failed (or skipped for password auth) — remove the workspace
          // from the sidebar. Password-auth workspaces can never auto-reconnect, and
          // showing a permanently-broken entry would confuse the user.
          log.warn('Auto-reconnect failed, removing workspace from sidebar', {
            connectionId: workspace.connectionId,
          });
          await workspaceManager.removeRemoteWorkspace(workspace.connectionId).catch(() => {});
        }
      }
    } catch (e) {
      log.error('checkRemoteWorkspace failed', e);
    }
  };

  const statusRef = useRef<ConnectionStatus>(status);
  statusRef.current = status;

  const startHeartbeat = (connId: string) => {
    if (heartbeatInterval.current) {
      clearInterval(heartbeatInterval.current);
    }

    heartbeatInterval.current = window.setInterval(async () => {
      try {
        const connected = await sshApi.isConnected(connId);
        if (!connected && statusRef.current === 'connected') {
          handleConnectionLost(connId);
        }
      } catch {
        // Ignore heartbeat errors
      }
    }, 30000);
  };

  const handleConnectionLost = (connId: string) => {
    log.warn('Remote connection lost, attempting auto-reconnect...');
    setStatus('error');
    setWorkspaceStatus(connId, 'error');
    setConnectionError('Connection lost. Attempting to reconnect...');
    setIsConnected(false);
    if (heartbeatInterval.current) {
      clearInterval(heartbeatInterval.current);
      heartbeatInterval.current = null;
    }
    // Attempt auto-reconnect in background
    void checkRemoteWorkspace();
  };

  const connect = useCallback(async (_connId: string, config: SSHConnectionConfig) => {
    log.debug('SSH connect called', { host: config.host });
    setStatus('connecting');
    setIsConnecting(true);
    setConnectionError(null);
    setError(null);

    try {
      const result = await sshApi.connect(config);
      log.debug('SSH connect result', { success: result.success, connectionId: result.connectionId, error: result.error });

      if (result.success && result.connectionId) {
        log.info('SSH connection successful', { connectionId: result.connectionId });
        setStatus('connected');
        setIsConnected(true);
        setConnectionId(result.connectionId);
        setConnectionConfig(config);
        setShowConnectionDialog(false);
        setShowFileBrowser(true);
        startHeartbeat(result.connectionId);
      } else {
        log.warn('SSH connection failed', { error: result.error });
        setStatus('error');
        const errorMsg = result.error || 'Connection failed';
        setConnectionError(errorMsg);
        throw new Error(errorMsg);
      }
    } catch (e) {
      log.error('SSH connection exception', e);
      if (e instanceof Error) {
        setStatus('error');
        setConnectionError(e.message);
        throw e;
      }
      const errorMsg = e instanceof Error ? e.message : 'Connection failed';
      setStatus('error');
      setConnectionError(errorMsg);
      throw new Error(errorMsg);
    } finally {
      setIsConnecting(false);
    }
  }, []);

  const disconnect = useCallback(async () => {
    const currentRemoteWorkspace = remoteWorkspace;
    const currentConnectionId = connectionId;

    if (heartbeatInterval.current) {
      clearInterval(heartbeatInterval.current);
      heartbeatInterval.current = null;
    }

    if (currentConnectionId) {
      try {
        await sshApi.disconnect(currentConnectionId);
      } catch {
        // Ignore disconnect errors
      }
    }
    setStatus('disconnected');
    setConnectionId(null);
    setConnectionConfig(null);
    setRemoteWorkspace(null);
    setIsConnected(false);
    setShowFileBrowser(false);

    if (currentRemoteWorkspace) {
      setWorkspaceStatus(currentRemoteWorkspace.connectionId, 'disconnected');
      try {
        await workspaceManager.removeRemoteWorkspace(currentRemoteWorkspace.connectionId);
      } catch {
        // Ignore errors
      }
    }
  }, [connectionId, remoteWorkspace, setWorkspaceStatus]);

  const openWorkspace = useCallback(async (pingPath: string) => {
    if (!connectionId) {
      throw new Error('Not connected');
    }
    const connName = connectionConfig?.name || 'Remote';
    const remotePath = normalizeRemoteWorkspacePath(pingPath);
    await sshApi.openWorkspace(connectionId, remotePath);
    const remoteWs = {
      connectionId,
      connectionName: connName,
      remotePath,
    };
    setRemoteWorkspace(remoteWs);
    setShowFileBrowser(false);
    setWorkspaceStatus(connectionId, 'connected');

    await workspaceManager.openRemoteWorkspace(remoteWs);
  }, [connectionId, connectionConfig, setWorkspaceStatus]);

  const closeWorkspace = useCallback(async () => {
    const currentRemoteWorkspace = remoteWorkspace;

    try {
      await sshApi.closeWorkspace();
    } catch {
      // Ignore errors
    }
    setRemoteWorkspace(null);
    setShowFileBrowser(true);

    if (currentRemoteWorkspace) {
      setWorkspaceStatus(currentRemoteWorkspace.connectionId, 'disconnected');
      try {
        await workspaceManager.removeRemoteWorkspace(currentRemoteWorkspace.connectionId);
      } catch {
        // Ignore errors
      }
    }
  }, [remoteWorkspace, setWorkspaceStatus]);

  const clearError = useCallback(() => {
    setError(null);
    setConnectionError(null);
  }, []);

  const value: SSHContextValue = {
    status,
    isConnected,
    isConnecting,
    connectionId,
    connectionConfig,
    remoteWorkspace,
    connectionError,
    workspaceStatuses,
    showConnectionDialog,
    showFileBrowser,
    error,
    connect,
    disconnect,
    openWorkspace,
    closeWorkspace,
    setShowConnectionDialog,
    setShowFileBrowser,
    clearError,
  };

  return <SSHContext.Provider value={value}>{children}</SSHContext.Provider>;
};

export default SSHRemoteProvider;
