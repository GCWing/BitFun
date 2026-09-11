import { useEffect, useSyncExternalStore } from 'react';
import { accountIdentityService } from './AccountIdentityService';

export function useAccountIdentity() {
  const snapshot = useSyncExternalStore(
    listener => accountIdentityService.subscribe(listener),
    () => accountIdentityService.getSnapshot(),
    () => accountIdentityService.getSnapshot(),
  );

  useEffect(() => {
    void accountIdentityService.initialize();
  }, []);

  return snapshot;
}
