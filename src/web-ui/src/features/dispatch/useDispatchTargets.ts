import { useCallback, useEffect, useState } from 'react';
import { createLogger } from '@/shared/utils/logger';
import { dispatchApi } from './dispatchApi';
import type { DispatchTargetOption } from './types';

const log = createLogger('DispatchTargets');

export function useDispatchTargets(enabled = true): {
  targets: DispatchTargetOption[];
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
} {
  const [targets, setTargets] = useState<DispatchTargetOption[]>([]);
  const [loading, setLoading] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (!enabled) return;
    setLoading(true);
    setError(null);
    try {
      const nextTargets = await dispatchApi.listTargets();
      setTargets(nextTargets.filter(
        target => target.kind === 'local' || target.kind === 'ssh' || target.kind === 'device',
      ));
    } catch (nextError) {
      const message = nextError instanceof Error ? nextError.message : String(nextError);
      log.warn('Failed to list dispatch targets', { error: nextError });
      setError(message);
      setTargets([{ kind: 'local', displayName: 'Local' }]);
    } finally {
      setLoading(false);
      setLoaded(true);
    }
  }, [enabled]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Opening the picker enables this hook one render before the effect starts
  // the request. Treat that first render as loading so users never see a
  // misleading empty-target message flash before saved SSH targets arrive.
  return { targets, loading: loading || (enabled && !loaded), error, refresh };
}
