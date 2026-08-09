import { useCallback, useEffect, useState } from 'react';
import { createLogger } from '@/shared/utils/logger';
import { dispatchApi } from './dispatchApi';
import type { DispatchTargetOption } from './types';

const log = createLogger('DispatchTargets');

export function useDispatchTargets(enabled = true): {
  targets: DispatchTargetOption[];
  loading: boolean;
  error: boolean;
  refresh: () => Promise<void>;
} {
  const [targets, setTargets] = useState<DispatchTargetOption[]>([]);
  const [loading, setLoading] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState(false);

  const refresh = useCallback(async () => {
    if (!enabled) return;
    setLoading(true);
    setError(false);
    try {
      const nextTargets = await dispatchApi.listTargets();
      setTargets(nextTargets.filter(
        target => target.kind === 'local' || target.kind === 'ssh' || target.kind === 'device',
      ));
    } catch (nextError) {
      log.warn('Failed to list dispatch targets', { error: nextError });
      setError(true);
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
