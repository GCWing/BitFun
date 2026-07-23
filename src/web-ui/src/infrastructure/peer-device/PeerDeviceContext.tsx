import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { setTransportAdapter, createTransportAdapter } from '@/infrastructure/api/adapters';
import { PeerDeviceTransportAdapter } from '@/infrastructure/api/adapters/peer-device-adapter';
import { remoteConnectAPI } from '@/infrastructure/api/service-api/RemoteConnectAPI';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { FlowChatManager } from '@/flow_chat/services/FlowChatManager';
import { TerminalService } from '@/tools/terminal/services/TerminalService';
import { workspaceManager } from '@/infrastructure/services/business/workspaceManager';
import { editorManager } from '@/tools/editor/services/EditorManager';
import { useSceneStore } from '@/app/stores/sceneStore';
import { clearAgentCanvasForPeerSwitch } from '@/app/components/panels/content-canvas/stores';
import { WorkspaceLspManager } from '@/tools/lsp/services/WorkspaceLspManager';
import { lspAdapterManager } from '@/tools/lsp/services/LspAdapterManager';
import { createLogger } from '@/shared/utils/logger';
import { setPeerDeviceModeActiveFlag } from './peerModeFlag';
import { shouldSurfacePeerDetachFailure } from './peerDetachPolicy';
import {
  PeerDeviceContext,
  type PeerModeState,
} from './peerDeviceContextState';

const log = createLogger('PeerDeviceMode');

/** Only high/normal HostInvoke transport failures count toward auto-exit. */
const PEER_RPC_FAILURE_LIMIT = 2147483647;
const PEER_PING_INTERVAL_MS = 20_000;
const PEER_CONTROL_RPC_TIMEOUT_MS = 15_000;

function emitPeerModeChanged(detail: { active: boolean; deviceId?: string }): void {
  setPeerDeviceModeActiveFlag(detail.active);
  window.dispatchEvent(new CustomEvent('peer-mode:changed', { detail }));
}

async function resetProductSurface(): Promise<void> {
  try {
    FlowChatManager.getInstance().resetForPeerModeSwitch();
  } catch (error) {
    log.warn('Failed to reset FlowChat during peer mode switch', error);
  }

  // Clear before peer flag / emit so SessionModule cannot read a stale
  // controller workspace while rebootstrap is still running.
  try {
    workspaceManager.clearForPeerModeSwitch();
  } catch (error) {
    log.warn('Failed to clear workspace during peer mode switch', error);
  }

  try {
    await TerminalService.getInstance().shutdownAll();
  } catch (error) {
    log.warn('Failed to shutdown terminals during peer mode switch', error);
  }

  try {
    await TerminalService.getInstance().disconnect();
  } catch (error) {
    log.warn('Failed to disconnect terminal listeners during peer mode switch', error);
  }

  try {
    lspAdapterManager.disposeAll();
    await WorkspaceLspManager.clearAllForPeerSwitch();
  } catch (error) {
    log.warn('Failed to reset LSP during peer mode switch', error);
  }

  try {
    editorManager.destroy();
  } catch (error) {
    log.warn('Failed to clear editor during peer mode switch', error);
  }

  try {
    clearAgentCanvasForPeerSwitch();
  } catch (error) {
    log.warn('Failed to clear canvas during peer mode switch', error);
  }

  try {
    useSceneStore.getState().resetForPeerSwitch();
  } catch (error) {
    log.warn('Failed to reset scenes during peer mode switch', error);
  }
}

async function rebootstrapWorkspaces(): Promise<void> {
  try {
    await workspaceManager.reinitializeForPeerModeSwitch();
  } catch (error) {
    log.warn('Peer mode workspace rebootstrap failed', error);
    throw error;
  }
}

async function reloadConfigFromCurrentTransport(): Promise<void> {
  try {
    await configAPI.reloadConfig();
    configManager.clearCache();
    await configManager.reload();
  } catch (error) {
    log.warn('Failed to reload config after peer mode transport switch', error);
  }
}

async function setPeerControllerActive(active: boolean, required: boolean): Promise<void> {
  try {
    await api.invoke('peer_controller_set_active', { active });
  } catch (error) {
    log.warn('Failed to update peer controller active flag', { active, error });
    if (required) {
      throw error instanceof Error
        ? error
        : new Error(`peer_controller_set_active(${active}) failed`);
    }
  }
}

