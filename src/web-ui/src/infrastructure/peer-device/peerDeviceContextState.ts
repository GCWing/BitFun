import { createContext, useContext } from 'react';

export type PeerModeState =
  | { active: false }
  | { active: true; deviceId: string; deviceName: string };

export interface PeerDeviceContextValue {
  peerMode: PeerModeState;
  enterPeerMode: (deviceId: string, deviceName: string) => Promise<void>;
  exitPeerMode: (reason?: string) => Promise<void>;
}

export const PeerDeviceContext = createContext<PeerDeviceContextValue | null>(null);

export function usePeerDeviceMode(): PeerDeviceContextValue {
  const context = useContext(PeerDeviceContext);
  if (!context) {
    throw new Error('usePeerDeviceMode must be used within PeerDeviceProvider');
  }
  return context;
}

export function usePeerDeviceModeOptional(): PeerDeviceContextValue | null {
  return useContext(PeerDeviceContext);
}
