import React, { useMemo, useState, useCallback, startTransition, memo, useEffect, useRef } from 'react';
import { File, FileText, Folder, ChevronRight, ChevronDown } from 'lucide-react';
import type { FileSearchResult } from '@/infrastructure/api/service-api/tauri-commands';
import { useI18n } from '@/infrastructure/i18n';
import './FileSearchResults.scss';

const INITIAL_DISPLAY_COUNT = 50;
const LOAD_MORE_COUNT = 50;

interface FileSearchResultsProps {
  results: FileSearchResult[];
  searchQuery: string;
  onFileSelect: (filePath: string, fileName: string) => void;
  className?: string;
}

interface GroupedResult {
  path: string;
  name: string;
  isDirectory: boolean;
  fileNameMatch?: FileSearchResult;
  contentMatches: FileSearchResult[];
}

function truncateLine(line: string, query: string, maxLength: number = 200): string {
  if (!line || line.length <= maxLength) {
    return line;
  }
  
  const lowerLine = line.toLowerCase();
  const lowerQuery = query.toLowerCase();
  const matchIndex = lowerLine.indexOf(lowerQuery);
  
  if (matchIndex === -1) {
    return line.substring(0, maxLength) + '...';
  }
  
  const beforeChars = 60;
  const afterChars = 60;
  
  const start = Math.max(0, matchIndex - beforeChars);
  const end = Math.min(line.length, matchIndex + query.length + afterChars);
  
  let result = '';
  if (start > 0) result += '...';
  result += line.substring(start, end);
  if (end < line.length) result += '...';
  
  return result;
}

interface HighlightedTextProps {
  text: string;
  query: string;
}

const HighlightedText = memo<HighlightedTextProps>(({ text, query }) => {
  if (!query || !text) return <>{text}</>;
  
  const lowerText = text.toLowerCase();
  const lowerQuery = query.toLowerCase();
  const parts: React.ReactNode[] = [];
  
  let lastIndex = 0;
  let matchIndex = lowerText.indexOf(lowerQuery, lastIndex);
  let keyIndex = 0;
  
  while (matchIndex !== -1) {
    if (matchIndex > lastIndex) {
      parts.push(<span key={keyIndex++}>{text.substring(lastIndex, matchIndex)}</span>);
    }
    parts.push(
      <mark key={keyIndex++} className="bitfun-search-results__highlight">
        {text.substring(matchIndex, matchIndex + query.length)}
      </mark>
    );
    lastIndex = matchIndex + query.length;
    matchIndex = lowerText.indexOf(lowerQuery, lastIndex);
  }
  
  if (lastIndex < text.length) {
    parts.push(<span key={keyIndex}>{text.substring(lastIndex)}</span>);
  }
  
  return <>{parts}</>;
});

HighlightedText.displayName = 'HighlightedText';

interface MatchItemProps {
  match: FileSearchResult;
  groupPath: string;
  groupName: string;
  searchQuery: string;
  onLineClick: (path: string, name: string, lineNumber?: number) => void;
}

const MatchItem = memo<MatchItemProps>(({ match, groupPath, groupName, searchQuery, onLineClick }) => {
  const truncatedContent = useMemo(
    () => truncateLine(match.matchedContent || '', searchQuery),
    [match.matchedContent, searchQuery]
  );

  return (
    <div
      className="bitfun-search-results__match"
      onClick={() => onLineClick(groupPath, groupName, match.lineNumber)}
    >
      <span className="bitfun-search-results__match-line">
        {match.lineNumber}
      </span>
      <span 
        className="bitfun-search-results__match-content"
        title={match.matchedContent || ''}
      >
        <code>
          <HighlightedText text={truncatedContent} query={searchQuery} />
        </code>
      </span>
    </div>
  );
});

MatchItem.displayName = 'MatchItem';

interface FileGroupProps {
  group: GroupedResult;
  isExpanded: boolean;
  searchQuery: string;
  onToggleExpand: (path: string) => void;
  onFileClick: (path: string, name: string) => void;
  onLineClick: (path: string, name: string, lineNumber?: number) => void;
}

const FileGroup = memo<FileGroupProps>(({ 
  group, 
  isExpanded, 
  searchQuery, 
  onToggleExpand, 
  onFileClick, 
  onLineClick 
}) => {
  const { t } = useI18n('tools');
  const hasContentMatches = group.contentMatches.length > 0;

  return (
    <div className="bitfun-search-results__group">
      <div className="bitfun-search-results__file">
        <div 
          className="bitfun-search-results__file-main"
          onClick={() => onFileClick(group.path, group.name)}
        >
          <span className="bitfun-search-results__file-icon">
            {group.isDirectory ? (
              <Folder size={16} />
            ) : (
              <File size={16} />
            )}
          </span>
          <span className="bitfun-search-results__file-info">
            <span className="bitfun-search-results__file-name">
              <HighlightedText text={group.name} query={searchQuery} />
            </span>
            <span className="bitfun-search-results__file-path">
              {group.path}
            </span>
          </span>
        </div>

        {hasContentMatches && (
          <span 
            className="bitfun-search-results__file-toggle"
            onClick={(e) => {
              e.stopPropagation();
              onToggleExpand(group.path);
            }}
            title={isExpanded ? t('search.collapse') : t('search.expand')}
          >
            {isExpanded ? (
              <ChevronDown size={12} />
            ) : (
              <ChevronRight size={12} />
            )}
            <span className="bitfun-search-results__file-toggle-count">
              {group.contentMatches.length}
            </span>
          </span>
        )}
      </div>

      {hasContentMatches && isExpanded && (
        <div className="bitfun-search-results__matches">
          {group.contentMatches.map((match, matchIndex) => (
            <MatchItem
              key={`${group.path}-${matchIndex}`}
              match={match}
              groupPath={group.path}
              groupName={group.name}
              searchQuery={searchQuery}
              onLineClick={onLineClick}
            />
          ))}
        </div>
      )}
    </div>
  );
});

