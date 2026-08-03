import { useSyncExternalStore } from 'react';
import { appearanceService } from '../index';
import type { AppearanceImportOptions } from '../types';

const getPreviewAsset = (id: string) => appearanceService.getPreviewAsset(id);

export function useAppearance() {
  const snapshot = useSyncExternalStore(
    listener => appearanceService.subscribe(listener),
    () => appearanceService.getSnapshot(),
    () => appearanceService.getSnapshot(),
  );
  return {
    ...snapshot,
    select: (id: string) => appearanceService.select(id),
    getPackage: (id: string) => appearanceService.getPackage(id),
    getPreviewAsset,
    importPackage: (source: ArrayBuffer, options?: AppearanceImportOptions) => (
      appearanceService.importPackage(source, options)
    ),
    exportPackage: (id: string) => appearanceService.exportPackage(id),
    activate: (id: string) => appearanceService.activate(id),
    deactivate: () => appearanceService.deactivate(),
    deletePackage: (id: string) => appearanceService.deletePackage(id),
  };
}
