import React, {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
} from 'react';
import { setTransportAdapter, createTransportAdapter } from '@/infrastructure/api/adapters';
import { PeerDeviceTransportAdapter } from '@/infrastructure/api/adapters/peer-device-adapter';
import { remoteConnectAPI } from '@/infrastructure/api/service-api/RemoteConnectAPI';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import { FlowChatManager } from '@/flow_chat/services/FlowChatManager';
import { TerminalService } from '@/tools/terminal/services/TerminalService';
import { workspaceManager } from '@/infrastructure/services/business/workspaceManager';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('PeerDeviceMode');

export type PeerModeState =
  | { active: false }
  | { active: true; deviceId: string; deviceName: string };

interface PeerDeviceContextValue {
  peerMode: PeerModeState;
  enterPeerMode: (deviceId: string, deviceName: string) => Promise<void>;
  exitPeerMode: () => Promise<void>;
}

const PeerDeviceContext = createContext<PeerDeviceContextValue | null>(null);

async function resetProductSurface(): Promise<void> {
  try {
    FlowChatManager.getInstance().cleanupEventListeners();
  } catch (error) {
    log.warn('Failed to cleanup FlowChat listeners during peer mode switch', error);
  }

  try {
    await TerminalService.getInstance().shutdownAll();
  } catch (error) {
    log.warn('Failed to shutdown terminals during peer mode switch', error);
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

  const enterPeerMode = useCallback(async (deviceId: string, deviceName: string) => {
    if (!deviceId) {
      throw new Error('deviceId is required');
    }

    parseHostInvokeResult(
      await remoteConnectAPI.accountDeviceRpc(
        deviceId,
        JSON.stringify({ cmd: 'host_invoke', command: 'peer_mode_ping', args: {} }),
      ),
    );

    const localInfo = await remoteConnectAPI.getDeviceInfo();
    const controllerDeviceId = localInfo.device_id;

    await resetProductSurface();

    const peerTransport = new PeerDeviceTransportAdapter(
      deviceId,
      (target, commandJson) => remoteConnectAPI.accountDeviceRpc(target, commandJson),
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
      ),
    );

    setPeerMode({ active: true, deviceId, deviceName });
    await rebootstrapWorkspaces();
    log.info('Entered peer device mode', { deviceId, deviceName });
  }, []);

  const exitPeerMode = useCallback(async () => {
    if (!peerMode.active) {
      return;
    }

    const { deviceId } = peerMode;
    try {
      const localInfo = await remoteConnectAPI.getDeviceInfo();
      parseHostInvokeResult(
        await remoteConnectAPI.accountDeviceRpc(
          deviceId,
          JSON.stringify({
            cmd: 'host_invoke',
            command: 'peer_control_detach',
            args: { controller_device_id: localInfo.device_id },
          }),
        ),
      );
    } catch (error) {
      log.warn('Failed to detach peer control subscription', error);
    }

    await resetProductSurface();

    const local = createTransportAdapter();
    await local.connect();
    setTransportAdapter(local);
    api.reattachTransportAdapter();

    setPeerMode({ active: false });
    await rebootstrapWorkspaces();
    log.info('Exited peer device mode', { deviceId });
  }, [peerMode]);

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

export function usePeerDeviceMode(): PeerDeviceContextValue {
  const ctx = useContext(PeerDeviceContext);
  if (!ctx) {
    throw new Error('usePeerDeviceMode must be used within PeerDeviceProvider');
  }
  return ctx;
}

export function usePeerDeviceModeOptional(): PeerDeviceContextValue | null {
  return useContext(PeerDeviceContext);
}
