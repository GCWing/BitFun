export { ExplorerController } from './controller/ExplorerController';
export { useExplorerController, useExplorerSnapshot } from './hooks/useExplorerController';
export { tauriExplorerFileSystemProvider } from './provider/TauriExplorerFileSystemProvider';
export {
  useExplorerSearch,
  type ExplorerSearchMode,
  type ExplorerSearchOptions,
  type ExplorerSearchPhase,
  type UseExplorerSearchOptions,
  type UseExplorerSearchResult,
} from './search/useExplorerSearch';
export {
  useWorkspaceSearchIndex,
  type UseWorkspaceSearchIndexOptions,
  type UseWorkspaceSearchIndexResult,
} from './search/useWorkspaceSearchIndex';
export { filterTreeByPredicate, filterTreeBySearch } from './search/treeFilter';
export type {
  ExplorerChildrenRequest,
  ExplorerControllerConfig,
  ExplorerFileSystemProvider,
  ExplorerNodeRecord,
  ExplorerSnapshot,
} from './types/explorer';
