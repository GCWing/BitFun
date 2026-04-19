import React, { useMemo, useState, useCallback, startTransition, memo, useEffect, useRef } from 'react';
import { File, FileText, Folder, ChevronRight, ChevronDown } from 'lucide-react';
import type {
  FileSearchResult,
  FileSearchResultGroup,
} from '@/infrastructure/api/service-api/tauri-commands';
import { useI18n } from '@/infrastructure/i18n';
import { i18nService } from '@/infrastructure/i18n';
import { notificationService } from '@/shared/notification-system';
import { useContextMenuStore } from '@/shared/context-menu-system';
import { ContextType } from '@/shared/context-menu-system/types/context.types';
import type { MenuItem } from '@/shared/context-menu-system/types/menu.types';
import { addFileMentionToChat, type FileMentionTarget } from '@/shared/utils/chatContext';
import { openFileInBestTarget } from '@/shared/utils/tabUtils';
import './FileSearchResults.scss';

const INITIAL_DISPLAY_COUNT = 50;
const LOAD_MORE_COUNT = 50;

interface FileSearchResultsProps {
  results: FileSearchResultGroup[];
  searchQuery: string;
  onFileSelect: (filePath: string, fileName: string) => void;
  onFolderNavigate?: (folderPath: string, folderName: string) => void;
  workspacePath?: string;
  className?: string;
}

type SearchResultTarget = FileMentionTarget;

interface MatchPreviewSegments {
  before: string;
  inside: string;
  after: string;
}

function buildFallbackMatchPreview(
  line: string,
  query: string,
  maxLength: number = 96
): MatchPreviewSegments {
  if (!line) {
    return { before: '', inside: '', after: '' };
  }

  const normalizedLine = line.replace(/\t/g, '  ');
  const trimmedQuery = query.trim();

  if (!trimmedQuery) {
    const truncated =
      normalizedLine.length <= maxLength
        ? normalizedLine
        : `${normalizedLine.slice(0, maxLength - 1)}…`;
    return { before: '', inside: '', after: truncated };
  }

  const lowerLine = normalizedLine.toLowerCase();
  const lowerQuery = trimmedQuery.toLowerCase();
  const matchIndex = lowerLine.indexOf(lowerQuery);

  if (matchIndex === -1) {
    const truncated =
      normalizedLine.length <= maxLength
        ? normalizedLine
        : `${normalizedLine.slice(0, maxLength - 1)}…`;
    return { before: '', inside: '', after: truncated };
  }

  const contextWindow = Math.max(12, Math.floor((maxLength - trimmedQuery.length) / 2));
  let start = Math.max(0, matchIndex - contextWindow);
  let end = Math.min(
    normalizedLine.length,
    matchIndex + trimmedQuery.length + contextWindow
  );

  const visibleLength = end - start;
  if (visibleLength < maxLength) {
    const remaining = maxLength - visibleLength;
    const expandLeft = Math.min(start, Math.floor(remaining / 2));
    const expandRight = Math.min(
      normalizedLine.length - end,
      remaining - expandLeft
    );
    start -= expandLeft;
    end += expandRight;
  }

  const snippet = normalizedLine.slice(start, end);
  const relativeMatchStart = matchIndex - start;
  const relativeMatchEnd = relativeMatchStart + trimmedQuery.length;

  return {
    before: `${start > 0 ? '…' : ''}${snippet.slice(0, relativeMatchStart)}`,
    inside: snippet.slice(relativeMatchStart, relativeMatchEnd),
    after: `${snippet.slice(relativeMatchEnd)}${end < normalizedLine.length ? '…' : ''}`,
  };
}

