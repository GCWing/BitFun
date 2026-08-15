/**
 * Device surface controller.
 *
 * Two independent concepts live here:
 *
 * - **Attachment** — a live control link to a same-account peer. It is what
 *   keeps that peer's agent running on this controller's behalf and what makes
 *   its events flow back. Attachments survive UI switches.
 * - **Rendered surface** — the single device this window is currently drawing.
 *   Switching it swaps the product transport and rebuilds the local UI; it
 *   never mutates the device being left.
 *
 * Keeping those separate is what lets several devices work at the same time:
 * dispatch a turn on B, switch the UI back to this machine, dispatch another
 * turn here, and both keep running.
 */

import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import {
  createTransportAdapter,
  getTransportAdapter,
  setTransportAdapter,
} from '@/infrastructure/api/adapters';
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
import { setActiveSurfaceDeviceId } from './deviceSurfaceRouting';
import {
  clearDeviceActivity,
  installDeviceActivityTracking,
} from './deviceActivity';
import { markDeviceSurfaceSwitched } from './deviceSurfaceReconcile';
import {
  PeerDeviceContext,
  type PeerAttachmentState,
  type PeerModeState,
} from './peerDeviceContextState';

const log = createLogger('PeerDeviceMode');

const PEER_PING_INTERVAL_MS = 20_000;
const PEER_CONTROL_RPC_TIMEOUT_MS = 15_000;

interface PeerAttachment {
  deviceId: string;
  deviceName: string;
  adapter: PeerDeviceTransportAdapter;
}

function emitPeerModeChanged(detail: { active: boolean; deviceId?: string }): void {
  setPeerDeviceModeActiveFlag(detail.active);
  window.dispatchEvent(new CustomEvent('peer-mode:changed', { detail }));
}

/**
 * Drop this window's product state so the next bootstrap loads the target
 * device's data.
 *
 * Every step here is **frontend-only on purpose**. Anything that reaches the
 * backend would land on the device being left — that is how switching used to
 * kill the previous device's terminals and LSP servers, and with them any
 * agent work depending on them.
 */
async function resetProductSurface(): Promise<void> {
  try {
    FlowChatManager.getInstance().resetForPeerModeSwitch();
  } catch (error) {
    log.warn('Failed to reset FlowChat during device surface switch', error);
  }

  // Clear before the surface flag / emit so SessionModule cannot read a stale
  // workspace while rebootstrap is still running.
  try {
    workspaceManager.clearForPeerModeSwitch();
  } catch (error) {
    log.warn('Failed to clear workspace during device surface switch', error);
  }

  // Detach from the terminal event stream but leave the PTYs running: the
  // device being left may be executing an agent turn inside one of them.
  try {
    await TerminalService.getInstance().disconnect();
  } catch (error) {
    log.warn('Failed to disconnect terminal listeners during device surface switch', error);
  }

  try {
    lspAdapterManager.disposeAll();
    WorkspaceLspManager.detachAllForSurfaceSwitch();
  } catch (error) {
    log.warn('Failed to reset LSP during device surface switch', error);
  }

  try {
    editorManager.destroy();
  } catch (error) {
    log.warn('Failed to clear editor during device surface switch', error);
  }

  try {
    clearAgentCanvasForPeerSwitch();
  } catch (error) {
    log.warn('Failed to clear canvas during device surface switch', error);
  }

  try {
    useSceneStore.getState().resetForPeerSwitch();
  } catch (error) {
    log.warn('Failed to reset scenes during device surface switch', error);
  }
}

async function rebootstrapWorkspaces(): Promise<void> {
  try {
    await workspaceManager.reinitializeForPeerModeSwitch();
  } catch (error) {
    log.warn('Device surface workspace rebootstrap failed', error);
    throw error;
  }
}

async function reloadConfigFromCurrentTransport(): Promise<void> {
  try {
    await configAPI.reloadConfig();
    configManager.clearCache();
    await configManager.reload();
  } catch (error) {
    log.warn('Failed to reload config after device surface transport switch', error);
  }
}

