import { useEffect, useRef } from 'react';
import { RemoteSessionManager } from '../services/RemoteSessionManager';
import { useMobileStore } from '../services/store';

const PING_INTERVAL = 15000;
const PING_TIMEOUT = 10000;

function pingWithTimeout(mgr: RemoteSessionManager, ms: number): Promise<void> {
  return Promise.race([
    mgr.ping(),
    new Promise<void>((_, reject) =>
      setTimeout(() => reject(new Error('ping timeout')), ms),
    ),
  ]);
}

export function useConnectionHealth(sessionMgr: RemoteSessionManager | null) {
  const setConnectionHealth = useMobileStore((s) => s.setConnectionHealth);
  const timerRef = useRef<ReturnType<typeof setTimeout>>();

  useEffect(() => {
    let cancelled = false;

    if (!sessionMgr) {
      setConnectionHealth('unpaired');
      return;
    }

    const loop = async () => {
      try {
        await pingWithTimeout(sessionMgr, PING_TIMEOUT);
        if (!cancelled) setConnectionHealth('connected');
      } catch {
        if (!cancelled) setConnectionHealth('unreachable');
      }

      if (!cancelled) {
        timerRef.current = setTimeout(loop, PING_INTERVAL);
      }
    };

    loop();

    return () => {
      cancelled = true;
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, [sessionMgr, setConnectionHealth]);
}
