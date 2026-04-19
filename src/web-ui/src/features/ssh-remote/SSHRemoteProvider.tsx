/**
 * SSH Remote Feature - React Context Provider
 */
import React, { useState, useCallback, useEffect, useRef } from 'react';
import { createLogger } from '@/shared/utils/logger';
import { workspaceManager } from '@/infrastructure/services/business/workspaceManager';
import { WorkspaceKind } from '@/shared/types/global-state';
import type { SSHConnectionConfig, RemoteWorkspace } from './types';
import { sshApi } from './sshApi';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import { normalizeRemoteWorkspacePath } from '@/shared/utils/pathUtils';
import {
  SSHContext,
  type ConnectionStatus,
  type SSHContextValue,
} from './SSHRemoteContext';
import { SSHConnectionDialog } from './SSHConnectionDialog';
import { RemoteFileBrowser } from './RemoteFileBrowser';

const log = createLogger('SSHRemoteProvider');

/** Match opened `WorkspaceInfo` so list_sessions maps to ~/.bitfun/remote_ssh/... */
function sshHostForRemoteWorkspace(connectionId: string, remotePath: string): string | undefined {
  const norm = normalizeRemoteWorkspacePath(remotePath);
  const cid = connectionId.trim();
  for (const w of workspaceManager.getState().openedWorkspaces.values()) {
    if (w.workspaceKind !== WorkspaceKind.Remote) continue;
    if ((w.connectionId ?? '').trim() !== cid) continue;
    if (normalizeRemoteWorkspacePath(w.rootPath) === norm) {
      const h = w.sshHost?.trim();
      if (h) return h;
    }
  }
  return undefined;
}