/**
 * Pause this device's cloud settings pull while its UI renders another device,
 * so a mid-switch reconcile cannot rewrite local settings. Tied to the
 * rendered surface, not to attachments: a background attachment leaves this
 * machine free to sync normally.
 */
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

function parseHostInvokeResult<T = unknown>(raw: string): T | undefined {
  const envelope = JSON.parse(raw) as {
    resp?: string;
    ok?: boolean;
    value?: unknown;
    error?: string;
    message?: string;
  };
  if (envelope.resp === 'error') {
    throw new Error(envelope.message || 'Peer HostInvoke failed');
  }
  if (envelope.resp === 'host_invoke_result' && !envelope.ok) {
    throw new Error(envelope.error || 'Peer HostInvoke failed');
  }
  return envelope.value as T | undefined;
}

export const PeerDeviceProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [peerMode, setPeerMode] = useState<PeerModeState>({ active: false });
  const [attachments, setAttachments] = useState<PeerAttachmentState[]>([]);
  const peerModeRef = useRef(peerMode);
  peerModeRef.current = peerMode;

  const attachmentsRef = useRef(new Map<string, PeerAttachment>());
  const localAdapterRef = useRef<ReturnType<typeof createTransportAdapter> | null>(null);
  const switchInFlightRef = useRef<Promise<void> | null>(null);

  useEffect(() => installDeviceActivityTracking(), []);

  const publishAttachments = useCallback(() => {
    setAttachments(
      Array.from(attachmentsRef.current.values()).map(({ deviceId, deviceName }) => ({
        deviceId,
        deviceName,
      })),
    );
  }, []);

  /**
   * One local adapter for the window's lifetime. Reuses the adapter the app
   * booted with so startup listener bookkeeping is not split across instances.
   */
  const localAdapter = useCallback(async () => {
    if (!localAdapterRef.current) {
      const booted = getTransportAdapter();
      const local = booted instanceof PeerDeviceTransportAdapter
        ? createTransportAdapter()
        : booted;
      await local.connect();
      localAdapterRef.current = local;
    }
    return localAdapterRef.current;
  }, []);

  /**
   * Establish (or reuse) the control link to a peer. Attaching does not render
   * the device — that is `switchToDevice`.
   */
  const ensureAttachment = useCallback(async (
    deviceId: string,
    deviceName: string,
  ): Promise<PeerAttachment> => {
    const existing = attachmentsRef.current.get(deviceId);
    if (existing) {
      if (existing.deviceName !== deviceName && deviceName) {
        existing.deviceName = deviceName;
        publishAttachments();
      }
      return existing;
    }

    const peerCapabilities = parseHostInvokeResult<{
      capabilities?: {
        idempotent_dialog_submit?: boolean;
        targeted_session_rollback?: boolean;
      };
    }>(
      await remoteConnectAPI.accountDeviceRpc(
        deviceId,
        JSON.stringify({ cmd: 'host_invoke', command: 'peer_mode_ping', args: {} }),
        PEER_CONTROL_RPC_TIMEOUT_MS,
      ),
    );

    const localInfo = await remoteConnectAPI.getDeviceInfo();
    const controllerDeviceId = localInfo.device_id;

    const adapter = new PeerDeviceTransportAdapter(
      deviceId,
      (target, commandJson, timeoutMs) =>
        remoteConnectAPI.accountDeviceRpc(target, commandJson, timeoutMs),
      {
        supportsIdempotentDialogSubmit:
          peerCapabilities?.capabilities?.idempotent_dialog_submit === true,
        supportsTargetedSessionRollback:
          peerCapabilities?.capabilities?.targeted_session_rollback === true,
      },
    );
    await adapter.connect();

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

    const attachment: PeerAttachment = { deviceId, deviceName, adapter };
    attachmentsRef.current.set(deviceId, attachment);
    publishAttachments();
    log.info('Attached peer device', { deviceId, deviceName });
    return attachment;
  }, [publishAttachments]);

  /**
   * Point the UI at one device. Nothing is sent to the device being left.
   */
  const switchSurface = useCallback(async (
    target: PeerAttachment | null,
    reason?: string,
  ): Promise<void> => {
    const current = peerModeRef.current;
    const currentId = current.active ? current.deviceId : null;
    const nextId = target?.deviceId ?? null;
    if (currentId === nextId) {
      return;
    }

    // Pause cloud pull *before* clearing the UI so a mid-switch reconcile
    // cannot rewrite this device's settings while the rebuild is in flight.
    if (nextId !== null) {
      await setPeerControllerActive(true, true);
    }

    await resetProductSurface();

    setActiveSurfaceDeviceId(nextId);
    setTransportAdapter(target ? target.adapter : await localAdapter());
    api.reattachTransportAdapter();
    markDeviceSurfaceSwitched();

    const nextState: PeerModeState = target
      ? { active: true, deviceId: target.deviceId, deviceName: target.deviceName }
      : { active: false };
    setPeerMode(nextState);
    peerModeRef.current = nextState;
    emitPeerModeChanged({ active: nextId !== null, deviceId: nextId ?? currentId ?? undefined });

    await reloadConfigFromCurrentTransport();
    await rebootstrapWorkspaces();

    // Resume cloud pull only once this device's own surface is rebuilt.
    // Best-effort: a transport hiccup here must not strand the UI.
    if (nextId === null) {
      await setPeerControllerActive(false, false);
    }
    log.info('Device surface switched', { from: currentId, to: nextId, reason });
  }, [localAdapter]);

  /** Serialize switches so a fast double click cannot interleave two rebuilds. */
  const runExclusive = useCallback(async (task: () => Promise<void>): Promise<void> => {
    const previous = switchInFlightRef.current ?? Promise.resolve();
    const next = previous.catch(() => undefined).then(task);
    switchInFlightRef.current = next.catch(() => undefined);
    return next;
  }, []);

  const switchToLocal = useCallback(async (reason?: string): Promise<void> => {
    await runExclusive(async () => {
      await switchSurface(null, reason ?? 'manual');
    });
  }, [runExclusive, switchSurface]);

  const switchToDevice = useCallback(async (
    deviceId: string,
    deviceName: string,
  ): Promise<void> => {
    if (!deviceId) {
      throw new Error('deviceId is required');
    }
    await runExclusive(async () => {
      const previous = peerModeRef.current;
      const attachment = await ensureAttachment(deviceId, deviceName);
      try {
        await switchSurface(attachment, 'manual');
      } catch (error) {
        log.error('Device surface switch failed; returning to the local surface', error);
        // The attachment stays: the peer may already be running work for us.
        // Only the rendered surface rolls back.
        if (previous.active && previous.deviceId !== deviceId) {
          log.warn('Rolling back to the local surface instead of the previous peer', {
            previousDeviceId: previous.deviceId,
          });
        }
        try {
          await switchSurface(null, 'switch_failed');
        } catch (rollbackError) {
          log.error('Failed to roll back to the local surface', rollbackError);
        }
        throw error;
      }
    });
  }, [ensureAttachment, runExclusive, switchSurface]);

  const dropAttachment = useCallback(async (
    deviceId: string,
    options: { notifyPeer: boolean },
  ): Promise<void> => {
    const attachment = attachmentsRef.current.get(deviceId);
    if (!attachment) {
      return;
    }
    attachmentsRef.current.delete(deviceId);
    publishAttachments();
    clearDeviceActivity(deviceId);

    if (options.notifyPeer) {
      try {
        const localInfo = await remoteConnectAPI.getDeviceInfo();
        await detachPeerControl(deviceId, localInfo.device_id);
      } catch (error) {
        // The peer keeps the stale controller until presence prunes it; the
        // controller side is already detached either way.
        log.warn('Failed to detach peer control subscription', { deviceId, error });
        throw error;
      }
    }
  }, [publishAttachments]);

  const disconnectDevice = useCallback(async (
    deviceId: string,
    reason?: string,
  ): Promise<void> => {
    const isRendered = peerModeRef.current.active
      && peerModeRef.current.deviceId === deviceId;
    if (isRendered) {
      await switchToLocal(reason ?? 'disconnect');
    }
    try {
      await dropAttachment(deviceId, { notifyPeer: reason !== 'peer_offline' });
    } catch (error) {
      // An offline peer cannot confirm the detach and there is nothing the
      // user can do about it; a manual disconnect that failed may still be
      // running our work, so it has to be surfaced.
      if (shouldSurfacePeerDetachFailure(reason)) {
        throw error;
      }
      log.warn('Ignoring detach failure for an unreachable peer', { deviceId, reason });
    }
    log.info('Disconnected peer device', { deviceId, reason: reason ?? 'manual' });
  }, [dropAttachment, switchToLocal]);

  const disconnectAllDevices = useCallback(async (reason?: string): Promise<void> => {
    const deviceIds = Array.from(attachmentsRef.current.keys());
    for (const deviceId of deviceIds) {
      try {
        await disconnectDevice(deviceId, reason);
      } catch (error) {
        log.warn('Failed to disconnect peer device', { deviceId, error });
      }
    }
  }, [disconnectDevice]);

  // Prune attachments whose device dropped out of account presence. A peer
  // that goes offline can no longer run our work, so the link is dead even if
  // it is not the rendered surface.
  useEffect(() => {
    if (attachments.length === 0) {
      return;
    }
    return api.listen<{ devices: Array<{ device_id: string; device_name: string }> }>(
      'account://device-presence',
      (payload) => {
        const online = new Set((payload?.devices ?? []).map((d) => d.device_id));
        for (const deviceId of Array.from(attachmentsRef.current.keys())) {
          if (online.has(deviceId)) {
            continue;
          }
          const deviceName = attachmentsRef.current.get(deviceId)?.deviceName ?? deviceId;
          const wasRendered = peerModeRef.current.active
            && peerModeRef.current.deviceId === deviceId;
          void disconnectDevice(deviceId, 'peer_offline').finally(() => {
            if (wasRendered) {
              window.dispatchEvent(
                new CustomEvent('peer-mode:auto-exit', {
                  detail: { deviceId, deviceName, reason: 'peer_offline' },
                }),
              );
            }
          });
        }
      },
    );
  }, [attachments.length, disconnectDevice]);

  // Signing out revokes the control links; nothing may keep running on this
  // account's peers on our behalf afterwards.
  useEffect(() => {
    return api.listen<{ logged_in: boolean }>('account://login-state', (payload) => {
      if (payload?.logged_in === false && attachmentsRef.current.size > 0) {
        void disconnectAllDevices('account_logout');
      }
    });
  }, [disconnectAllDevices]);

  // Keepalive for every attachment, rendered or not — a background peer that
  // stopped answering is not running our work any more.
  useEffect(() => {
    if (attachments.length === 0) {
      return;
    }
    const timer = setInterval(() => {
      for (const { deviceId } of Array.from(attachmentsRef.current.values())) {
        void (async () => {
          try {
            parseHostInvokeResult(
              await remoteConnectAPI.accountDeviceRpc(
                deviceId,
                JSON.stringify({ cmd: 'host_invoke', command: 'peer_mode_ping', args: {} }),
                PEER_CONTROL_RPC_TIMEOUT_MS,
              ),
            );
          } catch (error) {
            // Presence is the authority for teardown; a single missed ping on a
            // weak link must not drop a peer that is mid-turn.
            log.warn('Peer keepalive ping failed', { deviceId, error });
          }
        })();
      }
    }, PEER_PING_INTERVAL_MS);
    return () => clearInterval(timer);
  }, [attachments.length]);

  const value = useMemo(
    () => ({
      peerMode,
      attachments,
      switchToDevice,
      switchToLocal,
      disconnectDevice,
      disconnectAllDevices,
    }),
    [peerMode, attachments, switchToDevice, switchToLocal, disconnectDevice, disconnectAllDevices],
  );

  return (
    <PeerDeviceContext.Provider value={value}>
      {children}
    </PeerDeviceContext.Provider>
  );
};
