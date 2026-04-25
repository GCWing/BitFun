import React, { useCallback, useEffect, useMemo, useState } from 'react';
import {
  Database,
  FileText,
  FolderOpen,
  RefreshCcw,
  Save,
  Settings,
  Sparkles,
  Trash2,
  X,
} from 'lucide-react';
import { Badge, Button, ConfirmDialog, Markdown, Search } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { WorkspaceKind } from '@/shared/types';
import { notificationService } from '@/shared/notification-system';
import { useOverlayManager } from '../../hooks/useOverlayManager';
import { useSettingsStore } from '../settings/settingsStore';
import {
  memoryLibraryAPI,
  type AutoMemoryStatus,
  type MemoryRecord,
  type MemoryRecordType,
  type MemoryScopeKey,
  type MemorySpace,
} from './MemoryLibraryAPI';
import './MemoryScene.scss';

type ScopeFilter = 'all' | MemoryScopeKey | 'workspace_overview';
type TypeFilter = 'all' | MemoryRecordType;

const MEMORY_TYPES: TypeFilter[] = [
  'all',
  'index',
  'user',
  'feedback',
  'project',
  'reference',
  'workspace_overview',
  'unknown',
];

const SCOPE_FILTERS: ScopeFilter[] = ['all', 'global', 'workspace', 'workspace_overview'];

function formatDate(timestamp?: number): string {
  if (!timestamp) return '';
  return new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(timestamp));
}

function getRecordExcerpt(record: MemoryRecord): string {
  return (record.description || record.body || record.relativePath)
    .replace(/\s+/g, ' ')
    .trim();
}