function resolveMatchPreview(
  match: FileSearchResult,
  searchQuery: string
): MatchPreviewSegments {
  if (
    match.previewInside !== undefined
    || match.previewBefore !== undefined
    || match.previewAfter !== undefined
  ) {
    return {
      before: match.previewBefore ?? '',
      inside: match.previewInside ?? '',
      after: match.previewAfter ?? '',
    };
  }

  return buildFallbackMatchPreview(match.matchedContent || '', searchQuery);
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
  target: SearchResultTarget;
  searchQuery: string;
  onLineClick: (target: SearchResultTarget, lineNumber?: number) => void;
}

const MatchItem = memo<MatchItemProps>(({ match, target, searchQuery, onLineClick }) => {
  const preview = useMemo(
    () => resolveMatchPreview(match, searchQuery),
    [
      match,
      searchQuery,
    ]
  );

  return (
    <button
      type="button"
      className="bitfun-search-results__match"
      onClick={() => onLineClick(target, match.lineNumber)}
    >
      <span 
        className="bitfun-search-results__match-content"
        title={match.matchedContent || ''}
      >
        <code>
          {preview.before && (
            <span className="bitfun-search-results__match-before">{preview.before}</span>
          )}
          {preview.inside ? (
            <mark className="bitfun-search-results__highlight bitfun-search-results__match-highlight">
              {preview.inside}
            </mark>
          ) : null}
          {preview.after && (
            <span className="bitfun-search-results__match-after">{preview.after}</span>
          )}
        </code>
      </span>
    </button>
  );
});

MatchItem.displayName = 'MatchItem';

interface FileGroupProps {
  group: FileSearchResultGroup;
  isExpanded: boolean;
  searchQuery: string;
  onToggleExpand: (path: string) => void;
  onFileClick: (target: SearchResultTarget) => void;
  onFileContextMenu: (event: React.MouseEvent<HTMLElement>, target: SearchResultTarget) => void;
  onLineClick: (target: SearchResultTarget, lineNumber?: number) => void;
}

