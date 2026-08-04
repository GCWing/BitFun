import { useEffect, useSyncExternalStore } from 'react';
import { marketAccountService } from './MarketAccountService';

export function useMarketAccount() {
  const snapshot = useSyncExternalStore(
    listener => marketAccountService.subscribe(listener),
    () => marketAccountService.getSnapshot(),
    () => marketAccountService.getSnapshot(),
  );

  useEffect(() => {
    void marketAccountService.initialize();
  }, []);

  return snapshot;
}
