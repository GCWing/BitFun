import React, { useCallback, useEffect, useMemo, useState } from 'react';
import {
  Brain,
  Database,
  ExternalLink,
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

  const selectedCanDelete = selectedRecord && !selectedRecord.isIndex;

  return (
    <div className="memory-scene">
      <aside className="memory-scene__sidebar">
        <div className="memory-scene__brand">
          <span className="memory-scene__brand-icon"><Brain size={18} /></span>
          <div>
            <h2>{t('memoryLibrary.title')}</h2>
            <p>{t('memoryLibrary.subtitle')}</p>
          </div>
        </div>

        <div className="memory-scene__scope-list">
          {SCOPE_FILTERS.map((scope) => (
            <button
              key={scope}
              type="button"
              className={`memory-scene__scope-item${scopeFilter === scope ? ' is-active' : ''}`}
              onClick={() => setScopeFilter(scope)}
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
          ))}
        </div>

        <div className="memory-scene__status-card">
          <div className="memory-scene__status-title">
            <Sparkles size={14} />
            {t('memoryLibrary.autoMemory.title')}
          </div>
          <p>
            {autoMemoryStatus
              ? t('memoryLibrary.autoMemory.summary', {
                global: autoMemoryStatus.globalEnabled
                  ? t('memoryLibrary.autoMemory.enabledEvery', { count: autoMemoryStatus.globalEvery })
                  : t('memoryLibrary.autoMemory.disabled'),
                workspace: autoMemoryStatus.workspaceEnabled
                  ? t('memoryLibrary.autoMemory.enabledEvery', { count: autoMemoryStatus.workspaceEvery })
                  : t('memoryLibrary.autoMemory.disabled'),
              })
              : t('memoryLibrary.autoMemory.loading')}
          </p>
          <button type="button" onClick={handleOpenSettings}>
            <Settings size={13} />
            {t('memoryLibrary.actions.openSettings')}
          </button>
        </div>

        {spaces.map((space) => (
          <div key={space.scope} className="memory-scene__path-card">
            <span>{space.label}</span>
            <code>{space.available ? space.memoryDir : t('memoryLibrary.empty.unavailable')}</code>
          </div>
        ))}
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
                  <span className="memory-scene__record-title">{record.title}</span>
                  <span className="memory-scene__record-desc">
                    {record.description || record.relativePath}
                  </span>
                  <span className="memory-scene__record-meta">
                    <Badge variant="neutral">{t(`memoryLibrary.types.${record.type}`)}</Badge>
                    <span>{t(`memoryLibrary.scopes.${record.scope}`)}</span>
                    {record.updatedAt ? <span>{formatDate(record.updatedAt)}</span> : null}
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
                <ExternalLink size={14} />
                <span>{t(`memoryLibrary.usageHints.${selectedRecord.type}`)}</span>
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
