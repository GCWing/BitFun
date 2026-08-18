/**
 * In-app directory browser for Peer Device Mode.
 * Lists directories on the peer via HostInvoke FS APIs.
 */

import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import {
  ArrowLeft,
  Folder,
  Home,
  Loader2,
  RefreshCw,
} from 'lucide-react';
import { Button, PopupCloseButton } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { workspaceAPI } from '@/infrastructure/api';
import { globalAPI } from '@/infrastructure/api/service-api/GlobalAPI';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import { createLogger } from '@/shared/utils/logger';
import {
  joinDirectoryPath,
  parentDirectoryPath,
} from './peerDirectoryPath';
import './PeerDirectoryBrowser.scss';

const log = createLogger('PeerDirectoryBrowser');

export interface PeerDirectoryBrowserProps {
  visible?: boolean;
  title: string;
  initialPath?: string;
  onSelect: (path: string) => void;
  onCancel: () => void;
}

interface DirectoryEntry {
  name: string;
  path: string;
}

async function resolveStartPath(preferred?: string): Promise<string> {
  if (preferred && preferred.trim()) {
    return preferred.trim();
  }
  try {
    const opened = await globalAPI.getOpenedWorkspaces();
    const first = Array.isArray(opened) ? opened[0] : null;
    const rootPath = first && typeof first.rootPath === 'string' ? first.rootPath : null;
    if (rootPath) {
      return rootPath;
    }
  } catch (error) {
    log.debug('Failed to resolve peer start path from opened workspaces', error);
  }
  try {
    const info = await systemAPI.getSystemInfo();
    const platform = typeof info?.platform === 'string' ? info.platform.toLowerCase() : '';
    if (platform.includes('win')) {
      return 'C:\\';
    }
  } catch (error) {
    log.debug('Failed to resolve peer start path from system info', error);
  }
  return '/';
}

