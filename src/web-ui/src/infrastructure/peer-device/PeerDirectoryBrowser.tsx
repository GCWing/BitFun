/**
 * In-app directory browser for Peer Device Mode.
 * Lists directories on the peer via HostInvoke FS APIs.
 */

import { Button, Icon, IconButton, Input, ScrollArea } from '@bitfun/ui';
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { Home, Loader2 } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';
import { workspaceAPI } from '@/infrastructure/api';
import { globalAPI } from '@/infrastructure/api/service-api/GlobalAPI';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import { createLogger } from '@/shared/utils/logger';
import { isImeOwnedKeyboardEvent } from '@/shared/utils/ime';
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
  const pathInputCompositionActiveRef = useRef(false);
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
          <IconButton
            className="peer-directory-browser__close-btn"
            icon={<Icon name="xmark" size="lg" />}
            size="md"
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
            <Icon name="arrow-left" size="sm" />
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
            <Icon name="refresh" size="sm" />
          </button>
          <div
            className="peer-directory-browser__path"
            data-bf-component="peer-device"
            data-bf-part="path"
          >
            {isEditingPath ? (
              <span
                className="peer-directory-browser__path-input-field"
                data-bf-component="peer-device"
                data-bf-part="pathInput"
              >
                <Input
                  ref={pathInputRef}
                  value={pathInputValue}
                  onValueChange={setPathInputValue}
                  onBlur={handleCommitPathInput}
                  onKeyDown={(event) => {
                    if (
                      (event.key === 'Enter' || event.key === 'Escape')
                      && isImeOwnedKeyboardEvent(event, pathInputCompositionActiveRef.current)
                    ) {
                      event.stopPropagation();
                      return;
                    }
                    if (event.key === 'Enter') {
                      event.preventDefault();
                      handleCommitPathInput();
                    } else if (event.key === 'Escape') {
                      event.preventDefault();
                      setPathInputValue(currentPath);
                      setIsEditingPath(false);
                    }
                  }}
                  onCompositionStart={() => {
                    pathInputCompositionActiveRef.current = true;
                  }}
                  onCompositionEnd={() => {
                    pathInputCompositionActiveRef.current = false;
                  }}
                />
              </span>
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

        <ScrollArea
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
                    <Icon name="folder" size="sm" />
                    <span>{entry.name}</span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </ScrollArea>

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
            <Button type="button" variant="outline" size="sm" onClick={onCancel}>
              {t('peerDirectoryPicker.cancel')}
            </Button>
            <Button
              type="button"
              variant="fill"
              size="sm"
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