/** After parallel reconnects: prefer the user's active remote workspace, else last in sidebar order (matches legacy serial last-write). */
function pickGlobalRemoteAfterReconnect(
  connected: Array<{ workspace: RemoteWorkspace; connectionId: string }>,
  orderedList: RemoteWorkspace[]
): { workspace: RemoteWorkspace; connectionId: string } | null {
  if (connected.length === 0) return null;
  const st = workspaceManager.getState();
  const aid = st.activeWorkspaceId;
  if (aid) {
    const aw = st.openedWorkspaces.get(aid);
    if (aw && aw.workspaceKind === WorkspaceKind.Remote && aw.connectionId) {
      const arp = normalizeRemoteWorkspacePath(aw.rootPath);
      const hit = connected.find(
        c =>
          c.connectionId === aw.connectionId &&
          normalizeRemoteWorkspacePath(c.workspace.remotePath) === arp
      );
      if (hit) return hit;
    }
  }
  for (let i = orderedList.length - 1; i >= 0; i--) {
    const ws = orderedList[i];
    const hit = connected.find(
      c =>
        c.connectionId === ws.connectionId &&
        normalizeRemoteWorkspacePath(c.workspace.remotePath) ===
          normalizeRemoteWorkspacePath(ws.remotePath)
    );
    if (hit) return hit;
  }
  return connected[connected.length - 1] ?? null;
}

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
  /** Fallback only when home cannot be resolved (never use literal `~` for SFTP). */
  const [remoteFileBrowserInitialPath, setRemoteFileBrowserInitialPath] = useState('/tmp');
  // Per-workspace connection statuses (keyed by connectionId)
  const [workspaceStatuses, setWorkspaceStatuses] = useState<Record<string, ConnectionStatus>>({});
  const heartbeatInterval = useRef<number | null>(null);
  const startHeartbeatRef = useRef<(connId: string) => void>(() => {});
  const checkRemoteWorkspaceRef = useRef<() => Promise<void>>(async () => {});

  const setWorkspaceStatus = useCallback((connId: string, st: ConnectionStatus) => {
    setWorkspaceStatuses(prev => ({ ...prev, [connId]: st }));
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

  const tryReconnectWithRetry = useCallback(async (
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

    // Determine auth method from tagged enum (password uses empty string; backend fills from vault)
    let authMethod: SSHConnectionConfig['auth'] | null = null;
    if (savedConn.authType.type === 'PrivateKey') {
      authMethod = { type: 'PrivateKey', keyPath: savedConn.authType.keyPath };
    } else {
      // Caller must only invoke password reconnect when vault has a password (see checkRemoteWorkspace).
      authMethod = { type: 'Password', password: '' };
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
          sshHost: reconnectConfig.host?.trim() || workspace.sshHost?.trim() || undefined,
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
  }, []);

  const statusRef = useRef<ConnectionStatus>(status);
  statusRef.current = status;

  const handleConnectionLost = useCallback((connId: string) => {
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
    void checkRemoteWorkspaceRef.current();
  }, [setWorkspaceStatus]);

  const startHeartbeat = useCallback((connId: string) => {
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
  }, [handleConnectionLost]);
  startHeartbeatRef.current = startHeartbeat;

  const checkRemoteWorkspace = useCallback(async () => {
    try {
      // ── Collect all remote workspaces to reconnect ──────────────────────
      const wmState0 = workspaceManager.getState();
      const allWorkspaces = Array.from(wmState0.openedWorkspaces.values());
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
          sshHost: ws.sshHost?.trim() || undefined,
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

      const reconnectList = Array.from(toReconnect.values());
      const savedConnectionsList = await sshApi.listSavedConnections();

      const skipPasswordAutoReconnect = new Set<string>();
      for (const ws of reconnectList) {
        const sc = savedConnectionsList.find(c => c.id === ws.connectionId);
        if (sc?.authType.type === 'Password') {
          let hasVault = false;
          try {
            hasVault = await sshApi.hasStoredPassword(sc.id);
          } catch {
            hasVault = false;
          }
          if (!hasVault) {
            skipPasswordAutoReconnect.add(ws.connectionId);
          }
        }
      }

      const initialStatuses: Record<string, ConnectionStatus> = {};
      for (const [, ws] of toReconnect) {
        initialStatuses[ws.connectionId] = skipPasswordAutoReconnect.has(ws.connectionId)
          ? 'error'
          : 'connecting';
      }
      setWorkspaceStatuses(prev => ({ ...prev, ...initialStatuses }));

      type ConnectedEntry = { workspace: RemoteWorkspace; connectionId: string };
      const results = await Promise.all(
        reconnectList.map(async workspace => {
          const isAlreadyOpened = openedRemote.some(
            ws =>
              ws.connectionId === workspace.connectionId &&
              normalizeRemoteWorkspacePath(ws.rootPath) ===
                normalizeRemoteWorkspacePath(workspace.remotePath)
          );

          const alreadyConnected = await sshApi.isConnected(workspace.connectionId).catch(() => false);

          if (alreadyConnected) {
            log.info('Remote workspace already connected', { connectionId: workspace.connectionId });
            await sshApi.openWorkspace(workspace.connectionId, workspace.remotePath).catch(() => {});
            setWorkspaceStatus(workspace.connectionId, 'connected');

            if (!isAlreadyOpened) {
              await workspaceManager.openRemoteWorkspace(workspace).catch(() => {});
            }
            void flowChatStore
              .initializeFromDisk(
                workspace.remotePath,
                workspace.connectionId,
                workspace.sshHost?.trim() ||
                  sshHostForRemoteWorkspace(workspace.connectionId, workspace.remotePath)
              )
              .catch(() => {});

            return { ok: true as const, connected: { workspace, connectionId: workspace.connectionId } };
          }

          if (skipPasswordAutoReconnect.has(workspace.connectionId)) {
            log.info('Skipping auto-reconnect: password auth but no stored password', {
              connectionId: workspace.connectionId,
            });
            return { ok: false as const };
          }

          log.info('Remote workspace disconnected, attempting auto-reconnect', {
            connectionId: workspace.connectionId,
            remotePath: workspace.remotePath,
          });
          const result = await tryReconnectWithRetry(workspace, 5, 5000);

          if (result !== false) {
            log.info('Reconnection successful', { newConnectionId: result.connectionId });
            setWorkspaceStatus(result.workspace.connectionId, 'connected');

            if (!isAlreadyOpened) {
              await workspaceManager.openRemoteWorkspace(result.workspace).catch(() => {});
            }
            void flowChatStore
              .initializeFromDisk(
                result.workspace.remotePath,
                result.workspace.connectionId,
                result.workspace.sshHost?.trim() ||
                  sshHostForRemoteWorkspace(
                    result.workspace.connectionId,
                    result.workspace.remotePath
                  )
              )
              .catch(() => {});

            return {
              ok: true as const,
              connected: { workspace: result.workspace, connectionId: result.connectionId },
            };
          }

          const savedConn = savedConnectionsList.find(c => c.id === workspace.connectionId);
          log.warn('Auto-reconnect failed', {
            connectionId: workspace.connectionId,
            auth: savedConn?.authType.type,
          });
          if (savedConn?.authType.type === 'Password') {
            // Keep workspace in sidebar; user reconnects manually. No auto password dialog.
            setWorkspaceStatus(workspace.connectionId, 'error');
          } else {
            await workspaceManager
              .removeRemoteWorkspace(workspace.connectionId, workspace.remotePath)
              .catch(() => {});
          }
          return { ok: false as const };
        })
      );

      const connectedEntries: ConnectedEntry[] = results
        .filter((r): r is { ok: true; connected: ConnectedEntry } => r.ok)
        .map(r => r.connected);

      const chosen = pickGlobalRemoteAfterReconnect(connectedEntries, reconnectList);
      if (chosen) {
        setIsConnected(true);
        setConnectionId(chosen.connectionId);
        setRemoteWorkspace(chosen.workspace);
        startHeartbeatRef.current(chosen.connectionId);
      }
    } catch (e) {
      log.error('checkRemoteWorkspace failed', e);
    }
  }, [setWorkspaceStatus, tryReconnectWithRetry]);
  checkRemoteWorkspaceRef.current = checkRemoteWorkspace;

  // Wait for workspace manager to finish loading, then check remote workspaces
  useEffect(() => {
    const state = workspaceManager.getState();
    if (!state.loading) {
      void checkRemoteWorkspace();
      return;
    }

    const unsubscribe = workspaceManager.addEventListener(event => {
      if (event.type === 'workspace:loading' && !event.loading) {
        unsubscribe();
        void checkRemoteWorkspace();
      }
    });

    return unsubscribe;
  }, [checkRemoteWorkspace]);

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
        let home = result.serverInfo?.homeDir?.trim();
        if (!home && result.connectionId) {
          try {
            const info = await sshApi.getServerInfo(result.connectionId);
            home = info?.homeDir?.trim();
          } catch {
            /* non-desktop or probe skipped */
          }
        }
        setRemoteFileBrowserInitialPath(
          home && home.length > 0 ? normalizeRemoteWorkspacePath(home) : '/tmp'
        );
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
  }, [startHeartbeat]);

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
    setRemoteFileBrowserInitialPath('/tmp');

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
      sshHost: connectionConfig?.host?.trim() || undefined,
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
    remoteFileBrowserInitialPath,
    connect,
    disconnect,
    openWorkspace,
    closeWorkspace,
    setShowConnectionDialog,
    setShowFileBrowser,
    clearError,
  };

  return (
    <SSHContext.Provider value={value}>
      {children}
      <SSHConnectionDialog
        open={showConnectionDialog}
        onClose={() => setShowConnectionDialog(false)}
      />
      {showFileBrowser && connectionId ? (
        <RemoteFileBrowser
          key={connectionId}
          connectionId={connectionId}
          initialPath={remoteFileBrowserInitialPath}
          onSelect={path => {
            void openWorkspace(path);
          }}
          onCancel={() => setShowFileBrowser(false)}
        />
      ) : null}
    </SSHContext.Provider>
  );
};

export default SSHRemoteProvider;