export const PeerDirectoryBrowser: React.FC<PeerDirectoryBrowserProps> = ({
  visible = true,
  title,
  initialPath,
  onSelect,
  onCancel,
}) => {
  const { t } = useI18n('common');
  const [currentPath, setCurrentPath] = useState(initialPath || '/');
  const [pathInputValue, setPathInputValue] = useState(initialPath || '/');
  const [isEditingPath, setIsEditingPath] = useState(false);
  const [entries, setEntries] = useState<DirectoryEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selectedPath, setSelectedPath] = useState<string | null>(initialPath || null);
  const pathInputRef = useRef<HTMLInputElement>(null);
  const loadSeqRef = useRef(0);

  const parentPath = useMemo(() => parentDirectoryPath(currentPath), [currentPath]);

  const loadDirectory = useCallback(async (path: string) => {
    const seq = ++loadSeqRef.current;
    setLoading(true);
    setError(null);
    try {
      const children = await workspaceAPI.getDirectoryChildren(path);
      if (seq !== loadSeqRef.current) {
        return;
      }
      const directories = (children || [])
        .filter((node) => node.isDirectory)
        .map((node) => ({
          name: node.name,
          path: node.path || joinDirectoryPath(path, node.name),
        }))
        .sort((a, b) => a.name.localeCompare(b.name));
      setEntries(directories);
      setCurrentPath(path);
      setPathInputValue(path);
      setSelectedPath(path);
    } catch (loadError) {
      if (seq !== loadSeqRef.current) {
        return;
      }
      const message = loadError instanceof Error ? loadError.message : String(loadError);
      log.warn('Failed to list peer directory', { path, error: loadError });
      setError(message);
      setEntries([]);
    } finally {
      if (seq === loadSeqRef.current) {
        setLoading(false);
      }
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const start = await resolveStartPath(initialPath);
      if (!cancelled) {
        await loadDirectory(start);
      }
    })();
    return () => {
      cancelled = true;
      loadSeqRef.current += 1;
    };
  }, [initialPath, loadDirectory]);

  useEffect(() => {
    if (isEditingPath) {
      pathInputRef.current?.focus();
      pathInputRef.current?.select();
    }
  }, [isEditingPath]);

  const handleGoParent = useCallback(() => {
    if (!parentPath) {
      return;
    }
    void loadDirectory(parentPath);
  }, [loadDirectory, parentPath]);

  const handleGoHome = useCallback(() => {
    void (async () => {
      const start = await resolveStartPath(initialPath);
      await loadDirectory(start);
    })();
  }, [initialPath, loadDirectory]);

  const handleRefresh = useCallback(() => {
    void loadDirectory(currentPath);
  }, [currentPath, loadDirectory]);

  const handleOpenEntry = useCallback((entry: DirectoryEntry) => {
    void loadDirectory(entry.path);
  }, [loadDirectory]);

  const handleCommitPathInput = useCallback(() => {
    const next = pathInputValue.trim();
    setIsEditingPath(false);
    if (!next || next === currentPath) {
      setPathInputValue(currentPath);
      return;
    }
    void loadDirectory(next);
  }, [currentPath, loadDirectory, pathInputValue]);

  const handleConfirm = useCallback(() => {
    const path = selectedPath || currentPath;
    if (!path) {
      return;
    }
    onSelect(path);
  }, [currentPath, onSelect, selectedPath]);

  return createPortal(
    <div
      className="peer-directory-browser-overlay"
      data-state={visible ? 'open' : 'closed'}
      role="dialog"
      aria-modal="true"
      aria-hidden={!visible}
      {...(!visible ? { inert: '' } : {})}
      data-bf-component="peer-device"
      data-bf-part="overlay"
    >
      <div
        className="peer-directory-browser"
        data-testid="peer-directory-browser"
        data-bf-component="peer-device"
        data-bf-part="dialog"
      >
        <div
          className="peer-directory-browser__header"
          data-bf-component="peer-device"
          data-bf-part="header"
        >
          <h2
            className="peer-directory-browser__header-title"
            data-bf-component="peer-device"
            data-bf-part="title"
          >{title}</h2>
          <PopupCloseButton
            className="peer-directory-browser__close-btn"
            aria-label={t('peerDirectoryPicker.cancel')}
            onClick={onCancel}
            data-bf-component="peer-device"
            data-bf-part="closeButton"
          />
        </div>

        <div
          className="peer-directory-browser__toolbar"
          data-bf-component="peer-device"
          data-bf-part="toolbar"
        >
          <button
            type="button"
            className="peer-directory-browser__tool-btn"
            disabled={!parentPath || loading}
            onClick={handleGoParent}
            title={t('peerDirectoryPicker.parent')}
            data-bf-component="peer-device"
            data-bf-part="toolButton"
          >
            <ArrowLeft size={14} />
          </button>
          <button
            type="button"
            className="peer-directory-browser__tool-btn"
            disabled={loading}
            onClick={handleGoHome}
            title={t('peerDirectoryPicker.home')}
            data-bf-component="peer-device"
            data-bf-part="toolButton"
          >
            <Home size={14} />
          </button>
          <button
            type="button"
            className="peer-directory-browser__tool-btn"
            disabled={loading}
            onClick={handleRefresh}
            title={t('peerDirectoryPicker.refresh')}
            data-bf-component="peer-device"
            data-bf-part="toolButton"
          >
            <RefreshCw size={14} />
          </button>
          <div
            className="peer-directory-browser__path"
            data-bf-component="peer-device"
            data-bf-part="path"
          >
            {isEditingPath ? (
              <input
                ref={pathInputRef}
                className="peer-directory-browser__path-input"
                value={pathInputValue}
                onChange={(event) => setPathInputValue(event.target.value)}
                onBlur={handleCommitPathInput}
                data-bf-component="peer-device"
                data-bf-part="pathInput"
                onKeyDown={(event) => {
                  if (event.key === 'Enter') {
                    event.preventDefault();
                    handleCommitPathInput();
                  } else if (event.key === 'Escape') {
                    event.preventDefault();
                    setPathInputValue(currentPath);
                    setIsEditingPath(false);
                  }
                }}
              />
            ) : (
              <button
                type="button"
                className="peer-directory-browser__path-display"
                onClick={() => setIsEditingPath(true)}
                title={currentPath}
                data-bf-component="peer-device"
                data-bf-part="pathDisplay"
              >
                {currentPath}
              </button>
            )}
          </div>
        </div>

        <div
          className="peer-directory-browser__body"
          data-bf-component="peer-device"
          data-bf-part="body"
        >
          {loading ? (
            <div
              className="peer-directory-browser__state"
              data-bf-component="peer-device"
              data-bf-part="status"
              data-bf-state="loading"
            >
              <Loader2 size={16} className="peer-directory-browser__spinner" />
              <span>{t('peerDirectoryPicker.loading')}</span>
            </div>
          ) : error ? (
            <div
              className="peer-directory-browser__state peer-directory-browser__state--error"
              data-bf-component="peer-device"
              data-bf-part="status"
              data-bf-state="error"
            >
              <span>{error}</span>
            </div>
          ) : entries.length === 0 ? (
            <div
              className="peer-directory-browser__state"
              data-bf-component="peer-device"
              data-bf-part="status"
              data-bf-state="empty"
            >
              <span>{t('peerDirectoryPicker.empty')}</span>
            </div>
          ) : (
            <ul
              className="peer-directory-browser__list"
              data-bf-component="peer-device"
              data-bf-part="list"
            >
              {entries.map((entry) => (
                <li key={entry.path}>
                  <button
                    type="button"
                    className={`peer-directory-browser__item${
                      selectedPath === entry.path ? ' is-selected' : ''
                    }`}
                    onClick={() => setSelectedPath(entry.path)}
                    onDoubleClick={() => handleOpenEntry(entry)}
                    data-bf-component="peer-device"
                    data-bf-part="item"
                    data-bf-state={selectedPath === entry.path ? 'selected' : undefined}
                  >
                    <Folder size={14} />
                    <span>{entry.name}</span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>

        <div
          className="peer-directory-browser__footer"
          data-bf-component="peer-device"
          data-bf-part="footer"
        >
          <div
            className="peer-directory-browser__selected"
            title={selectedPath || currentPath}
            data-bf-component="peer-device"
            data-bf-part="selection"
          >
            {t('peerDirectoryPicker.selected', { path: selectedPath || currentPath })}
          </div>
          <div
            className="peer-directory-browser__actions"
            data-bf-component="peer-device"
            data-bf-part="actions"
          >
            <Button type="button" variant="ghost" size="small" onClick={onCancel}>
              {t('peerDirectoryPicker.cancel')}
            </Button>
            <Button
              type="button"
              variant="primary"
              size="small"
              onClick={handleConfirm}
              disabled={!(selectedPath || currentPath)}
            >
              {t('peerDirectoryPicker.select')}
            </Button>
          </div>
        </div>
      </div>
    </div>,
    getAppearanceOverlayHost(),
  );
};

export default PeerDirectoryBrowser;
