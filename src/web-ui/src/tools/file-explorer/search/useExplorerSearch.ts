import {
  startTransition,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type Dispatch,
  type SetStateAction,
} from 'react';
import { workspaceAPI } from '@/infrastructure/api';
import type { FileSearchResult } from '@/infrastructure/api/service-api/tauri-commands';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('useExplorerSearch');

export interface ExplorerSearchOptions {
  caseSensitive: boolean;
  useRegex: boolean;
  wholeWord: boolean;
}

export interface ExplorerSearchPhase {
  phase: 'idle' | 'filename' | 'content' | 'complete';
  filenameComplete: boolean;
  contentComplete: boolean;
}

export interface UseExplorerSearchOptions {
  workspacePath?: string;
  enableContentSearch?: boolean;
  contentSearchDebounce?: number;
  minSearchLength?: number;
}

export interface UseExplorerSearchResult {
  query: string;
  setQuery: (query: string) => void;
  filenameResults: FileSearchResult[];
  contentResults: FileSearchResult[];
  allResults: FileSearchResult[];
  searchPhase: ExplorerSearchPhase;
  isSearching: boolean;
  error: string | null;
  searchOptions: ExplorerSearchOptions;
  setSearchOptions: Dispatch<SetStateAction<ExplorerSearchOptions>>;
  clearSearch: () => void;
  triggerSearch: (query: string) => void;
}

export function useExplorerSearch(
  options: UseExplorerSearchOptions = {}
): UseExplorerSearchResult {
  const {
    workspacePath,
    enableContentSearch = true,
    contentSearchDebounce = 150,
    minSearchLength = 1,
  } = options;

  const [query, setQueryState] = useState('');
  const [filenameResults, setFilenameResults] = useState<FileSearchResult[]>([]);
  const [contentResults, setContentResults] = useState<FileSearchResult[]>([]);
  const [searchPhase, setSearchPhase] = useState<ExplorerSearchPhase>({
    phase: 'idle',
    filenameComplete: false,
    contentComplete: false,
  });
  const [error, setError] = useState<string | null>(null);
  const [searchOptions, setSearchOptions] = useState<ExplorerSearchOptions>({
    caseSensitive: false,
    useRegex: false,
    wholeWord: false,
  });

  const filenameAbortController = useRef<AbortController | null>(null);
  const contentAbortController = useRef<AbortController | null>(null);
  const contentSearchTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const searchIdRef = useRef(0);

  const clearSearch = useCallback(() => {
    filenameAbortController.current?.abort();
    contentAbortController.current?.abort();
    if (contentSearchTimer.current) {
      clearTimeout(contentSearchTimer.current);
      contentSearchTimer.current = null;
    }

    setQueryState('');
    setFilenameResults([]);
    setContentResults([]);
    setSearchPhase({
      phase: 'idle',
      filenameComplete: false,
      contentComplete: false,
    });
    setError(null);
  }, []);

  const executeFilenameSearch = useCallback(
    async (searchQuery: string, searchId: number) => {
      if (!workspacePath || searchQuery.trim().length < minSearchLength) {
        setFilenameResults([]);
        return;
      }

      filenameAbortController.current?.abort();
      filenameAbortController.current = new AbortController();

      try {
        setSearchPhase((prev) => ({
          ...prev,
          phase: 'filename',
          filenameComplete: false,
        }));

        const results = await workspaceAPI.searchFilenamesOnly(
          workspacePath,
          searchQuery.trim(),
          searchOptions.caseSensitive,
          searchOptions.useRegex,
          searchOptions.wholeWord,
          filenameAbortController.current.signal
        );

        if (searchId !== searchIdRef.current) {
          return;
        }

        setFilenameResults(results);
        setSearchPhase((prev) => ({
          ...prev,
          filenameComplete: true,
          phase: enableContentSearch ? 'content' : 'complete',
        }));
      } catch (err) {
        if (err instanceof DOMException && err.name === 'AbortError') {
          return;
        }
        if (searchId !== searchIdRef.current) {
          return;
        }

        log.error('Filename search failed', { query: searchQuery, error: err });
        setError(err instanceof Error ? err.message : 'Search failed');
      }
    },
    [workspacePath, minSearchLength, searchOptions, enableContentSearch]
  );

  const executeContentSearch = useCallback(
    async (searchQuery: string, searchId: number) => {
      if (
        !workspacePath ||
        !enableContentSearch ||
        searchQuery.trim().length < minSearchLength
      ) {
        setContentResults([]);
        return;
      }

      contentAbortController.current?.abort();
      contentAbortController.current = new AbortController();

      try {
        const results = await workspaceAPI.searchContentOnly(
          workspacePath,
          searchQuery.trim(),
          searchOptions.caseSensitive,
          searchOptions.useRegex,
          searchOptions.wholeWord,
          contentAbortController.current.signal
        );

        if (searchId !== searchIdRef.current) {
          return;
        }

        startTransition(() => {
          setContentResults(results);
          setSearchPhase((prev) => ({
            ...prev,
            contentComplete: true,
            phase: 'complete',
          }));
        });
      } catch (err) {
        if (err instanceof DOMException && err.name === 'AbortError') {
          return;
        }
        if (searchId !== searchIdRef.current) {
          return;
        }

        log.error('Content search failed', { query: searchQuery, error: err });
      }
    },
    [workspacePath, enableContentSearch, minSearchLength, searchOptions]
  );

  const triggerSearch = useCallback(
    (searchQuery: string) => {
      const newSearchId = ++searchIdRef.current;

      if (contentSearchTimer.current) {
        clearTimeout(contentSearchTimer.current);
        contentSearchTimer.current = null;
      }

      setError(null);

      if (!searchQuery.trim() || searchQuery.trim().length < minSearchLength) {
        clearSearch();
        return;
      }

      executeFilenameSearch(searchQuery, newSearchId);

      if (enableContentSearch) {
        contentSearchTimer.current = setTimeout(() => {
          void executeContentSearch(searchQuery, newSearchId);
        }, contentSearchDebounce);
      }
    },
    [
      minSearchLength,
      clearSearch,
      executeFilenameSearch,
      executeContentSearch,
      enableContentSearch,
      contentSearchDebounce,
    ]
  );

  const setQuery = useCallback(
    (newQuery: string) => {
      setQueryState(newQuery);
      triggerSearch(newQuery);
    },
    [triggerSearch]
  );

  useEffect(() => {
    if (query.trim().length >= minSearchLength) {
      triggerSearch(query);
    }
  }, [searchOptions, minSearchLength, query, triggerSearch]);

  useEffect(() => {
    return () => {
      filenameAbortController.current?.abort();
      contentAbortController.current?.abort();
      if (contentSearchTimer.current) {
        clearTimeout(contentSearchTimer.current);
      }
    };
  }, []);

  const allResults = useMemo(
    () => [...filenameResults, ...contentResults],
    [filenameResults, contentResults]
  );

  const isSearching =
    searchPhase.phase === 'filename' ||
    (searchPhase.phase === 'content' && !searchPhase.contentComplete);

  return {
    query,
    setQuery,
    filenameResults,
    contentResults,
    allResults,
    searchPhase,
    isSearching,
    error,
    searchOptions,
    setSearchOptions,
    clearSearch,
    triggerSearch,
  };
}
