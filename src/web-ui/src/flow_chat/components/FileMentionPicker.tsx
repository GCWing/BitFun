/**
 * File and session mention picker.
 * Shown when the user types @ to select files, folders, or idle sessions.
 */

import React, { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { createPortal } from 'react-dom';
import {
  IconButton,
  KeyHint,
  Listbox,
  ListboxEmpty,
  ListboxOption,
  Tooltip,
} from '@bitfun/ui';
import { useTranslation } from 'react-i18next';
import {
  File,
  Folder,
  Loader2,
  MessageCircle,
  Search,
  ChevronRight,
  ChevronLeft,
} from 'lucide-react';
import { sessionAPI, workspaceAPI } from '@/infrastructure/api';
import {
  externalSourcesAPI,
  type WorkspaceReferenceEntry,
} from '@/infrastructure/api/service-api/ExternalSourcesAPI';
import type {
  ExplorerNodeDto,
  FileSearchResultGroup,
} from '@/infrastructure/api/service-api/tauri-commands';
import type { SessionReferenceCandidate } from '@/infrastructure/api/service-api/SessionAPI';
import type {
  DirectoryContext,
  FileContext,
  SessionReferenceContext,
} from '@/shared/types/context';
import { createLogger } from '@/shared/utils/logger';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { useAnchoredPopoverPosition } from '@/shared/utils/useAnchoredPopoverPosition';
import {
  workspaceReferenceItems,
  type FileItem,
} from './workspaceReferenceItems';
import './FileMentionPicker.scss';

const log = createLogger('FileMentionPicker');
const FILE_MENTION_SEARCH_DEBOUNCE_MS = 300;
const FILE_MENTION_MAX_RESULTS = 30;

export interface FileMentionPickerProps {
  isOpen: boolean;
  searchQuery: string;
  workspacePath?: string;
  workspaceId?: string;
  /** Disambiguates identical POSIX workspace paths on different remote hosts. */
  remoteConnectionId?: string;
  /** The composing session itself must not appear as a reference candidate. */
  excludeSessionId?: string;
  onSelect: (context: FileContext | DirectoryContext | SessionReferenceContext) => void;
  onClose: () => void;
  /** Anchor used by the default portalled overlay mode. */
  anchorRef?: React.RefObject<HTMLElement | null>;
  position?: { top: number; left: number };
  onNavigate?: (direction: 'up' | 'down' | 'enter' | 'escape') => void;
}

type MentionItem =
  | { kind: 'file'; item: FileItem }
  | { kind: 'session'; item: SessionReferenceCandidate };

function mergeFileSearchResults(
  previous: FileItem[],
  groups: FileSearchResultGroup[],
  getRelativePath: (path: string) => string,
): FileItem[] {
  const byPath = new Map(previous.map(item => [item.path, item]));
  for (const group of groups) {
    const result = group.fileNameMatch;
    if (!result) continue;
    byPath.set(result.path, {
      path: result.path,
      name: result.name,
      isDirectory: result.isDirectory || false,
      relativePath: getRelativePath(result.path),
    });
  }

  return Array.from(byPath.values())
    .sort((a, b) => {
      if (a.isDirectory !== b.isDirectory) return a.isDirectory ? -1 : 1;
      return a.name.localeCompare(b.name);
    })
    .slice(0, FILE_MENTION_MAX_RESULTS);
}

export const FileMentionPicker: React.FC<FileMentionPickerProps> = ({
  isOpen,
  searchQuery,
  workspacePath,
  workspaceId,
  remoteConnectionId,
  excludeSessionId,
  onSelect,
  onClose,
  anchorRef,
  position,
}) => {
  const { t } = useTranslation('flow-chat');
  const [results, setResults] = useState<FileItem[]>([]);
  const [sessionResults, setSessionResults] = useState<SessionReferenceCandidate[]>([]);
  const [workspaceReferences, setWorkspaceReferences] = useState<WorkspaceReferenceEntry[]>([]);
  const [currentFiles, setCurrentFiles] = useState<FileItem[]>([]);
  const [isDirectoryLoading, setIsDirectoryLoading] = useState(false);
  const [isFileSearchLoading, setIsFileSearchLoading] = useState(false);
  const [isSessionLoading, setIsSessionLoading] = useState(false);
  const [directoryLoadError, setDirectoryLoadError] = useState(false);
  const [fileSearchError, setFileSearchError] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [currentPath, setCurrentPath] = useState<string>('');
  const [pathHistory, setPathHistory] = useState<string[]>([]);
  const containerRef = useRef<HTMLDivElement>(null);
  const fallbackAnchorRef = useRef<HTMLElement>(null);
  const fileAbortControllerRef = useRef<AbortController | null>(null);
  const fileSearchDebounceTimerRef = useRef<number | null>(null);
  const sessionSearchDebounceTimerRef = useRef<number | null>(null);
  const selectedItemHistoryRef = useRef<string[]>([]);
  const targetSelectedPathRef = useRef<string | null>(null);
  const directoryLoadRequestIdRef = useRef(0);
  const fileSearchRequestIdRef = useRef(0);
  const sessionSearchRequestIdRef = useRef(0);
  const workspaceReferenceRequestIdRef = useRef(0);
  const skipNextPathLoadRef = useRef(false);

  const getRelativePath = useCallback((fullPath: string): string => {
    if (!workspacePath) return fullPath;
    const normalizedWorkspace = workspacePath.replace(/\\/g, '/');
    const normalizedPath = fullPath.replace(/\\/g, '/');
    if (normalizedPath.startsWith(normalizedWorkspace)) {
      return normalizedPath.slice(normalizedWorkspace.length).replace(/^\//, '');
    }
    return fullPath;
  }, [workspacePath]);

  const loadDirectory = useCallback(async (dirPath: string, targetSelectedPath?: string | null) => {
    if (!workspacePath) {
      setCurrentFiles([]);
      setIsDirectoryLoading(false);
      setDirectoryLoadError(false);
      return;
    }

    const requestId = ++directoryLoadRequestIdRef.current;
    setIsDirectoryLoading(true);
    setDirectoryLoadError(false);
    try {
      const children = await workspaceAPI.getDirectoryChildren(
        dirPath || workspacePath,
        remoteConnectionId,
      );
      const items: FileItem[] = children
        .filter((entry: ExplorerNodeDto) => {
          const name = entry.name || '';
          return !name.startsWith('.') &&
            !['node_modules', 'target', 'dist', 'build', '__pycache__'].includes(name);
        })
        .map((entry: ExplorerNodeDto) => ({
          path: entry.path,
          name: entry.name,
          isDirectory: entry.isDirectory || false,
          relativePath: getRelativePath(entry.path),
        }));
      items.sort((a, b) => {
        if (a.isDirectory !== b.isDirectory) return a.isDirectory ? -1 : 1;
        return a.name.localeCompare(b.name);
      });
      if (requestId !== directoryLoadRequestIdRef.current) return;
      setCurrentFiles(items);
      const targetIndex = targetSelectedPath
        ? items.findIndex(item => item.path === targetSelectedPath)
        : 0;
      setSelectedIndex(targetIndex >= 0 ? targetIndex : 0);
    } catch (error) {
      log.error('Failed to load directory', error);
      if (requestId === directoryLoadRequestIdRef.current) {
        setCurrentFiles([]);
        setDirectoryLoadError(true);
      }
    } finally {
      if (requestId === directoryLoadRequestIdRef.current) setIsDirectoryLoading(false);
    }
  }, [workspacePath, remoteConnectionId, getRelativePath]);

  const enterDirectory = useCallback((item: FileItem) => {
    if (!item.isDirectory) return;
    selectedItemHistoryRef.current = [...selectedItemHistoryRef.current, item.path];
    setPathHistory(previous => [...previous, currentPath]);
    setCurrentPath(item.path);
  }, [currentPath]);

  const goBack = useCallback(() => {
    if (pathHistory.length === 0) return;
    const previousPath = pathHistory[pathHistory.length - 1];
    targetSelectedPathRef.current = selectedItemHistoryRef.current.length > 0
      ? selectedItemHistoryRef.current[selectedItemHistoryRef.current.length - 1]
      : null;
    selectedItemHistoryRef.current = selectedItemHistoryRef.current.slice(0, -1);
    setPathHistory(previous => previous.slice(0, -1));
    setCurrentPath(previousPath);
  }, [pathHistory]);

  useEffect(() => {
    if (!isOpen || !workspacePath) return;
    skipNextPathLoadRef.current = true;
    setCurrentPath('');
    setPathHistory([]);
    setCurrentFiles([]);
    setResults([]);
    setSessionResults([]);
    setDirectoryLoadError(false);
    setFileSearchError(false);
    setSelectedIndex(0);
    selectedItemHistoryRef.current = [];
    targetSelectedPathRef.current = null;
    loadDirectory('', null);
  }, [isOpen, workspacePath, remoteConnectionId, loadDirectory]);

  useEffect(() => {
    if (!isOpen || !workspacePath) {
      workspaceReferenceRequestIdRef.current += 1;
      setWorkspaceReferences([]);
      return;
    }
    const requestId = ++workspaceReferenceRequestIdRef.current;
    void externalSourcesAPI
      .getWorkspaceReferences(workspacePath, workspaceId)
      .then(snapshot => {
        if (requestId === workspaceReferenceRequestIdRef.current) {
          setWorkspaceReferences(snapshot.references);
        }
      })
      .catch(() => {
        if (requestId === workspaceReferenceRequestIdRef.current) {
          setWorkspaceReferences([]);
        }
      });
  }, [isOpen, workspaceId, workspacePath]);

  useEffect(() => {
    if (!isOpen || searchQuery.trim()) return;
    if (skipNextPathLoadRef.current) {
      skipNextPathLoadRef.current = false;
      return;
    }
    const targetPath = targetSelectedPathRef.current;
    targetSelectedPathRef.current = null;
    loadDirectory(currentPath, targetPath);
  }, [currentPath, isOpen, loadDirectory, searchQuery]);

  const searchFiles = useCallback(async (
    query: string,
    controller: AbortController,
    requestId: number,
  ) => {
    if (!workspacePath) {
      setResults([]);
      setIsFileSearchLoading(false);
      return;
    }
    try {
      await workspaceAPI.searchFilenamesOnlyStreamDetailed(
        workspacePath,
        query,
        false,
        false,
        false,
        undefined,
        FILE_MENTION_MAX_RESULTS,
        true,
        {
          onProgress: (event) => {
            if (
              requestId !== fileSearchRequestIdRef.current
              || controller.signal.aborted
            ) return;

            setResults(previous => mergeFileSearchResults(previous, event.results, getRelativePath));
          },
        },
        controller.signal,
        remoteConnectionId,
      );
    } catch (error) {
      if (!(error instanceof DOMException && error.name === 'AbortError')) {
        log.error('File mention search failed', error);
        if (requestId === fileSearchRequestIdRef.current) setFileSearchError(true);
      }
    } finally {
      if (requestId === fileSearchRequestIdRef.current && fileAbortControllerRef.current === controller) {
        fileAbortControllerRef.current = null;
        setIsFileSearchLoading(false);
      }
    }
  }, [workspacePath, remoteConnectionId, getRelativePath]);

  useEffect(() => {
    if (!isOpen) return;
    if (fileSearchDebounceTimerRef.current !== null) {
      window.clearTimeout(fileSearchDebounceTimerRef.current);
      fileSearchDebounceTimerRef.current = null;
    }
    fileAbortControllerRef.current?.abort();
    fileAbortControllerRef.current = null;

    const query = searchQuery.trim();
    if (!query) {
      fileSearchRequestIdRef.current += 1;
      setResults([]);
      setSelectedIndex(0);
      setIsFileSearchLoading(false);
      setFileSearchError(false);
      return;
    }

    const requestId = ++fileSearchRequestIdRef.current;
    const controller = new AbortController();
    fileAbortControllerRef.current = controller;
    setResults([]);
    setSelectedIndex(0);
    setIsFileSearchLoading(true);
    setFileSearchError(false);
    fileSearchDebounceTimerRef.current = window.setTimeout(() => {
      fileSearchDebounceTimerRef.current = null;
      void searchFiles(query, controller, requestId);
    }, FILE_MENTION_SEARCH_DEBOUNCE_MS);
    return () => {
      if (fileSearchDebounceTimerRef.current !== null) {
        window.clearTimeout(fileSearchDebounceTimerRef.current);
        fileSearchDebounceTimerRef.current = null;
      }
      controller.abort();
    };
  }, [isOpen, searchQuery, searchFiles]);

  useEffect(() => {
    if (sessionSearchDebounceTimerRef.current !== null) {
      window.clearTimeout(sessionSearchDebounceTimerRef.current);
      sessionSearchDebounceTimerRef.current = null;
    }
    const query = searchQuery.trim();
    if (!isOpen || !query) {
      sessionSearchRequestIdRef.current += 1;
      setSessionResults([]);
      setIsSessionLoading(false);
      return;
    }

    const requestId = ++sessionSearchRequestIdRef.current;
    setIsSessionLoading(true);
    sessionSearchDebounceTimerRef.current = window.setTimeout(() => {
      sessionSearchDebounceTimerRef.current = null;
      void sessionAPI.searchReferenceableSessions(query, FILE_MENTION_MAX_RESULTS)
        .then((items) => {
          if (requestId === sessionSearchRequestIdRef.current) {
            setSessionResults(items.filter(item => item.sessionId !== excludeSessionId));
          }
        })
        .catch((error) => {
          log.error('Session mention search failed', error);
          if (requestId === sessionSearchRequestIdRef.current) setSessionResults([]);
        })
        .finally(() => {
          if (requestId === sessionSearchRequestIdRef.current) setIsSessionLoading(false);
        });
    }, FILE_MENTION_SEARCH_DEBOUNCE_MS);
    return () => {
      if (sessionSearchDebounceTimerRef.current !== null) {
        window.clearTimeout(sessionSearchDebounceTimerRef.current);
        sessionSearchDebounceTimerRef.current = null;
      }
    };
  }, [excludeSessionId, isOpen, searchQuery]);

  const isSearchMode = searchQuery.trim().length > 0;
  const isFileLoading = isSearchMode ? isFileSearchLoading : isDirectoryLoading;
  const fileLoadError = isSearchMode
    ? (fileSearchError ? 'search' : null)
    : (directoryLoadError ? 'directory' : null);
  const referenceItems = useMemo(
    () => workspaceReferenceItems(workspaceReferences, isSearchMode ? searchQuery : ''),
    [isSearchMode, searchQuery, workspaceReferences],
  );
  const displayItems = useMemo<MentionItem[]>(() => (
    isSearchMode
      ? [
          ...results.map(item => ({ kind: 'file' as const, item })),
          ...referenceItems.map(item => ({ kind: 'file' as const, item })),
          ...sessionResults.map(item => ({ kind: 'session' as const, item })),
        ]
      : [
          ...currentFiles.map(item => ({ kind: 'file' as const, item })),
          ...(currentPath ? [] : referenceItems.map(item => ({ kind: 'file' as const, item }))),
        ]
  ), [currentFiles, currentPath, isSearchMode, referenceItems, results, sessionResults]);
  const currentDirectoryDisplay = useMemo(() => {
    if (!workspacePath) {
      const rootDirectory = t('fileMention.rootDirectory');
      return { name: rootDirectory, parentPath: '', fullPath: rootDirectory };
    }

    const normalizedWorkspace = workspacePath.replace(/\\/g, '/').replace(/\/+$/, '');
    const workspaceName = normalizedWorkspace.split('/').pop() || t('fileMention.rootDirectory');
    const directorySegments = [workspaceName];

    if (currentPath) {
      const relativeCurrentPath = getRelativePath(currentPath)
        .replace(/\\/g, '/')
        .replace(/^\/+|\/+$/g, '');
      if (relativeCurrentPath) directorySegments.push(...relativeCurrentPath.split('/').filter(Boolean));
    }

    return {
      name: directorySegments[directorySegments.length - 1],
      parentPath: directorySegments.slice(0, -1).join('/'),
      fullPath: directorySegments.join('/'),
    };
  }, [currentPath, getRelativePath, t, workspacePath]);
  const isOverlay = Boolean(anchorRef) && !position;
  const overlayLayout = useAnchoredPopoverPosition({
    open: isOpen && isOverlay,
    anchorRef: anchorRef ?? fallbackAnchorRef,
    popoverRef: containerRef,
    preferredPlacement: 'top',
    alignment: 'start',
    gap: 8,
    layoutRevision: `${displayItems.length}:${currentPath}:${isSearchMode}`,
  });

  useEffect(() => () => {
    if (fileSearchDebounceTimerRef.current !== null) window.clearTimeout(fileSearchDebounceTimerRef.current);
    if (sessionSearchDebounceTimerRef.current !== null) window.clearTimeout(sessionSearchDebounceTimerRef.current);
    fileAbortControllerRef.current?.abort();
  }, []);

  const handleSelect = useCallback((mention: MentionItem) => {
    const timestamp = Date.now();
    if (mention.kind === 'session') {
      const session = mention.item;
      onSelect({
        id: `session-reference-${timestamp}-${Math.random().toString(36).slice(2, 9)}`,
        type: 'session-reference',
        sessionId: session.sessionId,
        sessionName: session.sessionName,
        workspacePath: session.workspacePath,
        remoteConnectionId: session.remoteConnectionId,
        remoteSshHost: session.remoteSshHost,
        workspaceLabel: session.workspaceLabel,
        timestamp,
      });
      onClose();
      return;
    }

    const item = mention.item;
    onSelect(item.isDirectory ? {
      id: `dir-${timestamp}-${Math.random().toString(36).slice(2, 9)}`,
      type: 'directory',
      directoryPath: item.path,
      directoryName: item.name,
      recursive: true,
      timestamp,
    } : {
      id: `file-${timestamp}-${Math.random().toString(36).slice(2, 9)}`,
      type: 'file',
      filePath: item.path,
      fileName: item.name,
      relativePath: item.relativePath,
      timestamp,
    });
    onClose();
  }, [onClose, onSelect]);

  const handleItemClick = useCallback((mention: MentionItem) => {
    if (mention.kind === 'file' && mention.item.isDirectory && !isSearchMode) {
      enterDirectory(mention.item);
      return;
    }
    handleSelect(mention);
  }, [enterDirectory, handleSelect, isSearchMode]);

  const handleKeyDown = useCallback((event: KeyboardEvent) => {
    if (!isOpen) return;
    switch (event.key) {
      case 'ArrowUp':
      case 'ArrowDown': {
        event.preventDefault();
        event.stopPropagation();
        if (displayItems.length > 0) {
          setSelectedIndex(previous => event.key === 'ArrowUp'
            ? (previous > 0 ? previous - 1 : displayItems.length - 1)
            : (previous < displayItems.length - 1 ? previous + 1 : 0));
        }
        break;
      }
      case 'ArrowRight': {
        event.preventDefault();
        event.stopPropagation();
        const selected = displayItems[selectedIndex];
        if (!isSearchMode && selected?.kind === 'file' && selected.item.isDirectory) {
          enterDirectory(selected.item);
        }
        break;
      }
      case 'ArrowLeft':
        event.preventDefault();
        event.stopPropagation();
        if (!isSearchMode && pathHistory.length > 0) goBack();
        break;
      case 'Enter':
      case 'Tab': {
        event.preventDefault();
        event.stopPropagation();
        const selected = displayItems[selectedIndex];
        if (selected) {
          if (event.key === 'Tab') handleSelect(selected);
          else handleItemClick(selected);
        }
        break;
      }
      case 'Escape':
        event.preventDefault();
        event.stopPropagation();
        onClose();
        break;
    }
  }, [displayItems, enterDirectory, goBack, handleItemClick, handleSelect, isOpen, isSearchMode, onClose, pathHistory.length, selectedIndex]);

  useEffect(() => {
    if (!isOpen) return;
    document.addEventListener('keydown', handleKeyDown, true);
    return () => document.removeEventListener('keydown', handleKeyDown, true);
  }, [handleKeyDown, isOpen]);

  useEffect(() => {
    if (!isOpen) return;
    const handleClickOutside = (event: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) onClose();
    };
    document.addEventListener('mousedown', handleClickOutside, true);
    return () => document.removeEventListener('mousedown', handleClickOutside, true);
  }, [isOpen, onClose]);

  useEffect(() => {
    if (!containerRef.current || displayItems.length === 0) return;
    containerRef.current.querySelector(`[data-index="${selectedIndex}"]`)
      ?.scrollIntoView({ block: 'nearest', behavior: 'auto' });
  }, [displayItems.length, selectedIndex]);

  if (!isOpen) return null;
  const style: React.CSSProperties = position
    ? { position: 'absolute', top: position.top, left: position.left }
    : isOverlay
      ? {
          top: `${overlayLayout?.top ?? 0}px`,
          left: `${overlayLayout?.left ?? 0}px`,
          visibility: overlayLayout ? 'visible' : 'hidden',
        }
      : {};
  const isLoading = isFileLoading || isSessionLoading;

  const picker = (
    <div
      data-bf-component="file-mention-picker"
      data-bf-part="root"
      data-bf-state={[
        isLoading && 'loading',
        fileLoadError && 'error',
      ].filter(Boolean).join(' ') || undefined}
      data-bf-placement={isOverlay ? overlayLayout?.placement ?? 'top' : undefined}
      ref={containerRef}
      className={`file-mention-picker${isOverlay ? ' file-mention-picker--overlay' : ''}`}
      style={style}
      onMouseDown={event => event.preventDefault()}
    >
      <div data-bf-component="file-mention-picker" data-bf-part="header" className="file-mention-picker__header">
        {!isSearchMode && pathHistory.length > 0 && (
          <Tooltip content={t('fileMention.goBack')}>
            <IconButton
              aria-label={t('fileMention.goBack')}
              icon={<ChevronLeft aria-hidden="true" />}
              onClick={goBack}
              size="xs"
              variant="quiet"
            />
          </Tooltip>
        )}
        {isSearchMode ? <><Search size={11} /><span>{t('fileMention.searchResults')}</span></> : (
          <div className="file-mention-picker__directory-label" title={currentDirectoryDisplay.fullPath}>
            <span
              data-bf-component="file-mention-picker"
              data-bf-part="currentDirectoryName"
              className="file-mention-picker__dir-name"
            >
              {currentDirectoryDisplay.name}
            </span>
            {currentDirectoryDisplay.parentPath && (
              <span
                data-bf-component="file-mention-picker"
                data-bf-part="parentDirectoryPath"
                className="file-mention-picker__parent-path"
              >
                {currentDirectoryDisplay.parentPath}
              </span>
            )}
          </div>
        )}
      </div>
      <div data-bf-component="file-mention-picker" data-bf-part="content" className="file-mention-picker__content">
        <Listbox
          aria-label={isSearchMode
            ? t('fileMention.searchResults')
            : currentDirectoryDisplay.fullPath}
          className="file-mention-picker__list"
          focusMode="virtual"
        >
          {displayItems.length === 0 && fileLoadError ? (
          <ListboxEmpty data-bf-state="error" className="file-mention-picker__empty">
            <span>{t(fileLoadError === 'search'
              ? 'fileMention.searchUnavailable'
              : 'fileMention.browseUnavailable')}</span>
          </ListboxEmpty>
        ) : displayItems.length === 0 && isLoading ? (
          <ListboxEmpty className="file-mention-picker__loading"><Loader2 size={14} className="file-mention-picker__spinner" /><span>{t('fileMention.loading')}</span></ListboxEmpty>
        ) : displayItems.length === 0 ? (
          <ListboxEmpty className="file-mention-picker__empty"><span>{isSearchMode ? t('fileMention.noMatchingFiles') : t('fileMention.emptyDirectory')}</span></ListboxEmpty>
        ) : (
          <>
            {displayItems.map((mention, index) => {
              const isSession = mention.kind === 'session';
              const file = mention.kind === 'file' ? mention.item : null;
              const session = mention.kind === 'session' ? mention.item : null;
              const key = isSession
                ? `session-${session?.sessionId}-${session?.workspacePath}`
                : `file-${file?.referenceStableKey || file?.path}`;
              return (
                <ListboxOption
                  active={index === selectedIndex}
                  key={key}
                  data-index={index}
                  indicator={file?.isDirectory && !isSearchMode
                    ? <ChevronRight aria-hidden="true" />
                    : undefined}
                  leading={isSession
                    ? <MessageCircle aria-hidden="true" />
                    : file?.isDirectory
                      ? <Folder aria-hidden="true" />
                      : <File aria-hidden="true" />}
                  metadata={session?.workspaceLabel
                    ?? (file?.referenceStableKey
                      ? file.referenceDescription || file.path
                      : undefined)}
                  onClick={() => handleItemClick(mention)}
                  onContextMenu={(event) => {
                    event.preventDefault();
                    if (file?.isDirectory) enterDirectory(file);
                  }}
                  value={key}
                >
                  {session?.sessionName ?? file?.name}
                </ListboxOption>
              );
            })}
            {isLoading && (
              <ListboxEmpty className="file-mention-picker__loading">
                <Loader2 size={14} className="file-mention-picker__spinner" />
                <span>{t('fileMention.loading')}</span>
              </ListboxEmpty>
            )}
            {fileLoadError && (
              <ListboxEmpty data-bf-state="error" className="file-mention-picker__empty">
                <span>{t(fileLoadError === 'search'
                  ? 'fileMention.searchUnavailable'
                  : 'fileMention.browseUnavailable')}</span>
              </ListboxEmpty>
            )}
          </>
        )}
        </Listbox>
      </div>
      <div data-bf-component="file-mention-picker" data-bf-part="footer" className="file-mention-picker__footer">
        <span><KeyHint>↑</KeyHint><KeyHint>↓</KeyHint> {t('fileMention.navHint')}</span>
        <span><KeyHint>→</KeyHint> {t('fileMention.enterHint')}</span>
        <span><KeyHint>←</KeyHint> {t('fileMention.backHint')}</span>
        <span><KeyHint>Enter</KeyHint> {t('fileMention.selectHint')}</span>
      </div>
    </div>
  );

  return isOverlay ? createPortal(picker, getAppearanceOverlayHost()) : picker;
};

export default FileMentionPicker;
