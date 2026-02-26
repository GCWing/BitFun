/**
 * Global search component.
 * Used in the Editor-mode header for filename and content search.
 *
 * Performance optimizations:
 * - Filename search responds immediately (no debounce).
 * - Content search is debounced by 150ms.
 * - Automatically cancels previous requests.
 * - Returns results in phases (filename first, then content).
 * - Uses React.memo to optimize result items.
 * - Limits result count to reduce DOM rendering.
 */

import React, { useCallback, useEffect, useRef, useMemo, memo, useState, startTransition } from 'react';
import { CaseSensitive, Regex, WholeWord, Loader2, MoreHorizontal } from 'lucide-react';
import { Search, Tooltip } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { useWorkspaceContext } from '../../../infrastructure/contexts/WorkspaceContext';
import { useFileSearch } from '@/hooks';
import type { FileSearchResult } from '../../../infrastructure/api/service-api/tauri-commands';
import { createLogger } from '@/shared/utils/logger';
import './GlobalSearch.scss';

const log = createLogger('GlobalSearch');

// Initial result count and load-more batch size
const INITIAL_DISPLAY_COUNT = 50;
const LOAD_MORE_COUNT = 50;

interface GlobalSearchProps {
  className?: string;
  onSearchResultClick?: (result: FileSearchResult) => void;
}

// Result item component (memoized)
interface ResultItemProps {
  result: FileSearchResult;
  onClick: () => void;
}

const ResultItem = memo<ResultItemProps>(({ result, onClick }) => {
  const { t } = useI18n('tools');
  const lineNumberPrefix = result.lineNumber
    ? t('search.global.lineNumberPrefix', { line: result.lineNumber })
    : '';

  return (
    <button
      className="bitfun-global-search__result-item"
      onClick={onClick}
    >
      <div className="bitfun-global-search__result-file">
        {result.path}
      </div>
      {result.matchedContent && (
        <div className="bitfun-global-search__result-preview">
          {lineNumberPrefix}{result.matchedContent.trim()}
        </div>
      )}
    </button>
  );
});

ResultItem.displayName = 'ResultItem';

// Search phase indicator
interface SearchPhaseIndicatorProps {
  filenameComplete: boolean;
  contentComplete: boolean;
  isSearching: boolean;
}

const SearchPhaseIndicator = memo<SearchPhaseIndicatorProps>(({ 
  filenameComplete, 
  contentComplete, 
  isSearching 
}) => {
  const { t } = useI18n('tools');
  if (!isSearching && filenameComplete && contentComplete) return null;
  
  return (
    <div className="bitfun-global-search__phase-indicator">
      {isSearching && <Loader2 size={12} className="bitfun-global-search__spinner" />}
      <span className="bitfun-global-search__phase-text">
        {!filenameComplete
          ? t('search.global.phaseFilename')
          : !contentComplete
            ? t('search.global.phaseContent')
            : ''}
      </span>
    </div>
  );
});

SearchPhaseIndicator.displayName = 'SearchPhaseIndicator';

