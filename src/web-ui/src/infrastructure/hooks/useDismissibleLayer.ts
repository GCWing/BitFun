import { useEffect, useId, useRef, useSyncExternalStore } from 'react';
import { dismissibleLayerManager } from '@/infrastructure/services/DismissibleLayerManager';
import type { ShortcutScope } from '@/shared/types/shortcut';

interface UseDismissibleLayerOptions {
  enabled: boolean;
  scope: ShortcutScope;
  onDismiss: () => void;
  id?: string;
}

export function useDismissibleLayer({
  enabled,
  scope,
  onDismiss,
  id,
}: UseDismissibleLayerOptions): void {
  const generatedId = useId();
  const layerIdRef = useRef(id ?? generatedId);
  const onDismissRef = useRef(onDismiss);
  onDismissRef.current = onDismiss;

  useEffect(() => {
    if (!enabled) {
      return;
    }

    return dismissibleLayerManager.register({
      id: layerIdRef.current,
      scope,
      onDismiss: () => onDismissRef.current(),
    });
  }, [enabled, scope]);
}

export function useHasDismissibleLayer(scope?: ShortcutScope): boolean {
  useSyncExternalStore(
    dismissibleLayerManager.subscribe.bind(dismissibleLayerManager),
    dismissibleLayerManager.getVersion.bind(dismissibleLayerManager),
    () => 0
  );

  return dismissibleLayerManager.hasLayers(scope);
}
