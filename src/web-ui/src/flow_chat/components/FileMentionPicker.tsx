/**
 * File and session mention picker.
 * Shown when the user types @ to select files, folders, or idle sessions.
 */

import React, { useState, useEffect, useCallback, useMemo, useRef } from 'react';
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
import type {
  ExplorerNodeDto,
  FileSearchResult,
} from '@/infrastructure/api/service-api/tauri-commands';
import type { SessionReferenceCandidate } from '@/infrastructure/api/service-api/SessionAPI';
import type {
  DirectoryContext,
  FileContext,
  SessionReferenceContext,
} from '@/shared/types/context';
import { Tooltip } from '@/component-library';
import { createLogger } from '@/shared/utils/logger';
import './FileMentionPicker.scss';

const log = createLogger('FileMentionPicker');
const FILE_MENTION_SEARCH_DEBOUNCE_MS = 300;
const FILE_MENTION_MAX_RESULTS = 30;

export interface FileMentionPickerProps {
  isOpen: boolean;
  searchQuery: string;
  workspacePath?: string;
  /** The composing session itself must not appear as a reference candidate. */
  excludeSessionId?: string;
  onSelect: (context: FileContext | DirectoryContext | SessionReferenceContext) => void;
  onClose: () => void;
  position?: { top: number; left: number };
  onNavigate?: (direction: 'up' | 'down' | 'enter' | 'escape') => void;
}

interface FileItem {
  path: string;
  name: string;
  isDirectory: boolean;
  relativePath: string;
}

type MentionItem =
  | { kind: 'file'; item: FileItem }
  | { kind: 'session'; item: SessionReferenceCandidate };

