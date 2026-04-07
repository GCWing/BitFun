import { useEffect, useRef, useSyncExternalStore } from 'react';
import { ExplorerController } from '../controller/ExplorerController';
import type { ExplorerControllerConfig, ExplorerSnapshot } from '../types/explorer';

const EMPTY_SNAPSHOT: ExplorerSnapshot = {
  rootPath: undefined,
  fileTree: [],
  selectedFile: undefined,
  expandedFolders: new Set(),
  loading: false,
  error: undefined,
  loadingPaths: new Set(),
  options: {
    enablePathCompression: true,
    showHiddenFiles: false,
    sortBy: 'name',
    sortOrder: 'asc',
    excludePatterns: [],
  },
};

export function useExplorerController(config: ExplorerControllerConfig): ExplorerController {
  const controllerRef = useRef<ExplorerController | null>(null);

  if (!controllerRef.current) {
    controllerRef.current = new ExplorerController();
  }

  useEffect(() => {
    const controller = controllerRef.current!;
    void controller.configure(config);
  }, [config]);

  useEffect(() => {
    const controller = controllerRef.current!;
    return () => controller.dispose();
  }, []);

  return controllerRef.current;
}

export function useExplorerSnapshot(controller: ExplorerController): ExplorerSnapshot {
  return useSyncExternalStore(
    (listener) => controller.subscribe(listener),
    () => controller.getSnapshot(),
    () => EMPTY_SNAPSHOT
  );
}

