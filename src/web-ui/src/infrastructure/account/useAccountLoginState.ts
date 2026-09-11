/** UI sign-in state follows the shared GitHub identity, independently of Relay availability. */
import { useEffect, useState } from 'react';
import { useAccountIdentity } from '@/infrastructure/account-identity';
import { remoteConnectAPI } from '@/infrastructure/api/service-api/RemoteConnectAPI';

export interface AccountLoginState {
  loggedIn: boolean;
  deviceName: string | null;
}

export function useAccountLoginState(): AccountLoginState {
  const identity = useAccountIdentity();
  const githubId = identity.me?.user.githubId;
  const loggedIn = identity.status === 'signed-in' && githubId !== undefined;
  const [device, setDevice] = useState<{ githubId: number; name: string | null } | null>(null);
  useEffect(() => {
    if (!loggedIn || githubId === undefined) return;
    let current = true;
    void remoteConnectAPI.getDeviceInfo().then(info => {
      if (current) setDevice({ githubId, name: info.device_name || null });
    }).catch(() => {
      if (current) setDevice({ githubId, name: null });
    });
    return () => { current = false; };
  }, [loggedIn, githubId]);
  return { loggedIn, deviceName: loggedIn && device?.githubId === githubId ? device.name : null };
}