export const FileMentionPicker: React.FC<FileMentionPickerProps> = ({
  isOpen,
  searchQuery,
  workspacePath,
  excludeSessionId,
  onSelect,
  onClose,
  position,
}) => {
  const { t } = useTranslation('flow-chat');
  const [results, setResults] = useState<FileItem[]>([]);
  const [sessionResults, setSessionResults] = useState<SessionReferenceCandidate[]>([]);
  const [currentFiles, setCurrentFiles] = useState<FileItem[]>([]);
  const [isFileLoading, setIsFileLoading] = useState(false);
  const [isSessionLoading, setIsSessionLoading] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [currentPath, setCurrentPath] = useState<string>('');
  const [pathHistory, setPathHistory] = useState<string[]>([]);
  const containerRef = useRef<HTMLDivElement>(null);
  const fileAbortControllerRef = useRef<AbortController | null>(null);
  const fileSearchDebounceTimerRef = useRef<number | null>(null);
  const sessionSearchDebounceTimerRef = useRef<number | null>(null);
  const selectedItemHistoryRef = useRef<string[]>([]);
  const targetSelectedPathRef = useRef<string | null>(null);
  const directoryLoadRequestIdRef = useRef(0);
  const fileSearchRequestIdRef = useRef(0);
  const sessionSearchRequestIdRef = useRef(0);
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
      return;
    }

    const requestId = ++directoryLoadRequestIdRef.current;
    setIsFileLoading(true);
    try {
      const children = await workspaceAPI.getDirectoryChildren(dirPath || workspacePath);
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
      if (requestId === directoryLoadRequestIdRef.current) setCurrentFiles([]);
    } finally {
      if (requestId === directoryLoadRequestIdRef.current) setIsFileLoading(false);
    }
  }, [workspacePath, getRelativePath]);

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
    setSelectedIndex(0);
    selectedItemHistoryRef.current = [];
    targetSelectedPathRef.current = null;
    loadDirectory('', null);
  }, [isOpen, workspacePath, loadDirectory]);

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
      return;
    }
    try {
      const searchResults = await workspaceAPI.searchFilenamesOnly(
        workspacePath, query, false, false, false, controller.signal,
      );
      if (requestId !== fileSearchRequestIdRef.current || controller.signal.aborted) return;
      const items = searchResults.map((result: FileSearchResult) => ({
        path: result.path,
        name: result.name,
        isDirectory: result.isDirectory || false,
        relativePath: getRelativePath(result.path),
      }));
      items.sort((a, b) => {
        if (a.isDirectory !== b.isDirectory) return a.isDirectory ? -1 : 1;
        return a.name.localeCompare(b.name);
      });
      setResults(items.slice(0, FILE_MENTION_MAX_RESULTS));
      setSelectedIndex(0);
    } catch (error) {
      if (!(error instanceof DOMException && error.name === 'AbortError')) {
        log.error('File mention search failed', error);
      }
      if (requestId === fileSearchRequestIdRef.current) setResults([]);
    } finally {
      if (requestId === fileSearchRequestIdRef.current && fileAbortControllerRef.current === controller) {
        fileAbortControllerRef.current = null;
        setIsFileLoading(false);
      }
    }
  }, [workspacePath, getRelativePath]);

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
      setIsFileLoading(false);
      return;
    }

    const requestId = ++fileSearchRequestIdRef.current;
    const controller = new AbortController();
    fileAbortControllerRef.current = controller;
    setIsFileLoading(true);
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
  const displayItems = useMemo<MentionItem[]>(() => (
    isSearchMode
      ? [
          ...results.map(item => ({ kind: 'file' as const, item })),
          ...sessionResults.map(item => ({ kind: 'session' as const, item })),
        ]
      : currentFiles.map(item => ({ kind: 'file' as const, item }))
  ), [currentFiles, isSearchMode, results, sessionResults]);
  const currentDirName = currentPath
    ? currentPath.replace(/\\/g, '/').split('/').pop() || ''
    : workspacePath?.replace(/\\/g, '/').split('/').pop() || t('fileMention.rootDirectory');

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
      ?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
  }, [displayItems.length, selectedIndex]);

  if (!isOpen) return null;
  const style: React.CSSProperties = position ? { position: 'absolute', top: position.top, left: position.left } : {};
  const isLoading = isFileLoading || isSessionLoading;

  return (
    <div ref={containerRef} className="file-mention-picker" style={style} onMouseDown={event => event.preventDefault()}>
      <div className="file-mention-picker__header">
        {!isSearchMode && pathHistory.length > 0 && (
          <Tooltip content={t('fileMention.goBack')}>
            <button className="file-mention-picker__back-btn" onClick={goBack}><ChevronLeft size={12} /></button>
          </Tooltip>
        )}
        {isSearchMode ? <><Search size={11} /><span>{t('fileMention.searchResults')}</span></> : (
          <span className="file-mention-picker__dir-name">{currentDirName}</span>
        )}
      </div>
      <div className="file-mention-picker__content">
        {displayItems.length === 0 && isLoading ? (
          <div className="file-mention-picker__loading"><Loader2 size={14} className="file-mention-picker__spinner" /><span>{t('fileMention.loading')}</span></div>
        ) : displayItems.length === 0 ? (
          <div className="file-mention-picker__empty"><span>{isSearchMode ? t('fileMention.noMatchingFiles') : t('fileMention.emptyDirectory')}</span></div>
        ) : (
          <div className="file-mention-picker__list">
            {displayItems.map((mention, index) => {
              const isSession = mention.kind === 'session';
              const file = mention.kind === 'file' ? mention.item : null;
              const session = mention.kind === 'session' ? mention.item : null;
              const key = isSession ? `session-${session?.sessionId}-${session?.workspacePath}` : `file-${file?.path}`;
              return (
                <div
                  key={key}
                  data-index={index}
                  className={`file-mention-picker__item ${index === selectedIndex ? 'file-mention-picker__item--selected' : ''}`}
                  onClick={() => handleItemClick(mention)}
                  onContextMenu={(event) => {
                    event.preventDefault();
                    if (file?.isDirectory) enterDirectory(file);
                  }}
                  onMouseEnter={() => setSelectedIndex(index)}
                >
                  {isSession ? <MessageCircle size={13} className="file-mention-picker__icon file-mention-picker__icon--session" /> : file?.isDirectory ? <Folder size={13} className="file-mention-picker__icon file-mention-picker__icon--folder" /> : <File size={13} className="file-mention-picker__icon file-mention-picker__icon--file" />}
                  <span className="file-mention-picker__item-name">{session?.sessionName ?? file?.name}</span>
                  {session && <span className="file-mention-picker__item-detail">{session.workspaceLabel}</span>}
                  {file?.isDirectory && !isSearchMode && <ChevronRight size={12} className="file-mention-picker__expand-icon" />}
                </div>
              );
            })}
          </div>
        )}
      </div>
      <div className="file-mention-picker__footer">
        <span><kbd>↑</kbd><kbd>↓</kbd> {t('fileMention.navHint')}</span>
        <span><kbd>→</kbd> {t('fileMention.enterHint')}</span>
        <span><kbd>←</kbd> {t('fileMention.backHint')}</span>
        <span><kbd>Enter</kbd> {t('fileMention.selectHint')}</span>
      </div>
    </div>
  );
};

export default FileMentionPicker;