const FileGroup = memo<FileGroupProps>(({ 
  group, 
  isExpanded, 
  searchQuery, 
  onToggleExpand, 
  onFileClick, 
  onFileContextMenu,
  onLineClick 
}) => {
  const { t } = useI18n('tools');
  const hasContentMatches = group.contentMatches.length > 0;
  const target = useMemo<SearchResultTarget>(() => ({
    path: group.path,
    name: group.name,
    isDirectory: group.isDirectory,
  }), [group.isDirectory, group.name, group.path]);

  return (
    <div className="bitfun-search-results__group">
      <div className="bitfun-search-results__file">
        <button
          type="button"
          className="bitfun-search-results__file-main"
          onClick={() => onFileClick(target)}
          onContextMenu={(event) => onFileContextMenu(event, target)}
        >
          <span
            className={`bitfun-search-results__file-icon${
              group.isDirectory ? ' bitfun-search-results__file-icon--directory' : ''
            }`}
          >
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
        </button>

        {hasContentMatches && (
          <button
            type="button"
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
          </button>
        )}
      </div>

      {hasContentMatches && isExpanded && (
        <div className="bitfun-search-results__matches">
          {group.contentMatches.map((match, matchIndex) => (
            <MatchItem
              key={`${group.path}-${matchIndex}`}
              match={match}
              target={target}
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
  onFolderNavigate,
  workspacePath,
  className = ''
}) => {
  const { t } = useI18n('tools');
  const [displayCount, setDisplayCount] = useState(INITIAL_DISPLAY_COUNT);
  const listRef = useRef<HTMLDivElement>(null);
  const pendingAutoLoadRef = useRef(false);
  const [manualExpandState, setManualExpandState] = useState<Map<string, boolean>>(new Map());
  const showMenu = useContextMenuStore((state) => state.showMenu);

  const prevResultsRef = useRef(results);
  const prevSearchQueryRef = useRef(searchQuery);
  
  useEffect(() => {
    const queryChanged = prevSearchQueryRef.current !== searchQuery;
    const resultsRestarted = results.length < prevResultsRef.current.length;

    if (queryChanged || resultsRestarted) {
      prevResultsRef.current = results;
      prevSearchQueryRef.current = searchQuery;
      startTransition(() => {
        setDisplayCount(INITIAL_DISPLAY_COUNT);
        setManualExpandState(new Map());
      });
      return;
    }

    prevResultsRef.current = results;
    prevSearchQueryRef.current = searchQuery;
  }, [results, searchQuery]);

  const visibleGroups = useMemo(() => {
    return results.slice(0, displayCount);
  }, [results, displayCount]);

  const hasMore = displayCount < results.length;
  const totalMatches = useMemo(() => {
    return results.reduce((count, group) => {
      return count + (group.fileNameMatch ? 1 : 0) + group.contentMatches.length;
    }, 0);
  }, [results]);

  const shouldDefaultExpand = results.length <= 100;
  
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
      setDisplayCount(prev => Math.min(prev + LOAD_MORE_COUNT, results.length));
    });
  }, [results.length]);

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

  const openResultTarget = useCallback((target: SearchResultTarget, lineNumber?: number) => {
    if (target.isDirectory) {
      return;
    }

    openFileInBestTarget({
      filePath: target.path,
      fileName: target.name,
      workspacePath,
      ...(lineNumber ? { jumpToLine: lineNumber, jumpToColumn: 1 } : {}),
    }, { source: 'project-nav' });
  }, [workspacePath]);

  const handleFileClick = useCallback((target: SearchResultTarget) => {
    if (target.isDirectory) {
      onFolderNavigate?.(target.path, target.name);
      return;
    }
    onFileSelect(target.path, target.name);
    openResultTarget(target);
  }, [onFileSelect, openResultTarget, onFolderNavigate]);

  const handleLineClick = useCallback((target: SearchResultTarget, lineNumber?: number) => {
    onFileSelect(target.path, target.name);

    if (lineNumber) {
      openResultTarget(target, lineNumber);
    } else {
      openResultTarget(target);
    }
  }, [onFileSelect, openResultTarget]);

  const handleCopyPath = useCallback(async (path: string) => {
    try {
      await navigator.clipboard.writeText(path);
      notificationService.success(i18nService.t('common:contextMenu.status.copyPathSuccess'), {
        duration: 2000,
      });
    } catch (error) {
      notificationService.error(
        error instanceof Error
          ? error.message
          : i18nService.t('errors:contextMenu.copyPathFailed'),
      );
    }
  }, []);

  const handleAddToChat = useCallback((target: SearchResultTarget) => {
    addFileMentionToChat(target, workspacePath);
  }, [workspacePath]);

  const handleFileContextMenu = useCallback((
    event: React.MouseEvent<HTMLElement>,
    target: SearchResultTarget,
  ) => {
    event.preventDefault();
    event.stopPropagation();

    const items: MenuItem[] = [
      {
        id: `search-result-copy-path:${target.path}`,
        label: i18nService.t('common:file.copyPath'),
        icon: 'Copy',
        onClick: async () => {
          await handleCopyPath(target.path);
        },
      },
      {
        id: `search-result-add-to-chat:${target.path}`,
        label: i18nService.t('common:editor.addToChat'),
        icon: 'MessageSquarePlus',
        onClick: () => {
          handleAddToChat(target);
        },
      },
    ];

    showMenu(
      { x: event.clientX, y: event.clientY },
      items,
      {
        type: ContextType.CUSTOM,
        customType: 'file-search-result',
        data: target,
        event,
        targetElement: event.currentTarget,
        position: { x: event.clientX, y: event.clientY },
        timestamp: Date.now(),
      },
    );
  }, [handleAddToChat, handleCopyPath, showMenu]);

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
          {t('search.resultsSummary', { files: results.length, matches: totalMatches })}
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
            onFileContextMenu={handleFileContextMenu}
            onLineClick={handleLineClick}
          />
        ))}
      </div>
    </div>
  );
};

export default FileSearchResults;