export const GlobalSearch: React.FC<GlobalSearchProps> = ({
  className = '',
  onSearchResultClick
}) => {
  const { t } = useI18n('tools');
  const { workspacePath } = useWorkspaceContext();
  const searchContainerRef = useRef<HTMLDivElement>(null);
  const resultsListRef = useRef<HTMLDivElement>(null);
  const [showResults, setShowResults] = useState(false);
  const [displayCount, setDisplayCount] = useState(INITIAL_DISPLAY_COUNT);

  // Use the optimized file search hook
  const {
    query,
    setQuery,
    allResults,
    searchPhase,
    isSearching,
    searchOptions,
    setSearchOptions,
    clearSearch,
  } = useFileSearch({
    workspacePath,
    enableContentSearch: true,
    contentSearchDebounce: 150, // 150ms debounce for content search
    minSearchLength: 1,
  });

  // Reset display count when results change
  useEffect(() => {
    setDisplayCount(INITIAL_DISPLAY_COUNT);
  }, [query]);

  // Handle result click
  const handleResultClick = useCallback(async (result: FileSearchResult) => {
    // If line number exists, jump to that line via EditorJumpService
    if (result.lineNumber) {
      const { editorJumpService } = await import('@/shared/services/EditorJumpService');
      await editorJumpService.jumpToFile(result.path, result.lineNumber, 1);
    } else {
      // Otherwise, open the file
      const { fileTabManager } = await import('@/shared/services/FileTabManager');
      fileTabManager.openFile({
        filePath: result.path,
        fileName: result.name,
        workspacePath
      });
    }
    
    onSearchResultClick?.(result);
    setShowResults(false);
  }, [onSearchResultClick, workspacePath]);

  // Clear search
  const handleClearSearch = useCallback(() => {
    clearSearch();
    setShowResults(false);
  }, [clearSearch]);

  // Close results when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (searchContainerRef.current && !searchContainerRef.current.contains(event.target as Node)) {
        setShowResults(false);
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  // Handle keyboard navigation
  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      setShowResults(false);
    }
  }, []);

  // On input focus, re-show results
  const handleFocus = useCallback(() => {
    if (query.trim() && allResults.length > 0) {
      setShowResults(true);
    }
  }, [query, allResults.length]);

  // Handle query changes
  const handleQueryChange = useCallback((val: string) => {
    setQuery(val);
    if (val.trim()) {
      setShowResults(true);
    } else {
      setShowResults(false);
    }
  }, [setQuery]);

  // Show results when available
  useEffect(() => {
    if (query.trim() && allResults.length > 0) {
      setShowResults(true);
    }
  }, [allResults.length, query]);

  // Limit displayed results
  const displayResults = useMemo(() => {
    return allResults.slice(0, displayCount);
  }, [allResults, displayCount]);

  const hasMoreResults = allResults.length > displayCount;
  const remainingCount = allResults.length - displayCount;

  // Load more results
  const handleLoadMore = useCallback(() => {
    startTransition(() => {
      setDisplayCount(prev => prev + LOAD_MORE_COUNT);
    });
  }, []);

  return (
    <div 
      className={`bitfun-global-search ${className}`}
      ref={searchContainerRef}
      onKeyDown={handleKeyDown}
    >
      <Search
        placeholder={workspacePath ? t('search.global.placeholder') : t('search.global.placeholderDisabled')}
        value={query}
        onChange={handleQueryChange}
        onClear={handleClearSearch}
        onFocus={handleFocus}
        clearable
        disabled={!workspacePath}
        loading={isSearching}
        size="small"
        suffixContent={
          /* Search options */
          <div className="bitfun-global-search__options">
            <Tooltip content={t('search.matchCase')}>
              <button
                className={`bitfun-global-search__option ${searchOptions.caseSensitive ? 'active' : ''}`}
                onClick={() => setSearchOptions(prev => ({ ...prev, caseSensitive: !prev.caseSensitive }))}
                disabled={!workspacePath}
              >
                <CaseSensitive size={12} />
              </button>
            </Tooltip>
            <Tooltip content={t('search.matchWholeWord')}>
              <button
                className={`bitfun-global-search__option ${searchOptions.wholeWord ? 'active' : ''}`}
                onClick={() => setSearchOptions(prev => ({ ...prev, wholeWord: !prev.wholeWord }))}
                disabled={!workspacePath}
              >
                <WholeWord size={12} />
              </button>
            </Tooltip>
            <Tooltip content={t('search.useRegex')}>
              <button
                className={`bitfun-global-search__option ${searchOptions.useRegex ? 'active' : ''}`}
                onClick={() => setSearchOptions(prev => ({ ...prev, useRegex: !prev.useRegex }))}
                disabled={!workspacePath}
              >
                <Regex size={12} />
              </button>
            </Tooltip>
          </div>
        }
      />

      {/* Search results dropdown */}
      {showResults && (query.trim() || allResults.length > 0) && (
        <div className="bitfun-global-search__results">
          <div className="bitfun-global-search__results-header">
            <span className="bitfun-global-search__results-count">
              {allResults.length > 0 ? (
                <>
                  {t('search.global.resultsFound', { count: allResults.length })}
                  {hasMoreResults && (
                    <span className="bitfun-global-search__results-showing">
                      {t('search.global.resultsShowing', { count: displayCount })}
                    </span>
                  )}
                </>
              ) : (
                isSearching ? '' : t('search.noResults')
              )}
            </span>
            <SearchPhaseIndicator 
              filenameComplete={searchPhase.filenameComplete}
              contentComplete={searchPhase.contentComplete}
              isSearching={isSearching}
            />
          </div>
          
          {displayResults.length > 0 && (
            <div className="bitfun-global-search__results-list" ref={resultsListRef}>
              {displayResults.map((result, index) => (
                <ResultItem
                  key={`${result.path}-${result.lineNumber || 0}-${index}`}
                  result={result}
                  onClick={() => handleResultClick(result)}
                />
              ))}
              
              {/* Load more button */}
              {hasMoreResults && (
                <button 
                  className="bitfun-global-search__load-more"
                  onClick={handleLoadMore}
                >
                  <MoreHorizontal size={14} />
                  <span>{t('search.global.loadMore', { count: remainingCount })}</span>
                </button>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
};
