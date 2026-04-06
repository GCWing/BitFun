import type { ExplorerSnapshot } from '../types/explorer';

export function projectExplorerSnapshot(snapshot: ExplorerSnapshot): ExplorerSnapshot {
  return {
    ...snapshot,
    expandedFolders: new Set(snapshot.expandedFolders),
    loadingPaths: new Set(snapshot.loadingPaths),
    options: {
      ...snapshot.options,
      excludePatterns: [...(snapshot.options.excludePatterns ?? [])],
    },
    fileTree: snapshot.fileTree.map(node => ({ ...node })),
  };
}

