/** Thin React binding for the window-wide device-surface controller. */

import React, { useCallback, useEffect, useMemo, useState } from 'react';
import type { PeerDeviceSurfaceSnapshot } from './PeerDeviceSurfaceController';
import { peerDeviceSurfaceController } from './peerDeviceSurfaceRuntime';
import {
  PeerDeviceContext,
  type SurfaceSwitchOutcome,
} from './peerDeviceContextState';

export const PeerDeviceProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [snapshot, setSnapshot] = useState<PeerDeviceSurfaceSnapshot>(
    () => peerDeviceSurfaceController.getSnapshot(),
  );

  useEffect(() => {
    const unsubscribe = peerDeviceSurfaceController.subscribe(setSnapshot);
    peerDeviceSurfaceController.start();
    return () => {
      unsubscribe();
      peerDeviceSurfaceController.stop();
    };
  }, []);

  const switchToDevice = useCallback(
    (deviceId: string, deviceName: string): Promise<SurfaceSwitchOutcome> =>
      peerDeviceSurfaceController.switchToDevice(deviceId, deviceName),
    [],
  );
  const switchToLocal = useCallback(
    (reason?: string): Promise<SurfaceSwitchOutcome> =>
      peerDeviceSurfaceController.switchToLocal(reason),
    [],
  );
  const disconnectDevice = useCallback(
    (deviceId: string, reason?: string): Promise<void> =>
      peerDeviceSurfaceController.disconnectDevice(deviceId, reason),
    [],
  );
  const disconnectAllDevices = useCallback(
    (reason?: string): Promise<void> =>
      peerDeviceSurfaceController.disconnectAllDevices(reason),
    [],
  );

  const value = useMemo(
    () => {
      const currentDeviceId = snapshot.peerMode.active
        ? snapshot.peerMode.deviceId
        : null;
      const currentPeerCapabilities = currentDeviceId
        ? snapshot.attachments.find(
            (attachment) => attachment.deviceId === currentDeviceId,
          )?.capabilities ?? null
        : null;
      return {
        peerMode: snapshot.peerMode,
        attachments: [...snapshot.attachments],
        currentPeerCapabilities,
        switchToDevice,
        switchToLocal,
        disconnectDevice,
        disconnectAllDevices,
      };
    },
    [
      snapshot,
      switchToDevice,
      switchToLocal,
      disconnectDevice,
      disconnectAllDevices,
    ],
  );

  return (
    <PeerDeviceContext.Provider value={value}>
      {children}
    </PeerDeviceContext.Provider>
  );
};
