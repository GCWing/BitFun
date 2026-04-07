import {
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

export type ExplorerSearchMode = 'filenames' | 'content' | 'both';

export interface ExplorerSearchOptions {
  caseSensitive: boolean;
  useRegex: boolean;
  wholeWord: boolean;
}

export interface ExplorerSearchPhase {
  phase: 'idle' | 'searching' | 'complete';
  mode: ExplorerSearchMode;
  filenameComplete: boolean;
  contentComplete: boolean;
}

export interface UseExplorerSearchOptions {
  workspacePath?: string;
  initialMode?: ExplorerSearchMode;
  filenameSearchDebounce?: number;
  contentSearchDebounce?: number;
  minFilenameLength?: number;
  minContentLength?: number;
  filenameMaxResults?: number;
  contentMaxResults?: number;
}

export interface UseExplorerSearchResult {
  query: string;
  setQuery: (query: string) => void;
  triggerSearch: (query: string) => void;
  searchMode: ExplorerSearchMode;
  setSearchMode: Dispatch<SetStateAction<ExplorerSearchMode>>;
  filenameResults: FileSearchResult[];
  contentResults: FileSearchResult[];
  allResults: FileSearchResult[];
  searchPhase: ExplorerSearchPhase;
  filenameLimit: number;
  contentLimit: number;
  filenameTruncated: boolean;
  contentTruncated: boolean;
  hasTruncatedResults: boolean;
  isSearching: boolean;
  error: string | null;
  searchOptions: ExplorerSearchOptions;
  setSearchOptions: Dispatch<SetStateAction<ExplorerSearchOptions>>;
  clearSearch: () => void;
}

function buildIdlePhase(mode: ExplorerSearchMode): ExplorerSearchPhase {
  return {
    phase: 'idle',
    mode,
    filenameComplete: mode === 'content',
    contentComplete: mode === 'filenames',
  };
}

function buildSearchingPhase(
  mode: ExplorerSearchMode,
  shouldRunFilename: boolean,
  shouldRunContent: boolean
): ExplorerSearchPhase {
  return {
    phase: 'searching',
    mode,
    filenameComplete: !shouldRunFilename,
    contentComplete: !shouldRunContent,
  };
}

export function useExplorerSearch(
  options: UseExplorerSearchOptions = {}
): UseExplorerSearchResult {
  const {
    workspacePath,
    initialMode = 'filenames',
    filenameSearchDebounce = 200,
    contentSearchDebounce = 300,
    minFilenameLength = 1,
    minContentLength = 2,
    filenameMaxResults = 500,
    contentMaxResults = 1000,
  } = options;

  const [query, setQueryState] = useState('');
  const [searchMode, setSearchMode] = useState<ExplorerSearchMode>(initialMode);
  const [filenameResults, setFilenameResults] = useState<FileSearchResult[]>([]);
  const [contentResults, setContentResults] = useState<FileSearchResult[]>([]);
  const [searchPhase, setSearchPhase] = useState<ExplorerSearchPhase>(() => buildIdlePhase(initialMode));
  const [filenameLimit, setFilenameLimit] = useState(filenameMaxResults);
  const [contentLimit, setContentLimit] = useState(contentMaxResults);
  const [filenameTruncated, setFilenameTruncated] = useState(false);
  const [contentTruncated, setContentTruncated] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [searchOptions, setSearchOptions] = useState<ExplorerSearchOptions>({
    caseSensitive: false,
    useRegex: false,
    wholeWord: false,
  });

  const filenameAbortController = useRef<AbortController | null>(null);
  const contentAbortController = useRef<AbortController | null>(null);
  const filenameTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const contentTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const searchRunIdRef = useRef(0);
  const requestIdRef = useRef(0);

  const nextSearchId = useCallback((kind: 'filenames' | 'content'): string => {
    requestIdRef.current += 1;
    return `explorer-${kind}-${requestIdRef.current}`;
  }, []);

  const cancelFilenameSearch = useCallback(() => {
    filenameAbortController.current?.abort();
    filenameAbortController.current = null;
    if (filenameTimer.current) {
      clearTimeout(filenameTimer.current);
      filenameTimer.current = null;
    }
  }, []);

  const cancelContentSearch = useCallback(() => {
    contentAbortController.current?.abort();
    contentAbortController.current = null;
    if (contentTimer.current) {
      clearTimeout(contentTimer.current);
      contentTimer.current = null;
    }
  }, []);

  const cancelAllSearches = useCallback(() => {
    cancelFilenameSearch();
    cancelContentSearch();
  }, [cancelContentSearch, cancelFilenameSearch]);

  const clearSearch = useCallback(() => {
    cancelAllSearches();
    searchRunIdRef.current += 1;
    setQueryState('');
    setFilenameResults([]);
    setContentResults([]);
    setFilenameLimit(filenameMaxResults);
    setContentLimit(contentMaxResults);
    setFilenameTruncated(false);
    setContentTruncated(false);
    setSearchPhase(buildIdlePhase(searchMode));
    setError(null);
  }, [cancelAllSearches, contentMaxResults, filenameMaxResults, searchMode]);

  const executeFilenameSearch = useCallback(
    async (searchQuery: string, runId: number) => {
      if (!workspacePath) {
        return;
      }

      const controller = new AbortController();
      filenameAbortController.current = controller;

      try {
        const response = await workspaceAPI.searchFilenamesOnlyDetailed(
          workspacePath,
          searchQuery,
          searchOptions.caseSensitive,
          searchOptions.useRegex,
          searchOptions.wholeWord,
          nextSearchId('filenames'),
          filenameMaxResults,
          true,
          controller.signal
        );

        if (runId !== searchRunIdRef.current) {
          return;
        }

        setFilenameResults(response.results);
        setFilenameLimit(response.limit);
        setFilenameTruncated(response.truncated);
        setSearchPhase((prev) => {
          const nextPhase = { ...prev, filenameComplete: true };
          return {
            ...nextPhase,
            phase: nextPhase.filenameComplete && nextPhase.contentComplete ? 'complete' : 'searching',
          };
        });
      } catch (err) {
        if (err instanceof DOMException && err.name === 'AbortError') {
          return;
        }

        if (runId !== searchRunIdRef.current) {
          return;
        }

        log.error('Filename search failed', { query: searchQuery, error: err });
        setError(err instanceof Error ? err.message : 'Filename search failed');
        setSearchPhase((prev) => ({ ...prev, filenameComplete: true, phase: prev.contentComplete ? 'complete' : prev.phase }));
      }
    },
    [
      workspacePath,
      searchOptions.caseSensitive,
      searchOptions.useRegex,
      searchOptions.wholeWord,
      nextSearchId,
      filenameMaxResults,
    ]
  );

  const executeContentSearch = useCallback(
    async (searchQuery: string, runId: number) => {
      if (!workspacePath) {
        return;
      }

      const controller = new AbortController();
      contentAbortController.current = controller;

      try {
        const response = await workspaceAPI.searchContentOnlyDetailed(
          workspacePath,
          searchQuery,
          searchOptions.caseSensitive,
          searchOptions.useRegex,
          searchOptions.wholeWord,
          nextSearchId('content'),
          contentMaxResults,
          controller.signal
        );

        if (runId !== searchRunIdRef.current) {
          return;
        }

        setContentResults(response.results);
        setContentLimit(response.limit);
        setContentTruncated(response.truncated);
        setSearchPhase((prev) => {
          const nextPhase = { ...prev, contentComplete: true };
          return {
            ...nextPhase,
            phase: nextPhase.filenameComplete && nextPhase.contentComplete ? 'complete' : 'searching',
          };
        });
      } catch (err) {
        if (err instanceof DOMException && err.name === 'AbortError') {
          return;
        }

        if (runId !== searchRunIdRef.current) {
          return;
        }

        log.error('Content search failed', { query: searchQuery, error: err });
        setError(err instanceof Error ? err.message : 'Content search failed');
        setSearchPhase((prev) => ({ ...prev, contentComplete: true, phase: prev.filenameComplete ? 'complete' : prev.phase }));
      }
    },
    [
      workspacePath,
      searchOptions.caseSensitive,
      searchOptions.useRegex,
      searchOptions.wholeWord,
      nextSearchId,
      contentMaxResults,
    ]
  );

  const setQuery = useCallback((newQuery: string) => {
    setQueryState(newQuery);
  }, []);

  const triggerSearch = useCallback((newQuery: string) => {
    setQueryState(newQuery);
  }, []);

  useEffect(() => {
    cancelAllSearches();

    const trimmedQuery = query.trim();
    const shouldRunFilename =
      searchMode !== 'content' && trimmedQuery.length >= minFilenameLength;
    const shouldRunContent =
      searchMode !== 'filenames' && trimmedQuery.length >= minContentLength;

    if (!workspacePath || (!shouldRunFilename && !shouldRunContent)) {
      setFilenameResults([]);
      setContentResults([]);
      setFilenameLimit(filenameMaxResults);
      setContentLimit(contentMaxResults);
      setFilenameTruncated(false);
      setContentTruncated(false);
      setSearchPhase(buildIdlePhase(searchMode));
      setError(null);
      return;
    }

    const runId = ++searchRunIdRef.current;
    setError(null);
    setFilenameLimit(filenameMaxResults);
    setContentLimit(contentMaxResults);
    setFilenameTruncated(false);
    setContentTruncated(false);
    setSearchPhase(buildSearchingPhase(searchMode, shouldRunFilename, shouldRunContent));

    if (!shouldRunFilename) {
      setFilenameResults([]);
    } else {
      filenameTimer.current = setTimeout(() => {
        void executeFilenameSearch(trimmedQuery, runId);
      }, filenameSearchDebounce);
    }

    if (!shouldRunContent) {
      setContentResults([]);
    } else {
      contentTimer.current = setTimeout(() => {
        void executeContentSearch(trimmedQuery, runId);
      }, contentSearchDebounce);
    }

    return () => {
      cancelAllSearches();
    };
  }, [
    cancelAllSearches,
    contentMaxResults,
    contentSearchDebounce,
    executeContentSearch,
    executeFilenameSearch,
    filenameMaxResults,
    filenameSearchDebounce,
    minContentLength,
    minFilenameLength,
    query,
    searchMode,
    workspacePath,
  ]);

  useEffect(() => {
    return () => {
      cancelAllSearches();
    };
  }, [cancelAllSearches]);

  const allResults = useMemo(() => {
    switch (searchMode) {
      case 'filenames':
        return filenameResults;
      case 'content':
        return contentResults;
      default:
        return [...filenameResults, ...contentResults];
    }
  }, [contentResults, filenameResults, searchMode]);

  return {
    query,
    setQuery,
    triggerSearch,
    searchMode,
    setSearchMode,
    filenameResults,
    contentResults,
    allResults,
    searchPhase,
    filenameLimit,
    contentLimit,
    filenameTruncated,
    contentTruncated,
    hasTruncatedResults: filenameTruncated || contentTruncated,
    isSearching: searchPhase.phase === 'searching',
    error,
    searchOptions,
    setSearchOptions,
    clearSearch,
  };
}