const MemoryScene: React.FC = () => {
  const { t } = useI18n('common');
  const { workspace, workspacePath, hasWorkspace } = useCurrentWorkspace();
  const { openOverlay } = useOverlayManager();
  const setSettingsTab = useSettingsStore((state) => state.setActiveTab);

  const [records, setRecords] = useState<MemoryRecord[]>([]);
  const [spaces, setSpaces] = useState<MemorySpace[]>([]);
  const [autoMemoryStatus, setAutoMemoryStatus] = useState<AutoMemoryStatus | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [query, setQuery] = useState('');
  const [scopeFilter, setScopeFilter] = useState<ScopeFilter>('all');
  const [typeFilter, setTypeFilter] = useState<TypeFilter>('all');
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [isEditing, setIsEditing] = useState(false);
  const [draftContent, setDraftContent] = useState('');
  const [deleteTarget, setDeleteTarget] = useState<MemoryRecord | null>(null);

  const loadRecords = useCallback(async () => {
    setIsLoading(true);
    try {
      const [storagePaths, status] = await Promise.all([
        memoryLibraryAPI.getStoragePaths(),
        memoryLibraryAPI.getAutoMemoryStatus(),
      ]);

      const nextSpaces: MemorySpace[] = [
        {
          scope: 'global',
          label: t('memoryLibrary.scopes.global'),
          memoryDir: storagePaths.agenticOsMemoryDir,
          available: true,
        },
      ];

      if (hasWorkspace && workspacePath && workspace?.workspaceKind !== WorkspaceKind.Remote) {
        try {
          const projectPaths = await memoryLibraryAPI.getProjectStoragePaths(workspacePath);
          nextSpaces.push({
            scope: 'workspace',
            label: t('memoryLibrary.scopes.workspace'),
            memoryDir: projectPaths.memoryDir,
            available: true,
          });
        } catch {
          nextSpaces.push({
            scope: 'workspace',
            label: t('memoryLibrary.scopes.workspace'),
            memoryDir: '',
            available: false,
          });
        }
      }

      const nextRecords = (await Promise.all(
        nextSpaces.map((space) => memoryLibraryAPI.listMemoryRecords(space))
      )).flat();

      setSpaces(nextSpaces);
      setAutoMemoryStatus(status);
      setRecords(nextRecords);
      setSelectedId((current) => {
        if (current && nextRecords.some((record) => record.id === current)) return current;
        return nextRecords[0]?.id ?? null;
      });
    } catch (error) {
      notificationService.error(t('memoryLibrary.messages.loadFailed'));
    } finally {
      setIsLoading(false);
    }
  }, [hasWorkspace, t, workspace, workspacePath]);

  useEffect(() => {
    void loadRecords();
  }, [loadRecords]);

  const filteredRecords = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase();
    return records.filter((record) => {
      if (scopeFilter === 'global' && record.scope !== 'global') return false;
      if (scopeFilter === 'workspace' && record.scope !== 'workspace') return false;
      if (scopeFilter === 'workspace_overview' && !record.isWorkspaceOverview) return false;
      if (typeFilter !== 'all' && record.type !== typeFilter) return false;
      if (!normalizedQuery) return true;
      const haystack = [
        record.title,
        record.description,
        record.relativePath,
        record.type,
        record.scope,
        record.content,
      ].join(' ').toLowerCase();
      return haystack.includes(normalizedQuery);
    });
  }, [query, records, scopeFilter, typeFilter]);

  const selectedRecord = useMemo(
    () => records.find((record) => record.id === selectedId) ?? filteredRecords[0] ?? null,
    [filteredRecords, records, selectedId]
  );

  useEffect(() => {
    if (selectedRecord && !filteredRecords.some((record) => record.id === selectedRecord.id)) {
      setSelectedId(filteredRecords[0]?.id ?? null);
    }
  }, [filteredRecords, selectedRecord]);

  useEffect(() => {
    setIsEditing(false);
    setDraftContent(selectedRecord?.content ?? '');
  }, [selectedRecord?.id, selectedRecord?.content]);

  const counts = useMemo(() => ({
    global: records.filter((record) => record.scope === 'global').length,
    workspace: records.filter((record) => record.scope === 'workspace').length,
    overview: records.filter((record) => record.isWorkspaceOverview).length,
  }), [records]);

  const workspaceSpace = spaces.find((space) => space.scope === 'workspace');
  const workspaceUnavailable = !hasWorkspace || workspaceSpace?.available === false;

  useEffect(() => {
    if (scopeFilter === 'workspace' && workspaceUnavailable) {
      setScopeFilter('all');
    }
  }, [scopeFilter, workspaceUnavailable]);

  const handleSave = async () => {
    if (!selectedRecord) return;
    setIsSaving(true);
    try {
      const refreshed = await memoryLibraryAPI.saveMemoryRecord(selectedRecord, draftContent);
      setRecords((current) => current.map((record) => (
        record.id === selectedRecord.id ? refreshed : record
      )));
      setSelectedId(refreshed.id);
      setIsEditing(false);
      notificationService.success(t('memoryLibrary.messages.saveSuccess'));
    } catch {
      notificationService.error(t('memoryLibrary.messages.saveFailed'));
    } finally {
      setIsSaving(false);
    }
  };

  const handleDelete = async (record: MemoryRecord) => {
    try {
      await memoryLibraryAPI.deleteMemoryRecord(record);
      setRecords((current) => current.filter((item) => item.id !== record.id));
      setSelectedId((current) => current === record.id ? null : current);
      notificationService.success(t('memoryLibrary.messages.deleteSuccess'));
    } catch {
      notificationService.error(t('memoryLibrary.messages.deleteFailed'));
    }
  };

  const handleOpenSettings = () => {
    setSettingsTab('memory');
    openOverlay('settings');
  };

  const handleOpenMemorySpace = async (space: MemorySpace) => {
    if (!space.available) return;
    try {
      await memoryLibraryAPI.revealMemorySpace(space);
    } catch {
      notificationService.error(t('memoryLibrary.messages.revealFailed'));
    }
  };

  const selectedCanDelete = selectedRecord && !selectedRecord.isIndex;

  return (
    <div className="memory-scene">
      <aside className="memory-scene__sidebar">
        <div className="memory-scene__brand">
          <div>
            <h2>{t('memoryLibrary.title')}</h2>
            <p>{t('memoryLibrary.subtitle')}</p>
          </div>
          <button
            type="button"
            className="memory-scene__header-settings"
            onClick={handleOpenSettings}
            aria-label={t('memoryLibrary.actions.openSettings')}
            title={t('memoryLibrary.actions.openSettings')}
          >
            <Settings size={15} />
          </button>
        </div>

        <div className="memory-scene__scope-list">
          <div className="memory-scene__section-label">{t('memoryLibrary.sidebar.scope')}</div>
          {SCOPE_FILTERS.map((scope) => {
            const isDisabled = scope === 'workspace' && workspaceUnavailable;
            return (
              <button
                key={scope}
                type="button"
                className={`memory-scene__scope-item${scopeFilter === scope ? ' is-active' : ''}`}
                onClick={() => !isDisabled && setScopeFilter(scope)}
                disabled={isDisabled}
              >
                <span>{t(`memoryLibrary.scopeFilters.${scope}`)}</span>
                <strong>
                  {scope === 'all'
                    ? records.length
                    : scope === 'global'
                      ? counts.global
                      : scope === 'workspace'
                        ? counts.workspace
                        : counts.overview}
                </strong>
              </button>
            );
          })}
          {workspaceUnavailable ? (
            <div className="memory-scene__scope-note">
              {hasWorkspace
                ? t('memoryLibrary.sidebar.workspaceUnavailable')
                : t('memoryLibrary.sidebar.noWorkspace')}
            </div>
          ) : null}
        </div>

        <div className="memory-scene__status-section">
          <div className="memory-scene__section-heading">
            <span>{t('memoryLibrary.autoMemory.title')}</span>
          </div>
          <div className="memory-scene__status-rows">
            <div className="memory-scene__status-row">
              <span>
                <Sparkles size={13} />
                {t('memoryLibrary.scopes.global')}
              </span>
              <strong className={autoMemoryStatus?.globalEnabled ? 'is-on' : ''}>
                {autoMemoryStatus
                  ? autoMemoryStatus.globalEnabled
                    ? t('memoryLibrary.autoMemory.enabledEvery', { count: autoMemoryStatus.globalEvery })
                    : t('memoryLibrary.autoMemory.disabled')
                  : t('memoryLibrary.autoMemory.loading')}
              </strong>
            </div>
            <div className="memory-scene__status-row">
              <span>
                <Sparkles size={13} />
                {t('memoryLibrary.scopes.workspace')}
              </span>
              <strong className={autoMemoryStatus?.workspaceEnabled ? 'is-on' : ''}>
                {autoMemoryStatus
                  ? autoMemoryStatus.workspaceEnabled
                    ? t('memoryLibrary.autoMemory.enabledEvery', { count: autoMemoryStatus.workspaceEvery })
                    : t('memoryLibrary.autoMemory.disabled')
                  : t('memoryLibrary.autoMemory.loading')}
              </strong>
            </div>
          </div>
        </div>

        <details className="memory-scene__paths">
          <summary>{t('memoryLibrary.sidebar.storage')}</summary>
          <div className="memory-scene__path-list">
            {spaces.map((space) => (
              <div key={space.scope} className="memory-scene__path-row">
                <span>{space.label}</span>
                <button
                  type="button"
                  title={space.available ? space.memoryDir : t('memoryLibrary.empty.unavailable')}
                  disabled={!space.available}
                  onClick={() => void handleOpenMemorySpace(space)}
                >
                  {space.available
                    ? t('memoryLibrary.actions.openStorage')
                    : t('memoryLibrary.empty.unavailable')}
                </button>
              </div>
            ))}
          </div>
        </details>
      </aside>

      <main className="memory-scene__main">
        <section className="memory-scene__list-pane">
          <div className="memory-scene__toolbar">
            <Search
              value={query}
              onChange={setQuery}
              onClear={() => setQuery('')}
              placeholder={t('memoryLibrary.searchPlaceholder')}
              size="medium"
            />
            <button type="button" className="memory-scene__icon-btn" onClick={() => void loadRecords()}>
              <RefreshCcw size={15} />
            </button>
          </div>

          <div className="memory-scene__type-pills">
            {MEMORY_TYPES.map((type) => (
              <button
                key={type}
                type="button"
                className={`memory-scene__type-pill${typeFilter === type ? ' is-active' : ''}`}
                onClick={() => setTypeFilter(type)}
              >
                {t(`memoryLibrary.types.${type}`)}
              </button>
            ))}
          </div>

          <div className="memory-scene__result-line">
            <span>{t('memoryLibrary.results.showing', {
              shown: filteredRecords.length,
              total: records.length,
            })}</span>
            <span>{t(`memoryLibrary.scopeFilters.${scopeFilter}`)}</span>
          </div>

          <div className="memory-scene__records" aria-busy={isLoading}>
            {isLoading ? (
              <div className="memory-scene__empty">{t('memoryLibrary.loading')}</div>
            ) : filteredRecords.length === 0 ? (
              <div className="memory-scene__empty">{t('memoryLibrary.empty.noResults')}</div>
            ) : filteredRecords.map((record) => (
              <button
                key={record.id}
                type="button"
                className={`memory-scene__record-card${selectedRecord?.id === record.id ? ' is-selected' : ''}`}
                onClick={() => setSelectedId(record.id)}
              >
                <span className="memory-scene__record-icon">
                  {record.isIndex ? <Database size={15} /> : <FileText size={15} />}
                </span>
                <span className="memory-scene__record-body">
                  <span className="memory-scene__record-title-row">
                    <span className="memory-scene__record-title">{record.title}</span>
                    {record.updatedAt ? <span>{formatDate(record.updatedAt)}</span> : null}
                  </span>
                  <span className="memory-scene__record-desc">
                    {getRecordExcerpt(record)}
                  </span>
                  <span className="memory-scene__record-meta">
                    <Badge variant="neutral">{t(`memoryLibrary.types.${record.type}`)}</Badge>
                    <span>{t(`memoryLibrary.scopes.${record.scope}`)}</span>
                  </span>
                </span>
              </button>
            ))}
          </div>
        </section>

        <section className="memory-scene__detail-pane">
          {selectedRecord ? (
            <>
              <header className="memory-scene__detail-header">
                <div>
                  <div className="memory-scene__detail-kicker">
                    {t(`memoryLibrary.scopes.${selectedRecord.scope}`)} / {selectedRecord.relativePath}
                  </div>
                  <h3>{selectedRecord.title}</h3>
                  {selectedRecord.description ? <p>{selectedRecord.description}</p> : null}
                  <div className="memory-scene__detail-metadata">
                    <span>{t(`memoryLibrary.types.${selectedRecord.type}`)}</span>
                    {selectedRecord.updatedAt ? <span>{formatDate(selectedRecord.updatedAt)}</span> : null}
                  </div>
                </div>
                <div className="memory-scene__detail-actions">
                  {isEditing ? (
                    <>
                      <Button size="small" variant="primary" onClick={() => void handleSave()} disabled={isSaving}>
                          <Save size={14} />
                        {t('memoryLibrary.actions.save')}
                      </Button>
                      <Button size="small" variant="secondary" onClick={() => setIsEditing(false)} disabled={isSaving}>
                          <X size={14} />
                        {t('memoryLibrary.actions.cancel')}
                      </Button>
                    </>
                  ) : (
                    <>
                      <Button size="small" variant="secondary" onClick={() => setIsEditing(true)}>
                        {t('memoryLibrary.actions.edit')}
                      </Button>
                      <Button size="small" variant="secondary" onClick={() => void memoryLibraryAPI.revealMemoryRecord(selectedRecord)}>
                          <FolderOpen size={14} />
                        {t('memoryLibrary.actions.reveal')}
                      </Button>
                      <Button
                        size="small"
                        variant="danger"
                        disabled={!selectedCanDelete}
                        onClick={() => selectedCanDelete && setDeleteTarget(selectedRecord)}
                      >
                          <Trash2 size={14} />
                        {t('memoryLibrary.actions.forget')}
                      </Button>
                    </>
                  )}
                </div>
              </header>

              <div className="memory-scene__explain-card">
                {t(`memoryLibrary.usageHints.${selectedRecord.type}`)}
              </div>

              {isEditing ? (
                <textarea
                  className="memory-scene__editor"
                  value={draftContent}
                  onChange={(event) => setDraftContent(event.target.value)}
                  spellCheck={false}
                />
              ) : (
                <div className="memory-scene__markdown">
                  <Markdown content={selectedRecord.content || t('memoryLibrary.empty.emptyFile')} />
                </div>
              )}
            </>
          ) : (
            <div className="memory-scene__detail-empty">{t('memoryLibrary.empty.noSelection')}</div>
          )}
        </section>
      </main>

      <ConfirmDialog
        isOpen={Boolean(deleteTarget)}
        onClose={() => setDeleteTarget(null)}
        onConfirm={() => deleteTarget && void handleDelete(deleteTarget)}
        title={t('memoryLibrary.deleteDialog.title')}
        message={t('memoryLibrary.deleteDialog.message', { name: deleteTarget?.title ?? '' })}
        confirmText={t('memoryLibrary.actions.forget')}
        confirmDanger
        preview={deleteTarget?.relativePath}
      />
    </div>
  );
};

export default MemoryScene;
