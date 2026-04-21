import React, { useCallback, useEffect, useMemo, useState } from 'react';
import {
  CheckCircle2,
  ChevronLeft,
  ChevronRight,
  Download,
  FolderOpen,
  Package,
  Plus,
  Puzzle,
  Sparkles,
  Store,
  Trash2,
  TrendingUp,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Badge, Button, ConfirmDialog, Input, Modal, Search, Select } from '@/component-library';
import { GalleryDetailModal } from '@/app/components';
import type { SkillInfo, SkillLevel, SkillMarketItem } from '@/infrastructure/config/types';
import { workspaceAPI } from '@/infrastructure/api';
import { workspaceManager } from '@/infrastructure/services/business/workspaceManager';
import { useNotification } from '@/shared/notification-system';
import { isRemoteWorkspace } from '@/shared/types';
import { createLogger } from '@/shared/utils/logger';
import { getCardGradient } from '@/shared/utils/cardGradients';
import { useInstalledSkills } from './hooks/useInstalledSkills';
import { useSkillMarket } from './hooks/useSkillMarket';
import SkillCard from './components/SkillCard';
import './SkillsScene.scss';
import { useSkillsSceneStore } from './skillsSceneStore';
import { useGallerySceneAutoRefresh } from '@/app/hooks/useGallerySceneAutoRefresh';

const log = createLogger('SkillsScene');

const SKILLS_SOURCE_URL = 'https://skills.sh';

const INSTALLED_PAGE_SIZE = 10;

