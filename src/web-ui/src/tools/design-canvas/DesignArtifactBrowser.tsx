/**
 * Design artifacts browser — lists all artifacts in the current workspace with
 * search, archive toggles, and quick "Open in Canvas" actions. Rendered as a
 * dedicated `design-artifacts-browser` Tab in the right panel.
 */

import React, { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import {
  Archive,
  ArchiveRestore,
  ExternalLink,
  RefreshCcw,
  Search,
  Palette,
} from 'lucide-react';
import { useDesignArtifactStore } from './store/designArtifactStore';
import { designArtifactAPI } from './api';
import { ideControl } from '@/shared/services/ide-control';
import { createLogger } from '@/shared/utils/logger';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import './DesignArtifactBrowser.scss';

const log = createLogger('DesignArtifactBrowser');

export interface DesignArtifactBrowserProps {
  workspacePath?: string;
}

export const DesignArtifactBrowser: React.FC<DesignArtifactBrowserProps> = ({ workspacePath }) => {
  const { t } = useTranslation('flow-chat');
  const { workspacePath: currentWorkspacePath } = useCurrentWorkspace();
  const manifests = useDesignArtifactStore(
    useShallow((s) =>
      Object.values(s.artifacts)
        .map((artifact) => artifact.manifest)
        .sort((left, right) => left.id.localeCompare(right.id))
    )
  );
  const [query, setQuery] = useState('');
  const [showArchived, setShowArchived] = useState(false);
  const [isLoading, setIsLoading] = useState(false);

  const effectiveWorkspacePath = workspacePath || currentWorkspacePath;

  const refresh = useMemo(
    () => async () => {
      setIsLoading(true);
      try {
        await designArtifactAPI.list(effectiveWorkspacePath);
      } catch (err) {
        log.warn('Browser refresh failed', err);
      } finally {
        setIsLoading(false);
      }
    },
    [effectiveWorkspacePath]
  );

  useEffect(() => {
    refresh();
  }, [refresh]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return manifests
      .filter((m) => (showArchived ? true : !m.archived_at))
      .filter((m) => {
        if (!q) return true;
        return (
          m.title.toLowerCase().includes(q) ||
          m.id.toLowerCase().includes(q) ||
          m.kind.toLowerCase().includes(q)
        );
      })
      .sort((a, b) => (b.updated_at || '').localeCompare(a.updated_at || ''));
  }, [manifests, query, showArchived]);

  const openInCanvas = (id: string) => {
    const manifest = manifests.find((item) => item.id === id);
    if (!manifest) return;
    ideControl.panel.open('design-artifact', {
      position: 'right',
      config: {
        title: manifest.title,
        data: { artifactId: manifest.id, manifest },
        workspace_path: effectiveWorkspacePath,
      },
      options: { auto_focus: true, check_duplicate: true },
    });
  };

  const toggleArchive = async (id: string, archived: boolean) => {
    try {
      await designArtifactAPI.archive(id, archived, effectiveWorkspacePath);
    } catch (err) {
      log.warn('toggleArchive failed', err);
    }
  };

  return (
    <div className="design-artifact-browser">
      <div className="design-artifact-browser__toolbar">
        <div className="design-artifact-browser__search">
          <Search size={14} />
          <input
            type="text"
            value={query}
            placeholder={t('designCanvas.browser.searchPlaceholder')}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
        <label className="design-artifact-browser__archived-toggle">
          <input
            type="checkbox"
            checked={showArchived}
            onChange={(e) => setShowArchived(e.target.checked)}
          />
          {t('designCanvas.browser.showArchived')}
        </label>
        <button type="button" className="design-artifact-browser__refresh" onClick={refresh}>
          <RefreshCcw size={13} />
          {t('designCanvas.browser.refresh')}
        </button>
      </div>

      {isLoading && filtered.length === 0 ? (
        <div className="design-artifact-browser__empty">{t('designCanvas.browser.loading')}</div>
      ) : filtered.length === 0 ? (
        <div className="design-artifact-browser__empty">
          <Palette size={20} />
          <div>{t('designCanvas.browser.emptyTitle')}</div>
          <div className="design-artifact-browser__empty-hint">
            {t('designCanvas.browser.emptyHint')}
          </div>
        </div>
      ) : (
        <ul className="design-artifact-browser__list">
          {filtered.map((m) => {
            const archived = Boolean(m.archived_at);
            return (
              <li
                key={m.id}
                className={`design-artifact-browser__item${
                  archived ? ' design-artifact-browser__item--archived' : ''
                }`}
              >
                <button
                  type="button"
                  className="design-artifact-browser__thumb"
                  onClick={() => openInCanvas(m.id)}
                >
                  <Palette size={18} />
                </button>
                <div className="design-artifact-browser__meta">
                  <div className="design-artifact-browser__title">{m.title}</div>
                  <div className="design-artifact-browser__subtitle">
                    <code>{m.id}</code>
                    <span>{m.kind}</span>
                    <span>{t('designCanvas.browser.fileCount', { count: m.files.length })}</span>
                    <span>{t('designCanvas.browser.snapshotCount', { count: m.versions.length })}</span>
                    {m.current_version && <code>v{m.current_version.slice(0, 8)}</code>}
                  </div>
                </div>
                <div className="design-artifact-browser__actions">
                  <button
                    type="button"
                    className="design-artifact-browser__action"
                    onClick={() => openInCanvas(m.id)}
                    title={t('designCanvas.browser.openInCanvas')}
                  >
                    <ExternalLink size={13} />
                  </button>
                  <button
                    type="button"
                    className="design-artifact-browser__action"
                    onClick={() => toggleArchive(m.id, !archived)}
                    title={archived ? t('designCanvas.browser.unarchive') : t('designCanvas.browser.archive')}
                  >
                    {archived ? <ArchiveRestore size={13} /> : <Archive size={13} />}
                  </button>
                </div>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
};

export default DesignArtifactBrowser;
