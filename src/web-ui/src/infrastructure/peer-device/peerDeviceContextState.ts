import { createContext, useContext } from 'react';
import type {
  PeerConnectionHealth,
  PeerConnectionLostReason,
} from './PeerConnectionManager';

/**
 * The device surface currently rendered by this window.
 *
 * Exactly one device is rendered at a time; every attached device keeps
 * executing regardless of which one that is.
 */
export type PeerModeState =
  | { active: false }
  | { active: true; deviceId: string; deviceName: string };

/** A peer whose control link is kept alive, rendered or not. */
export interface PeerAttachmentState {
  deviceId: string;
  deviceName: string;
  health: PeerConnectionHealth;
  lostReason: PeerConnectionLostReason | null;
}

/** A newer request may intentionally supersede an earlier rapid switch. */
export type SurfaceSwitchOutcome = 'activated' | 'superseded';

export interface PeerDeviceContextValue {
  /** Rendered surface. `active: false` means this machine. */
  peerMode: PeerModeState;
  /**
   * Peers this controller holds a live control link to, including the rendered
   * one. Attachments survive UI switches so their agents keep running and keep
   * reporting progress.
   */
  attachments: PeerAttachmentState[];
  /** Render another device, attaching it first when needed. */
  switchToDevice: (deviceId: string, deviceName: string) => Promise<SurfaceSwitchOutcome>;
  /** Render this machine again. Peer attachments are left running. */
  switchToLocal: (reason?: string) => Promise<SurfaceSwitchOutcome>;
  /** Drop the control link to a peer, switching to local when it is rendered. */
  disconnectDevice: (deviceId: string, reason?: string) => Promise<void>;
  /** Drop every control link. Used on logout / account change. */
  disconnectAllDevices: (reason?: string) => Promise<void>;
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
