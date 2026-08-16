/**
 * Account device roster for the sidebar device switcher.
 *
 * A deliberately small read-only view of the account's devices: the account
 * dialog owns login, sync and device removal, this only needs "who can I hand
 * work to right now". Device routing is re-established by the host at startup,
 * so presence flows without opening the dialog.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import { remoteConnectAPI } from '@/infrastructure/api/service-api/RemoteConnectAPI';
import { createLogger } from '@/shared/utils/logger';
import { useAccountLoginState } from '@/infrastructure/account/useAccountLoginState';

const log = createLogger('AccountDeviceRoster');

const ROSTER_POLL_MS = 60_000;

export interface DeviceRosterEntry {
  deviceId: string;
  deviceName: string;
  online: boolean;
  isLocal: boolean;
}

export interface AccountDeviceRoster {
  loggedIn: boolean;
  localDeviceId: string | null;
  localDeviceName: string | null;
  /** Local device first, then online peers, then offline peers. */
  devices: DeviceRosterEntry[];
  refresh: () => void;
}

export function useAccountDeviceRoster(): AccountDeviceRoster {
  const { loggedIn, deviceName: localDeviceName } = useAccountLoginState();
  const [localDeviceId, setLocalDeviceId] = useState<string | null>(null);
  const [peers, setPeers] = useState<DeviceRosterEntry[]>([]);
  const generationRef = useRef(0);

  const refresh = useCallback(() => {
    if (!loggedIn) {
      return;
    }
    const generation = ++generationRef.current;
    void (async () => {
      try {
        const [info, list] = await Promise.all([
          remoteConnectAPI.getDeviceInfo(),
          remoteConnectAPI.accountListDevices(),
        ]);
        if (generationRef.current !== generation) {
          return;
        }
        setLocalDeviceId(info.device_id);
        setPeers(
          list
            .filter(device => device.device_id !== info.device_id)
            .map(device => ({
              deviceId: device.device_id,
              deviceName: device.device_name,
              online: device.online,
              isLocal: false,
            })),
        );
      } catch (error) {
        // A transport hiccup is not evidence that devices went away; keep the
        // last roster until presence or the next poll corrects it.
        log.warn('Failed to refresh account device roster', error);
      }
    })();
  }, [loggedIn]);

  useEffect(() => {
    if (!loggedIn) {
      generationRef.current += 1;
      setLocalDeviceId(null);
      setPeers([]);
      return;
    }
    refresh();
    const poll = setInterval(refresh, ROSTER_POLL_MS);
    return () => clearInterval(poll);
  }, [loggedIn, refresh]);

  useEffect(() => {
    if (!loggedIn) {
      return;
    }
    return api.listen<{ devices: Array<{ device_id: string; device_name: string }> }>(
      'account://device-presence',
      (payload) => {
        const online = payload?.devices ?? [];
        const onlineIds = new Set(online.map(device => device.device_id));
        setPeers(prev => {
          const byId = new Map(prev.map(device => [device.deviceId, device]));
          for (const device of online) {
            if (device.device_id === localDeviceId) {
              continue;
            }
            const existing = byId.get(device.device_id);
            byId.set(device.device_id, {
              deviceId: device.device_id,
              deviceName: device.device_name || existing?.deviceName || device.device_id,
              online: true,
              isLocal: false,
            });
          }
          for (const [deviceId, device] of byId) {
            if (!onlineIds.has(deviceId) && device.online) {
              byId.set(deviceId, { ...device, online: false });
            }
          }
          return Array.from(byId.values());
        });
      },
    );
  }, [loggedIn, localDeviceId]);

  const devices = useMemo<DeviceRosterEntry[]>(() => {
    const sortedPeers = [...peers].sort((a, b) => {
      if (a.online !== b.online) {
        return a.online ? -1 : 1;
      }
      return a.deviceName.localeCompare(b.deviceName);
    });
    if (!localDeviceId) {
      return sortedPeers;
    }
    return [
      {
        deviceId: localDeviceId,
        deviceName: localDeviceName || localDeviceId,
        online: true,
        isLocal: true,
      },
      ...sortedPeers,
    ];
  }, [peers, localDeviceId, localDeviceName]);

  return { loggedIn, localDeviceId, localDeviceName, devices, refresh };
}