const SkillsScene: React.FC = () => {
  const { t } = useTranslation('scenes/skills');
  const notification = useNotification();
  const {
    searchDraft,
    marketQuery,
    installedFilter,
    isAddFormOpen,
    setSearchDraft,
    submitMarketQuery,
    setInstalledFilter,
    setAddFormOpen,
    toggleAddForm,
  } = useSkillsSceneStore();

  const [deleteTarget, setDeleteTarget] = useState<SkillInfo | null>(null);
  const [installedListPage, setInstalledListPage] = useState(0);
  const [installedSearch, setInstalledSearch] = useState('');
  const [selectedDetail, setSelectedDetail] = useState<
    | { type: 'installed'; skill: SkillInfo }
    | { type: 'market'; skill: SkillMarketItem }
    | null
  >(null);

  const installed = useInstalledSkills({
    searchQuery: installedSearch,
    activeFilter: installedFilter,
  });

  const installedSkillNames = useMemo(
    () => new Set(installed.skills.map((skill) => skill.name)),
    [installed.skills],
  );

  const market = useSkillMarket({
    searchQuery: marketQuery,
    installedSkillNames,
    pageSize: 12,
    onInstalledChanged: async () => {
      await installed.loadSkills(true);
    },
  });

  const refetchSkillsScene = useCallback(async () => {
    await Promise.all([installed.loadSkills(true), market.refresh()]);
  }, [installed, market]);

  useGallerySceneAutoRefresh({
    sceneId: 'skills',
    refetch: refetchSkillsScene,
  });

  const canRevealSkillPath = !isRemoteWorkspace(workspaceManager.getState().currentWorkspace);

  const handleRevealSkillPath = useCallback(
    async (path: string) => {
      if (!canRevealSkillPath || !path.trim()) {
        return;
      }
      try {
        await workspaceAPI.revealInExplorer(path);
      } catch (error) {
        log.error('Failed to reveal skill path in explorer', { path, error });
        notification.error(t('messages.revealPathFailed', { error: String(error) }));
      }
    },
    [canRevealSkillPath, notification, t],
  );

  const handleAddSkill = async () => {
    const added = await installed.handleAdd();
    if (added) {
      setAddFormOpen(false);
      await market.refresh();
    }
  };

  const selectedInstalledSkill = selectedDetail?.type === 'installed' ? selectedDetail.skill : null;
  const selectedMarketSkill = selectedDetail?.type === 'market' ? selectedDetail.skill : null;

  const installedFiltered = installed.filteredSkills;
  const installedTotalPages = Math.max(
    1,
    Math.ceil(installedFiltered.length / INSTALLED_PAGE_SIZE),
  );
  const currentInstalledPage = Math.min(installedListPage, installedTotalPages - 1);
  const pagedInstalledSkills = installedFiltered.slice(
    currentInstalledPage * INSTALLED_PAGE_SIZE,
    (currentInstalledPage + 1) * INSTALLED_PAGE_SIZE,
  );

  useEffect(() => {
    setInstalledListPage(0);
  }, [installedFilter, installedSearch]);

  useEffect(() => {
    setInstalledListPage((p) => Math.min(p, Math.max(0, installedTotalPages - 1)));
  }, [installedTotalPages]);

  const marketSkeletonGrid = (keyPrefix: string) => (
    <div className="skills-split__skeleton-grid" aria-busy="true" aria-label={t('list.loading')}>
      {Array.from({ length: 12 }).map((_, i) => (
        <div
          key={`${keyPrefix}-${i}`}
          className="skills-split__skeleton-card"
          style={{ '--card-index': i } as React.CSSProperties}
        />
      ))}
    </div>
  );

  return (
    <div className="bitfun-skills-scene">
      {/* ── Top header ── */}
      <header className="skills-scene__header">
        <div className="skills-scene__identity">
          <h1 className="skills-scene__title">{t('page.title')}</h1>
          <div className="skills-scene__subline">
            <p className="skills-scene__subtitle">{t('page.subtitle')}</p>
            <div className="skills-scene__actions">
              <button
                type="button"
                className="skills-split__add-btn"
                onClick={toggleAddForm}
              >
                <Plus size={14} />
                <span>{t('toolbar.addTooltip')}</span>
              </button>
            </div>
          </div>
        </div>
      </header>

      {/* ── Two-column split layout ── */}
      <div className="skills-split">

        {/* ══ LEFT: market skills ══ */}
        <div className="skills-split__left">
          {/* Market panel header */}
          <div className="skills-split__left-header">
            <div className="skills-split__section-head">
              <span className="skills-split__section-title">{t('market.title')}</span>
              <span className="skills-split__section-sub">
                {t('market.subtitlePrefix')}
                {' '}
                <a href={SKILLS_SOURCE_URL} target="_blank" rel="noreferrer">skills.sh</a>
                {t('market.subtitleSuffix')}
              </span>
            </div>
            <Search
              className="skills-split__panel-search"
              value={searchDraft}
              onChange={setSearchDraft}
              onSearch={submitMarketQuery}
              onClear={submitMarketQuery}
              placeholder={t('page.searchPlaceholder')}
              size="small"
              clearable
              enterToSearch
            />
          </div>
          {/* Market body — scrollable */}
          <div className="skills-split__left-body">

            {/* Market loading — skeleton grid */}
            {market.marketLoading && marketSkeletonGrid('mkt-init')}

            {/* Market error */}
            {!market.marketLoading && market.marketError && (
              <div className="skills-split__empty skills-split__empty--error">
                <Store size={28} strokeWidth={1.5} />
                <span>{market.marketError}</span>
              </div>
            )}

            {/* Pagination fetch — same skeleton as initial load */}
            {!market.marketLoading && !market.marketError && market.loadingMore && marketSkeletonGrid('mkt-page')}

            {/* Market empty */}
            {!market.marketLoading && !market.marketError && !market.loadingMore && market.marketSkills.length === 0 && (
              <div className="skills-split__empty">
                <Store size={28} strokeWidth={1.5} />
                <span>{marketQuery ? t('market.empty.noMatch') : t('market.empty.noSkills')}</span>
              </div>
            )}

            {/* Market cards grid — 3×4, 12 per page */}
            {!market.marketLoading && !market.marketError && !market.loadingMore && market.marketSkills.length > 0 && (
              <div className="skills-split__market-grid">
                {market.marketSkills.map((skill, index) => {
                  const isInstalled = installedSkillNames.has(skill.name);
                  const isDownloading = market.downloadingPackage === skill.installId;
                  return (
                    <SkillCard
                      key={skill.installId}
                      name={skill.name}
                      description={skill.description}
                      index={index}
                      accentSeed={skill.installId}
                      iconKind="market"
                      badges={isInstalled ? (
                        <Badge variant="success">
                          <CheckCircle2 size={11} />
                          {t('market.item.installed')}
                        </Badge>
                      ) : null}
                      meta={(
                        <span className="bitfun-skills-scene__market-meta">
                          <TrendingUp size={12} />
                          {skill.installs ?? 0}
                        </span>
                      )}
                      actions={[
                        {
                          id: 'download',
                          icon: isInstalled ? <CheckCircle2 size={13} /> : <Download size={13} />,
                          ariaLabel: isInstalled ? t('market.item.installed') : t('market.item.downloadProject'),
                          title: isDownloading
                            ? t('market.item.downloading')
                            : (isInstalled ? t('market.item.installedTooltip') : t('market.item.downloadProject')),
                          disabled: isDownloading || !market.hasWorkspace || market.isRemoteWorkspace || isInstalled,
                          tone: isInstalled ? 'success' : 'primary',
                          onClick: () => market.handleDownload(skill),
                        },
                      ]}
                      onOpenDetails={() => setSelectedDetail({ type: 'market', skill })}
                    />
                  );
                })}
              </div>
            )}

            {/* Pagination */}
            {!market.marketLoading && !market.marketError && (market.totalPages > 1 || market.hasMore) && (
              <div className="skills-split__pagination">
                <button
                  type="button"
                  className="skills-split__page-btn"
                  onClick={market.goToPrevPage}
                  disabled={market.currentPage === 0 || market.loadingMore}
                  aria-label={t('market.pagination.prev')}
                >
                  <ChevronLeft size={14} />
                </button>
                <span className="skills-split__page-info">
                  {market.hasMore
                    ? t('market.pagination.infoMore', { current: market.currentPage + 1 })
                    : t('market.pagination.info', { current: market.currentPage + 1, total: market.totalPages })}
                </span>
                <button
                  type="button"
                  className="skills-split__page-btn"
                  onClick={() => void market.goToNextPage()}
                  disabled={(!market.hasMore && market.currentPage >= market.totalPages - 1) || market.loadingMore}
                  aria-label={t('market.pagination.next')}
                >
                  <ChevronRight size={14} />
                </button>
              </div>
            )}
          </div>
        </div>

        {/* ══ RIGHT: installed skills ══ */}
        <div className="skills-split__right">
          <div className="skills-split__right-frame">
            {/* Right header */}
            <div className="skills-split__right-header">
              <span className="skills-split__right-title">{t('installed.titleAll')}</span>
              <Search
                className="skills-split__panel-search skills-split__panel-search--installed"
                value={installedSearch}
                onChange={setInstalledSearch}
                onClear={() => setInstalledSearch('')}
                placeholder={t('page.searchPlaceholder')}
                size="small"
                clearable
              />
            </div>

            <div className="skills-split__right-workbench">
              {/* Category sidebar */}
              <aside className="skills-split__right-sidebar">
                <div className="skills-split__sidebar-scroll">
                  {([
                    ['all', installed.counts.all],
                    ['user', installed.counts.user],
                    ['project', installed.counts.project],
                  ] as const).map(([filter, count]) => (
                    <button
                      key={filter}
                      type="button"
                      className={[
                        'skills-split__sidebar-item',
                        installedFilter === filter && 'is-active',
                      ].filter(Boolean).join(' ')}
                      onClick={() => setInstalledFilter(filter)}
                    >
                      <span>{t(`filters.${filter}`)}</span>
                      <span className="skills-split__sidebar-count">{count}</span>
                    </button>
                  ))}
                </div>
              </aside>

              {/* Scrollable installed list */}
              <div className="skills-split__right-list">
                <div className="skills-split__right-body">
              {/* Loading — row skeletons */}
              {installed.loading && (
                <div className="skills-split__skeleton-list" aria-busy="true" aria-label={t('list.loading')}>
                  {Array.from({ length: 8 }).map((_, i) => (
                    <div
                      key={`ins-sk-${i}`}
                      className="skills-split__skeleton-row"
                      style={{ '--row-index': i } as React.CSSProperties}
                    >
                      <div className="skills-split__skeleton-row-avatar" />
                      <div className="skills-split__skeleton-row-lines">
                        <div className="skills-split__skeleton-line skills-split__skeleton-line--title" />
                        <div className="skills-split__skeleton-line skills-split__skeleton-line--desc" />
                      </div>
                      <div className="skills-split__skeleton-row-tail">
                        <div className="skills-split__skeleton-pill" />
                        <div className="skills-split__skeleton-icon" />
                      </div>
                    </div>
                  ))}
                </div>
              )}

              {/* Error */}
              {!installed.loading && installed.error && (
                <div className="skills-split__empty skills-split__empty--error">
                  <Package size={24} strokeWidth={1.5} />
                  <span>{installed.error}</span>
                </div>
              )}

              {/* Empty */}
              {!installed.loading && !installed.error && installedFiltered.length === 0 && (
                <div className="skills-split__empty">
                  <Sparkles size={24} strokeWidth={1.5} />
                  <span>
                    {installed.skills.length === 0
                      ? t('list.empty.noSkills')
                      : t('list.empty.noMatch')}
                  </span>
                </div>
              )}

              {/* Installed rows */}
              {!installed.loading && !installed.error && pagedInstalledSkills.map((skill, index) => (
                <div
                  key={skill.key}
                  className="skills-split__installed-row"
                  style={{ '--row-index': index } as React.CSSProperties}
                  onClick={() => setSelectedDetail({ type: 'installed', skill })}
                  role="button"
                  tabIndex={0}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.preventDefault();
                      setSelectedDetail({ type: 'installed', skill });
                    }
                  }}
                  aria-label={skill.name}
                >
                  <div className="skills-split__row-icon">
                    <Puzzle size={14} strokeWidth={1.6} />
                  </div>
                  <div className="skills-split__row-body">
                    <span className="skills-split__row-name">{skill.name}</span>
                    {skill.description?.trim() && (
                      <span className="skills-split__row-desc">{skill.description}</span>
                    )}
                  </div>
                  <div
                    className="skills-split__row-end"
                    onClick={(e) => e.stopPropagation()}
                    onKeyDown={(e) => e.stopPropagation()}
                  >
                    <Badge variant={skill.level === 'user' ? 'info' : 'purple'}>
                      {skill.level === 'user' ? t('list.item.user') : t('list.item.project')}
                    </Badge>
                    <button
                      type="button"
                      className="skills-split__row-delete"
                      onClick={() => setDeleteTarget(skill)}
                      aria-label={t('list.item.deleteTooltip')}
                      title={t('list.item.deleteTooltip')}
                    >
                      <Trash2 size={13} />
                    </button>
                  </div>
                </div>
              ))}
                </div>{/* end right-body */}

                {!installed.loading && !installed.error && installedFiltered.length > 0 && installedTotalPages > 1 && (
                  <div className="skills-split__pagination skills-split__pagination--installed">
                    <button
                      type="button"
                      className="skills-split__page-btn"
                      onClick={() => setInstalledListPage((p) => Math.max(0, p - 1))}
                      disabled={currentInstalledPage === 0}
                      aria-label={t('market.pagination.prev')}
                    >
                      <ChevronLeft size={14} />
                    </button>
                    <span className="skills-split__page-info">
                      {t('market.pagination.info', {
                        current: currentInstalledPage + 1,
                        total: installedTotalPages,
                      })}
                    </span>
                    <button
                      type="button"
                      className="skills-split__page-btn"
                      onClick={() => setInstalledListPage((p) => Math.min(installedTotalPages - 1, p + 1))}
                      disabled={currentInstalledPage >= installedTotalPages - 1}
                      aria-label={t('market.pagination.next')}
                    >
                      <ChevronRight size={14} />
                    </button>
                  </div>
                )}
              </div>{/* end right-list */}
            </div>{/* end right-workbench */}
          </div>
        </div>
      </div>

      {/* ── Detail modal ── */}
      <GalleryDetailModal
        isOpen={Boolean(selectedDetail)}
        onClose={() => setSelectedDetail(null)}
        icon={selectedMarketSkill ? <Package size={24} strokeWidth={1.6} /> : <Puzzle size={24} strokeWidth={1.6} />}
        iconGradient={getCardGradient(
          selectedInstalledSkill?.name
          ?? selectedMarketSkill?.installId
          ?? selectedMarketSkill?.name
          ?? 'skill'
        )}
        title={selectedInstalledSkill?.name ?? selectedMarketSkill?.name ?? ''}
        badges={selectedInstalledSkill ? (
          <Badge variant={selectedInstalledSkill.level === 'user' ? 'info' : 'purple'}>
            {selectedInstalledSkill.level === 'user' ? t('list.item.user') : t('list.item.project')}
          </Badge>
        ) : selectedMarketSkill && installedSkillNames.has(selectedMarketSkill.name) ? (
          <Badge variant="success">
            <CheckCircle2 size={11} />
            {t('market.item.installed')}
          </Badge>
        ) : null}
        description={selectedInstalledSkill?.description ?? selectedMarketSkill?.description}
        meta={selectedMarketSkill ? (
          <span className="bitfun-skills-scene__market-meta">
            <TrendingUp size={12} />
            {selectedMarketSkill.installs ?? 0}
          </span>
        ) : null}
        actions={selectedInstalledSkill ? (
          <Button
            variant="danger"
            size="small"
            onClick={() => {
              setDeleteTarget(selectedInstalledSkill);
              setSelectedDetail(null);
            }}
          >
            <Trash2 size={14} />
            {t('deleteModal.delete')}
          </Button>
        ) : selectedMarketSkill ? (
          <Button
            variant={installedSkillNames.has(selectedMarketSkill.name) ? 'secondary' : 'primary'}
            size="small"
            onClick={() => void market.handleDownload(selectedMarketSkill)}
            disabled={
              market.downloadingPackage === selectedMarketSkill.installId
              || !market.hasWorkspace
              || market.isRemoteWorkspace
              || installedSkillNames.has(selectedMarketSkill.name)
            }
          >
            {installedSkillNames.has(selectedMarketSkill.name)
              ? t('market.item.installed')
              : t('market.item.downloadProject')}
          </Button>
        ) : null}
      >
        {selectedInstalledSkill ? (
          <div className="bitfun-skills-scene__detail-row">
            <span className="bitfun-skills-scene__detail-label">{t('list.item.pathLabel')}</span>
            {canRevealSkillPath ? (
              <button
                type="button"
                className="bitfun-skills-scene__detail-path-btn"
                title={t('list.item.openPathInExplorer')}
                onClick={() => void handleRevealSkillPath(selectedInstalledSkill.path)}
              >
                {selectedInstalledSkill.path}
              </button>
            ) : (
              <code className="bitfun-skills-scene__detail-value">{selectedInstalledSkill.path}</code>
            )}
          </div>
        ) : null}

        {selectedMarketSkill?.source ? (
          <div className="bitfun-skills-scene__detail-row">
            <span className="bitfun-skills-scene__detail-label">{t('market.item.sourceLabel')}</span>
            <span className="bitfun-skills-scene__detail-value">{selectedMarketSkill.source}</span>
          </div>
        ) : null}

        {selectedMarketSkill ? (
          <div className="bitfun-skills-scene__detail-row">
            <span className="bitfun-skills-scene__detail-label">{t('market.detail.installsLabel')}</span>
            <span className="bitfun-skills-scene__detail-value">{selectedMarketSkill.installs ?? 0}</span>
          </div>
        ) : null}

        {selectedMarketSkill?.url ? (
          <div className="bitfun-skills-scene__detail-row">
            <span className="bitfun-skills-scene__detail-label">{t('market.detail.linkLabel')}</span>
            <a
              href={selectedMarketSkill.url}
              target="_blank"
              rel="noreferrer"
              className="bitfun-skills-scene__detail-link"
            >
              {selectedMarketSkill.url}
            </a>
          </div>
        ) : null}
      </GalleryDetailModal>

      {/* ── Add skill modal ── */}
      <Modal
        isOpen={isAddFormOpen}
        onClose={() => {
          installed.resetForm();
          setAddFormOpen(false);
        }}
        title={t('form.title')}
        size="small"
      >
        <div className="bitfun-skills-scene__modal-form">
          <Select
            label={t('form.level.label')}
            options={[
              { label: t('form.level.user'), value: 'user' },
              {
                label: `${t('form.level.project')}${installed.hasWorkspace && !installed.isRemoteWorkspace ? '' : t('form.level.projectDisabled')}`,
                value: 'project',
                disabled: !installed.hasWorkspace || installed.isRemoteWorkspace,
              },
            ]}
            value={installed.formLevel}
            onChange={(value) => installed.setFormLevel(value as SkillLevel)}
            size="medium"
          />

          {installed.formLevel === 'project' && installed.hasWorkspace ? (
            <div className="bitfun-skills-scene__form-hint">
              {t('form.level.currentWorkspace', { path: installed.workspacePath })}
            </div>
          ) : null}

          <div className="bitfun-skills-scene__path-input">
            <Input
              label={t('form.path.label')}
              placeholder={t('form.path.placeholder')}
              value={installed.formPath}
              onChange={(e) => installed.setFormPath(e.target.value)}
              variant="outlined"
            />
            <button
              type="button"
              className="gallery-action-btn"
              onClick={installed.handleBrowse}
              aria-label={t('form.path.browseTooltip')}
            >
              <FolderOpen size={15} />
            </button>
          </div>
          <div className="bitfun-skills-scene__path-hint">
            {t('form.path.hint')}
          </div>

          {installed.isValidating ? (
            <div className="bitfun-skills-scene__validating">{t('form.validating')}</div>
          ) : null}

          {installed.validationResult ? (
            <div
              className={[
                'bitfun-skills-scene__validation',
                installed.validationResult.valid ? 'is-valid' : 'is-invalid',
              ].filter(Boolean).join(' ')}
            >
              {installed.validationResult.valid ? (
                <>
                  <div className="bitfun-skills-scene__validation-name">
                    {installed.validationResult.name}
                  </div>
                  <div className="bitfun-skills-scene__validation-desc">
                    {installed.validationResult.description}
                  </div>
                </>
              ) : (
                <div className="bitfun-skills-scene__validation-error">
                  {installed.validationResult.error}
                </div>
              )}
            </div>
          ) : null}

          <div className="bitfun-skills-scene__modal-form-actions">
            <Button
              variant="secondary"
              size="small"
              onClick={() => {
                installed.resetForm();
                setAddFormOpen(false);
              }}
            >
              {t('form.actions.cancel')}
            </Button>
            <Button
              variant="primary"
              size="small"
              onClick={handleAddSkill}
              disabled={!installed.validationResult?.valid || installed.isAdding}
            >
              {installed.isAdding ? t('form.actions.adding') : t('form.actions.add')}
            </Button>
          </div>
        </div>
      </Modal>

      {/* ── Delete confirm ── */}
      <ConfirmDialog
        isOpen={Boolean(deleteTarget)}
        onClose={() => setDeleteTarget(null)}
        onConfirm={async () => {
          if (!deleteTarget) {
            return;
          }
          const deleted = await installed.handleDelete(deleteTarget);
          if (deleted) {
            setDeleteTarget(null);
          }
        }}
        title={t('deleteModal.title')}
        message={t('deleteModal.message', { name: deleteTarget?.name ?? '' })}
        type="warning"
        confirmDanger
        confirmText={t('deleteModal.delete')}
        cancelText={t('deleteModal.cancel')}
      />
    </div>
  );
};

export default SkillsScene;
