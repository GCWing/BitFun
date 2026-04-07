/**
 * File mention picker.
 * Shown when the user types @ to select files or folders.
 */

import React, { useState, useEffect, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { File, Folder, Loader2, Search, ChevronRight, ChevronLeft } from 'lucide-react';
import { workspaceAPI } from '@/infrastructure/api';
import type {
  ExplorerNodeDto,
  FileSearchResult,
} from '@/infrastructure/api/service-api/tauri-commands';
import type { FileContext, DirectoryContext } from '@/shared/types/context';
import { Tooltip } from '@/component-library';
import { createLogger } from '@/shared/utils/logger';
import './FileMentionPicker.scss';

const log = createLogger('FileMentionPicker');
const FILE_MENTION_SEARCH_DEBOUNCE_MS = 300;
const FILE_MENTION_MAX_RESULTS = 30;

export interface FileMentionPickerProps {
  /** Whether the picker is open. */
  isOpen: boolean;
  /** Search keyword. */
  searchQuery: string;
  /** Workspace path. */
  workspacePath?: string;
  /** Selection callback. */
  onSelect: (context: FileContext | DirectoryContext) => void;
  /** Close callback. */
  onClose: () => void;
  /** Position info. */
  position?: { top: number; left: number };
  /** Keyboard navigation callback. */
  onNavigate?: (direction: 'up' | 'down' | 'enter' | 'escape') => void;
}

interface FileItem {
  path: string;
  name: string;
  isDirectory: boolean;
  relativePath: string;
}

export const FileMentionPicker: React.FC<FileMentionPickerProps> = ({
  isOpen,
  searchQuery,
  workspacePath,
  onSelect,
  onClose,
  position,
}) => {
  const { t } = useTranslation('flow-chat');
  const [results, setResults] = useState<FileItem[]>([]);
  const [currentFiles, setCurrentFiles] = useState<FileItem[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [currentPath, setCurrentPath] = useState<string>(''); // Current directory
  const [pathHistory, setPathHistory] = useState<string[]>([]); // Back navigation stack
  const containerRef = useRef<HTMLDivElement>(null);
  const abortControllerRef = useRef<AbortController | null>(null);
  const searchDebounceTimerRef = useRef<number | null>(null);
  const selectedItemHistoryRef = useRef<string[]>([]); // Selected path when entering a directory
  const targetSelectedPathRef = useRef<string | null>(null); // Target selection when returning
  const directoryLoadRequestIdRef = useRef(0);
  const searchRequestIdRef = useRef(0);
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
    setIsLoading(true);

    try {
      const targetPath = dirPath || workspacePath;
      const children = await workspaceAPI.getDirectoryChildren(targetPath);
      
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
        if (a.isDirectory && !b.isDirectory) return -1;
        if (!a.isDirectory && b.isDirectory) return 1;
        return a.name.localeCompare(b.name);
      });

      if (requestId !== directoryLoadRequestIdRef.current) {
        return;
      }

      setCurrentFiles(items);
      
      if (targetSelectedPath) {
        const targetIndex = items.findIndex(item => item.path === targetSelectedPath);
        setSelectedIndex(targetIndex >= 0 ? targetIndex : 0);
      } else {
        setSelectedIndex(0);
      }
    } catch (err) {
      log.error('Failed to load directory', err);
      if (requestId === directoryLoadRequestIdRef.current) {
        setCurrentFiles([]);
      }
    } finally {
      if (requestId === directoryLoadRequestIdRef.current) {
        setIsLoading(false);
      }
    }
  }, [workspacePath, getRelativePath]);

  const enterDirectory = useCallback((item: FileItem) => {
    if (!item.isDirectory) return;
    selectedItemHistoryRef.current = [...selectedItemHistoryRef.current, item.path];
    setPathHistory(prev => [...prev, currentPath]);
    setCurrentPath(item.path);
  }, [currentPath]);

  const goBack = useCallback(() => {
    if (pathHistory.length === 0) return;
    const previousPath = pathHistory[pathHistory.length - 1];
    const targetPath = selectedItemHistoryRef.current.length > 0 
      ? selectedItemHistoryRef.current[selectedItemHistoryRef.current.length - 1]
      : null;
    selectedItemHistoryRef.current = selectedItemHistoryRef.current.slice(0, -1);
    setPathHistory(prev => prev.slice(0, -1));
    targetSelectedPathRef.current = targetPath;
    setCurrentPath(previousPath);
  }, [pathHistory]);

  useEffect(() => {
    if (isOpen && workspacePath) {
      skipNextPathLoadRef.current = true;
      setCurrentPath('');
      setPathHistory([]);
      setCurrentFiles([]);
      setResults([]);
      setSelectedIndex(0);
      selectedItemHistoryRef.current = [];
      targetSelectedPathRef.current = null;
      loadDirectory('', null);
    }
  }, [isOpen, workspacePath, loadDirectory]);

  useEffect(() => {
    if (!isOpen || searchQuery.trim()) {
      return;
    }

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
        workspacePath,
        query,
        false, // caseSensitive
        false, // useRegex
        false, // wholeWord
        controller.signal
      );

      if (requestId !== searchRequestIdRef.current || controller.signal.aborted) {
        return;
      }

      const items: FileItem[] = searchResults.map((result: FileSearchResult) => ({
        path: result.path,
        name: result.name,
        isDirectory: result.isDirectory || false,
        relativePath: getRelativePath(result.path),
      }));

      items.sort((a, b) => {
        if (a.isDirectory && !b.isDirectory) return -1;
        if (!a.isDirectory && b.isDirectory) return 1;
        return a.name.localeCompare(b.name);
      });

      setResults(items.slice(0, FILE_MENTION_MAX_RESULTS));
      setSelectedIndex(0);
    } catch (err) {
      if (err instanceof DOMException && err.name === 'AbortError') {
        return;
      }
      log.error('Search failed', err);
      if (requestId === searchRequestIdRef.current) {
        setResults([]);
      }
    } finally {
      if (requestId === searchRequestIdRef.current && abortControllerRef.current === controller) {
        abortControllerRef.current = null;
        setIsLoading(false);
      }
    }
  }, [workspacePath, getRelativePath]);

  useEffect(() => {
    if (!isOpen) {
      return;
    }

    if (searchDebounceTimerRef.current !== null) {
      window.clearTimeout(searchDebounceTimerRef.current);
      searchDebounceTimerRef.current = null;
    }

    abortControllerRef.current?.abort();
    abortControllerRef.current = null;

    const trimmedQuery = searchQuery.trim();
    if (!trimmedQuery) {
      searchRequestIdRef.current += 1;
      setResults([]);
      setSelectedIndex(0);
      setIsLoading(false);
      return;
    }

    const requestId = ++searchRequestIdRef.current;
    const controller = new AbortController();
    abortControllerRef.current = controller;
    setIsLoading(true);

    searchDebounceTimerRef.current = window.setTimeout(() => {
      searchDebounceTimerRef.current = null;
      void searchFiles(trimmedQuery, controller, requestId);
    }, FILE_MENTION_SEARCH_DEBOUNCE_MS);

    return () => {
      if (searchDebounceTimerRef.current !== null) {
        window.clearTimeout(searchDebounceTimerRef.current);
        searchDebounceTimerRef.current = null;
      }
      controller.abort();
      if (abortControllerRef.current === controller) {
        abortControllerRef.current = null;
      }
    };
  }, [isOpen, searchQuery, searchFiles]);

  const isSearchMode = searchQuery.trim().length > 0;
  
  const displayItems = isSearchMode ? results : currentFiles;
  
  const currentDirName = currentPath 
    ? currentPath.replace(/\\/g, '/').split('/').pop() || ''
    : workspacePath?.replace(/\\/g, '/').split('/').pop() || t('fileMention.rootDirectory');

  useEffect(() => {
    return () => {
      if (searchDebounceTimerRef.current !== null) {
        window.clearTimeout(searchDebounceTimerRef.current);
      }
      abortControllerRef.current?.abort();
    };
  }, []);

  const handleSelect = useCallback((item: FileItem) => {
    const timestamp = Date.now();
    
    if (item.isDirectory) {
      const dirContext: DirectoryContext = {
        id: `dir-${timestamp}-${Math.random().toString(36).slice(2, 9)}`,
        type: 'directory',
        directoryPath: item.path,
        directoryName: item.name,
        recursive: true,
        timestamp,
      };
      onSelect(dirContext);
    } else {
      const fileContext: FileContext = {
        id: `file-${timestamp}-${Math.random().toString(36).slice(2, 9)}`,
        type: 'file',
        filePath: item.path,
        fileName: item.name,
        relativePath: item.relativePath,
        timestamp,
      };
      onSelect(fileContext);
    }
    
    onClose();
  }, [onSelect, onClose]);

  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if (!isOpen) return;

    switch (e.key) {
      case 'ArrowUp':
        e.preventDefault();
        e.stopPropagation();
        if (displayItems.length > 0) {
          setSelectedIndex(prev => (prev > 0 ? prev - 1 : displayItems.length - 1));
        }
        break;
      case 'ArrowDown':
        e.preventDefault();
        e.stopPropagation();
        if (displayItems.length > 0) {
          setSelectedIndex(prev => (prev < displayItems.length - 1 ? prev + 1 : 0));
        }
        break;
      case 'ArrowRight':
        e.preventDefault();
        e.stopPropagation();
        if (!isSearchMode && displayItems[selectedIndex]?.isDirectory) {
          enterDirectory(displayItems[selectedIndex]);
        }
        break;
      case 'ArrowLeft':
        e.preventDefault();
        e.stopPropagation();
        if (!isSearchMode && pathHistory.length > 0) {
          goBack();
        }
        break;
      case 'Enter':
        e.preventDefault();
        e.stopPropagation();
        if (displayItems.length > 0 && displayItems[selectedIndex]) {
          handleSelect(displayItems[selectedIndex]);
        }
        break;
      case 'Escape':
        e.preventDefault();
        e.stopPropagation();
        onClose();
        break;
      case 'Tab':
        e.preventDefault();
        e.stopPropagation();
        if (displayItems.length > 0 && displayItems[selectedIndex]) {
          handleSelect(displayItems[selectedIndex]);
        }
        break;
    }
  }, [isOpen, displayItems, selectedIndex, handleSelect, onClose, isSearchMode, enterDirectory, goBack, pathHistory.length]);

  useEffect(() => {
    if (isOpen) {
      document.addEventListener('keydown', handleKeyDown, true);
      return () => {
        document.removeEventListener('keydown', handleKeyDown, true);
      };
    }
  }, [isOpen, handleKeyDown]);

  useEffect(() => {
    if (containerRef.current && displayItems.length > 0) {
      const container = containerRef.current;
      const selectedElement = container.querySelector(`[data-index="${selectedIndex}"]`);
      if (selectedElement) {
        selectedElement.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
      }
    }
  }, [selectedIndex, displayItems.length]);

  const getIcon = (item: FileItem) => {
    if (item.isDirectory) {
      return <Folder size={13} className="file-mention-picker__icon file-mention-picker__icon--folder" />;
    }
    return <File size={13} className="file-mention-picker__icon file-mention-picker__icon--file" />;
  };

  // Right-click to enter a folder (must be defined before early returns).
  const handleContextMenu = useCallback((e: React.MouseEvent, item: FileItem) => {
    e.preventDefault();
    if (item.isDirectory) {
      enterDirectory(item);
    }
  }, [enterDirectory]);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
  }, []);

  if (!isOpen) return null;

  const style: React.CSSProperties = position ? {
    position: 'absolute',
    top: position.top,
    left: position.left,
  } : {};

  return (
    <div 
      ref={containerRef}
      className="file-mention-picker"
      style={style}
      onMouseDown={handleMouseDown}
    >
      <div className="file-mention-picker__header">
        {!isSearchMode && pathHistory.length > 0 && (
          <Tooltip content={t('fileMention.goBack')}>
            <button 
              className="file-mention-picker__back-btn"
              onClick={goBack}
            >
              <ChevronLeft size={12} />
            </button>
          </Tooltip>
        )}
        {isSearchMode ? (
          <>
            <Search size={11} />
            <span>{t('fileMention.searchResults')}</span>
          </>
        ) : (
          <span className="file-mention-picker__dir-name">{currentDirName}</span>
        )}
      </div>
      
      <div className="file-mention-picker__content">
        {isLoading ? (
          <div className="file-mention-picker__loading">
            <Loader2 size={14} className="file-mention-picker__spinner" />
            <span>{t('fileMention.loading')}</span>
          </div>
        ) : displayItems.length === 0 ? (
          <div className="file-mention-picker__empty">
            {isSearchMode ? (
              <span>{t('fileMention.noMatchingFiles')}</span>
            ) : (
              <span>{t('fileMention.emptyDirectory')}</span>
            )}
          </div>
        ) : (
          <div className="file-mention-picker__list">
            {displayItems.map((item, index) => (
              <div
                key={item.path}
                data-index={index}
                className={`file-mention-picker__item ${index === selectedIndex ? 'file-mention-picker__item--selected' : ''}`}
                onClick={() => handleSelect(item)}
                onContextMenu={(e) => handleContextMenu(e, item)}
                onMouseEnter={() => setSelectedIndex(index)}
              >
                {getIcon(item)}
                <span className="file-mention-picker__item-name">{item.name}</span>
                {item.isDirectory && !isSearchMode && (
                  <ChevronRight size={12} className="file-mention-picker__expand-icon" />
                )}
              </div>
            ))}
          </div>
        )}
      </div>
      
      <div className="file-mention-picker__footer">
        <span><kbd>↑</kbd><kbd>↓</kbd> {t('fileMention.navHint')}</span>
        {!isSearchMode && <span><kbd>→</kbd> {t('fileMention.enterHint')}</span>}
        {!isSearchMode && <span><kbd>←</kbd> {t('fileMention.backHint')}</span>}
        <span><kbd>Enter</kbd> {t('fileMention.selectHint')}</span>
      </div>
    </div>
  );
};

export default FileMentionPicker;

