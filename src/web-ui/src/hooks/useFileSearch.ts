import {
  useExplorerSearch,
  type ExplorerSearchOptions,
  type ExplorerSearchPhase,
  type UseExplorerSearchOptions,
  type UseExplorerSearchResult,
} from '@/tools/file-explorer';

export type SearchOptions = ExplorerSearchOptions;
export type SearchPhase = ExplorerSearchPhase;
export type UseFileSearchOptions = UseExplorerSearchOptions;
export type UseFileSearchResult = UseExplorerSearchResult;

export function useFileSearch(options: UseFileSearchOptions = {}): UseFileSearchResult {
  return useExplorerSearch(options);
}

export default useFileSearch;
