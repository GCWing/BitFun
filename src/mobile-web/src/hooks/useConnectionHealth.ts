import { useEffect, useRef } from 'react';
import { isDelegatedIdentityChangedError } from '../services/RelayHttpClient';
import {
  isRemoteControlTargetChangedError,
  RemoteSessionManager,
} from '../services/RemoteSessionManager';
import { useMobileStore } from '../services/store';

const PING_INTERVAL = 15000;
const PING_TIMEOUT = 10000;
const OWNERSHIP_RETRY_DELAY = 250;

function pingWithTimeout(mgr: RemoteSessionManager, ms: number): Promise<void> {
  let timeoutId: ReturnType<typeof setTimeout> | undefined;
  return Promise.race([
    mgr.ping(),
    new Promise<void>((_, reject) => {
      timeoutId = setTimeout(() => reject(new Error('ping timeout')), ms);
    }),
  ]).finally(() => {
    if (timeoutId) clearTimeout(timeoutId);
  });
}

export function useConnectionHealth(sessionMgr: RemoteSessionManager | null) {
  const setConnectionHealth = useMobileStore((s) => s.setConnectionHealth);
  const timerRef = useRef<ReturnType<typeof setTimeout>>();

  useEffect(() => {
    let cancelled = false;
    let loopGeneration = 0;

    if (!sessionMgr) {
      setConnectionHealth('unpaired');
      return;
    }

    const schedule = (generation: number, delay: number) => {
      if (cancelled || generation !== loopGeneration) return;
      if (timerRef.current) clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => {
        void loop(generation);
      }, delay);
    };

    const loop = async (generation: number) => {
      if (cancelled || generation !== loopGeneration) return;
      try {
        await pingWithTimeout(sessionMgr, PING_TIMEOUT);
        if (cancelled || generation !== loopGeneration) return;
        setConnectionHealth('connected');
        schedule(generation, PING_INTERVAL);
      } catch (error: unknown) {
        if (cancelled || generation !== loopGeneration) return;
        if (
          isRemoteControlTargetChangedError(error)
          || isDelegatedIdentityChangedError(error)
        ) {
          setConnectionHealth('checking');
          schedule(generation, OWNERSHIP_RETRY_DELAY);
          return;
        }
        setConnectionHealth('unreachable');
        schedule(generation, PING_INTERVAL);
      }
    };

    const restart = () => {
      loopGeneration += 1;
      if (timerRef.current) clearTimeout(timerRef.current);
      setConnectionHealth('checking');
      void loop(loopGeneration);
    };

    const unlisten = sessionMgr.onControlTargetChange(restart);
    restart();

    return () => {
      cancelled = true;
      loopGeneration += 1;
      unlisten();
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, [sessionMgr, setConnectionHealth]);
}