FileGroup.displayName = 'FileGroup';

export const FileSearchResults: React.FC<FileSearchResultsProps> = ({
  results,
  searchQuery,
  onFileSelect,
  className = ''
}) => {
  const { t } = useI18n('tools');
  const [displayCount, setDisplayCount] = useState(INITIAL_DISPLAY_COUNT);
  const listRef = useRef<HTMLDivElement>(null);
  const pendingAutoLoadRef = useRef(false);
  const [manualExpandState, setManualExpandState] = useState<Map<string, boolean>>(new Map());

  const groupedResults = useMemo(() => {
    const groups = new Map<string, FileSearchResult[]>();
    
    for (const result of results) {
      const key = result.path;
      if (!groups.has(key)) {
        groups.set(key, []);
      }
      groups.get(key)!.push(result);
    }
    
    return Array.from(groups.entries()).map(([path, items]) => {
      const fileNameMatch = items.find(item => item.matchType === 'fileName');
      const contentMatches = items.filter(item => item.matchType === 'content');
      
      return {
        path,
        name: items[0].name,
        isDirectory: items[0].isDirectory,
        fileNameMatch,
        contentMatches
      };
    });
  }, [results]);

  const prevResultsRef = useRef(results);
  
  useEffect(() => {
    if (prevResultsRef.current !== results) {
      prevResultsRef.current = results;
      startTransition(() => {
        setDisplayCount(INITIAL_DISPLAY_COUNT);
        setManualExpandState(new Map());
      });
    }
  }, [results]);

  const visibleGroups = useMemo(() => {
    return groupedResults.slice(0, displayCount);
  }, [groupedResults, displayCount]);

  const hasMore = displayCount < groupedResults.length;

  const shouldDefaultExpand = groupedResults.length <= 100;
  
  const isExpanded = useCallback((path: string): boolean => {
    if (manualExpandState.has(path)) {
      return manualExpandState.get(path)!;
    }
    return shouldDefaultExpand;
  }, [manualExpandState, shouldDefaultExpand]);

  const toggleExpanded = useCallback((path: string) => {
    setManualExpandState(prev => {
      const newMap = new Map(prev);
      const currentState = newMap.has(path) ? newMap.get(path)! : shouldDefaultExpand;
      newMap.set(path, !currentState);
      return newMap;
    });
  }, [shouldDefaultExpand]);

  const handleLoadMore = useCallback(() => {
    pendingAutoLoadRef.current = true;
    startTransition(() => {
      setDisplayCount(prev => Math.min(prev + LOAD_MORE_COUNT, groupedResults.length));
    });
  }, [groupedResults.length]);

  useEffect(() => {
    pendingAutoLoadRef.current = false;
  }, [displayCount]);

  const maybeAutoLoadMore = useCallback(() => {
    if (!hasMore || pendingAutoLoadRef.current) {
      return;
    }

    const listElement = listRef.current;
    if (!listElement) {
      return;
    }

    const distanceToBottom =
      listElement.scrollHeight - listElement.scrollTop - listElement.clientHeight;
    const shouldLoadMore =
      distanceToBottom <= 32 || listElement.scrollHeight <= listElement.clientHeight + 1;

    if (shouldLoadMore) {
      handleLoadMore();
    }
  }, [handleLoadMore, hasMore]);

  useEffect(() => {
    maybeAutoLoadMore();
  }, [maybeAutoLoadMore, visibleGroups.length]);

  const handleFileClick = useCallback((path: string, name: string) => {
    onFileSelect(path, name);
  }, [onFileSelect]);

  const handleLineClick = useCallback(async (path: string, name: string, lineNumber?: number) => {
    if (lineNumber) {
      const { editorJumpService } = await import('@/shared/services/EditorJumpService');
      await editorJumpService.jumpToFile(path, lineNumber, 1);
    } else {
      onFileSelect(path, name);
    }
  }, [onFileSelect]);

  if (results.length === 0) {
    return (
      <div className={`bitfun-search-results bitfun-search-results--empty ${className}`}>
        <div className="bitfun-search-results__empty">
          <div className="bitfun-search-results__empty-icon">
            <FileText size={48} />
          </div>
          <p>{t('search.noResults')}</p>
          <p className="bitfun-search-results__empty-hint">
            {t('search.noResultsHint')}
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className={`bitfun-search-results ${className}`}>
      <div className="bitfun-search-results__header">
        <span className="bitfun-search-results__count">
          {t('search.resultsSummary', { files: groupedResults.length, matches: results.length })}
          {hasMore && <span className="bitfun-search-results__showing">{t('search.resultsShowing', { count: displayCount })}</span>}
        </span>
      </div>

      <div
        ref={listRef}
        className="bitfun-search-results__list"
        onScroll={maybeAutoLoadMore}
      >
        {visibleGroups.map((group, index) => (
          <FileGroup
            key={`${group.path}-${index}`}
            group={group}
            isExpanded={isExpanded(group.path)}
            searchQuery={searchQuery}
            onToggleExpand={toggleExpanded}
            onFileClick={handleFileClick}
            onLineClick={handleLineClick}
          />
        ))}
      </div>
    </div>
  );
};

export default FileSearchResults;
