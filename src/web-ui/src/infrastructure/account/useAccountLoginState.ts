/**
 * Shared account login state for UI chrome (menu items, tab badges).
 *
 * Initial state comes from `account_status`; afterwards the backend pushes
 * `account://login-state` on login / logout / finalize. Token expiry clears
 * the session without an event, so a slow poll keeps the state honest.
 * Components that need the full device list or relay details should still
 * query the API directly.
 */

import { useEffect, useState } from 'react';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import { remoteConnectAPI } from '@/infrastructure/api/service-api/RemoteConnectAPI';

const STATUS_POLL_MS = 60_000;

export interface AccountLoginState {
  loggedIn: boolean;
  /** Friendly name of this device, shown as the logged-in label. */
  deviceName: string | null;
}

export function useAccountLoginState(): AccountLoginState {
  const [state, setState] = useState<AccountLoginState>({ loggedIn: false, deviceName: null });

  useEffect(() => {
    let cancelled = false;
    const refresh = async () => {
      const status = await remoteConnectAPI.accountStatus();
      if (!status.logged_in) {
        if (!cancelled) setState({ loggedIn: false, deviceName: null });
        return;
      }
      // accountStatus only exposes a UUID user_id; the menu label needs the
      // human-readable device name instead.
      let deviceName: string | null = null;
      try {
        const info = await remoteConnectAPI.getDeviceInfo();
        deviceName = info.device_name || null;
      } catch {
        deviceName = null;
      }
      if (!cancelled) {
        setState({ loggedIn: true, deviceName });
      }
    };
    void refresh();

    const unlisten = api.listen<{ logged_in: boolean }>(
      'account://login-state',
      () => {
        // The event payload does not carry the device name; re-read the status
        // so the label always reflects the persisted account session.
        void refresh();
      },
    );
    const poll = setInterval(() => { void refresh(); }, STATUS_POLL_MS);

    return () => {
      cancelled = true;
      unlisten();
      clearInterval(poll);
    };
  }, []);

  return state;
}