async function detachPeerControl(deviceId: string, controllerDeviceId: string): Promise<void> {
  parseHostInvokeResult(
    await remoteConnectAPI.accountDeviceRpc(
      deviceId,
      JSON.stringify({
        cmd: 'host_invoke',
        command: 'peer_control_detach',
        args: { controller_device_id: controllerDeviceId },
      }),
      PEER_CONTROL_RPC_TIMEOUT_MS,
    ),
  );
}

function parseHostInvokeResult(raw: string): void {
  const envelope = JSON.parse(raw) as {
    resp?: string;
    ok?: boolean;
    error?: string;
    message?: string;
  };
  if (envelope.resp === 'error') {
    throw new Error(envelope.message || 'Peer HostInvoke failed');
  }
  if (envelope.resp === 'host_invoke_result' && !envelope.ok) {
    throw new Error(envelope.error || 'Peer HostInvoke failed');
  }
}

export const PeerDeviceProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [peerMode, setPeerMode] = useState<PeerModeState>({ active: false });
  const peerModeRef = useRef(peerMode);
  peerModeRef.current = peerMode;
  const exitInFlightRef = useRef(false);
  const rpcFailuresRef = useRef(0);

  const restoreLocalTransport = useCallback(async () => {
    const local = createTransportAdapter();
    await local.connect();
    setTransportAdapter(local);
    api.reattachTransportAdapter();
  }, []);

  const exitPeerMode = useCallback(async (reason?: string) => {
    if (!peerModeRef.current.active || exitInFlightRef.current) {
      return;
    }
    exitInFlightRef.current = true;
    const { deviceId, deviceName } = peerModeRef.current;
    let detachError: unknown;
    try {
      try {
        const localInfo = await remoteConnectAPI.getDeviceInfo();
        await detachPeerControl(deviceId, localInfo.device_id);
      } catch (error) {
        detachError = error;
        log.warn('Failed to detach peer control subscription', error);
      }

      await resetProductSurface();
      await restoreLocalTransport();
      await setPeerControllerActive(false, false);
      setPeerMode({ active: false });
      emitPeerModeChanged({ active: false, deviceId });
      rpcFailuresRef.current = 0;
      await reloadConfigFromCurrentTransport();
      await rebootstrapWorkspaces();
      log.info('Exited peer device mode', { deviceId, deviceName, reason: reason ?? 'manual' });
      if (reason && reason !== 'manual') {
        window.dispatchEvent(
          new CustomEvent('peer-mode:auto-exit', {
            detail: { deviceId, deviceName, reason },
          }),
        );
      }
      if (detachError && shouldSurfacePeerDetachFailure(reason)) {
        throw detachError instanceof Error
          ? detachError
          : new Error('Peer work may still be running after disconnect');
      }
    } finally {
      exitInFlightRef.current = false;
    }
  }, [restoreLocalTransport]);

  const notePeerTransportFailure = useCallback((
    error: unknown,
    meta?: { action: string; priority: 'high' | 'normal' | 'low' },
  ) => {
    if (!peerModeRef.current.active) {
      return;
    }
    // Background git/SSH/editor noise must not force-exit Peer Mode.
    if (meta?.priority === 'low') {
      log.warn('Peer transport failure ignored for auto-exit (low priority)', {
        action: meta.action,
        error,
      });
      return;
    }
    rpcFailuresRef.current += 1;
    log.warn('Peer transport failure counted', {
      failures: rpcFailuresRef.current,
      action: meta?.action,
      priority: meta?.priority,
      error,
    });
    if (rpcFailuresRef.current >= PEER_RPC_FAILURE_LIMIT) {
      void exitPeerMode('rpc_failures');
    }
  }, [exitPeerMode]);

  const notePeerRpcSuccess = useCallback(() => {
    rpcFailuresRef.current = 0;
  }, []);

  const enterPeerMode = useCallback(async (deviceId: string, deviceName: string) => {
    if (!deviceId) {
      throw new Error('deviceId is required');
    }
    if (peerModeRef.current.active) {
      await exitPeerMode('switch');
    }

    parseHostInvokeResult(
      await remoteConnectAPI.accountDeviceRpc(
        deviceId,
        JSON.stringify({ cmd: 'host_invoke', command: 'peer_mode_ping', args: {} }),
        PEER_CONTROL_RPC_TIMEOUT_MS,
      ),
    );

    const localInfo = await remoteConnectAPI.getDeviceInfo();
    const controllerDeviceId = localInfo.device_id;

    // Pause cloud pull before clearing local UI so mid-switch reconcile cannot rewrite A.
    await setPeerControllerActive(true, true);

    let attached = false;
    try {
      await resetProductSurface();

      const peerTransport = new PeerDeviceTransportAdapter(
        deviceId,
        (target, commandJson, timeoutMs) =>
          remoteConnectAPI.accountDeviceRpc(target, commandJson, timeoutMs),
        {
          onHostInvokeSuccess: notePeerRpcSuccess,
          onHostInvokeTransportFailure: notePeerTransportFailure,
        },
      );

      await peerTransport.connect();
      setTransportAdapter(peerTransport);
      api.reattachTransportAdapter();

      parseHostInvokeResult(
        await remoteConnectAPI.accountDeviceRpc(
          deviceId,
          JSON.stringify({
            cmd: 'host_invoke',
            command: 'peer_control_attach',
            args: { controller_device_id: controllerDeviceId },
          }),
          PEER_CONTROL_RPC_TIMEOUT_MS,
        ),
      );
      attached = true;

      setPeerMode({ active: true, deviceId, deviceName });
      emitPeerModeChanged({ active: true, deviceId });
      rpcFailuresRef.current = 0;
      await reloadConfigFromCurrentTransport();
      await rebootstrapWorkspaces();
      log.info('Entered peer device mode', { deviceId, deviceName });
    } catch (error) {
      log.error('enterPeerMode failed; restoring local transport', error);
      if (attached) {
        try {
          await detachPeerControl(deviceId, controllerDeviceId);
        } catch (detachError) {
          log.warn('Failed to detach after enterPeerMode failure', detachError);
        }
      }
      try {
        await restoreLocalTransport();
        await setPeerControllerActive(false, false);
        setPeerMode({ active: false });
        emitPeerModeChanged({ active: false, deviceId });
        await reloadConfigFromCurrentTransport();
        await rebootstrapWorkspaces();
      } catch (rollbackError) {
        log.error('Failed to roll back after enterPeerMode failure', rollbackError);
      }
      throw error;
    }
  }, [exitPeerMode, notePeerTransportFailure, notePeerRpcSuccess, restoreLocalTransport]);

  // Auto-exit when the peer drops from account presence or ping fails.
  useEffect(() => {
    if (!peerMode.active) {
      return;
    }
    const { deviceId } = peerMode;

    const unlistenPresence = api.listen<{ devices: Array<{ device_id: string; device_name: string }> }>(
      'account://device-presence',
      (payload) => {
        const online = payload?.devices ?? [];
        if (!online.some((d) => d.device_id === deviceId)) {
          void exitPeerMode('peer_offline');
        }
      },
    );

    const pingTimer = setInterval(() => {
      void (async () => {
        try {
          parseHostInvokeResult(
            await remoteConnectAPI.accountDeviceRpc(
              deviceId,
              JSON.stringify({ cmd: 'host_invoke', command: 'peer_mode_ping', args: {} }),
              PEER_CONTROL_RPC_TIMEOUT_MS,
            ),
          );
          notePeerRpcSuccess();
        } catch (error) {
          notePeerTransportFailure(error);
        }
      })();
    }, PEER_PING_INTERVAL_MS);

    return () => {
      unlistenPresence();
      clearInterval(pingTimer);
    };
  }, [peerMode, exitPeerMode, notePeerTransportFailure, notePeerRpcSuccess]);

  const value = useMemo(
    () => ({ peerMode, enterPeerMode, exitPeerMode }),
    [peerMode, enterPeerMode, exitPeerMode],
  );

  return (
    <PeerDeviceContext.Provider value={value}>
      {children}
    </PeerDeviceContext.Provider>
  );
};
