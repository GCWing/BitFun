import React, { useCallback, useEffect, useMemo, useState } from 'react';
import {
  ArrowRight,
  CheckCircle2,
  ChevronLeft,
  ChevronRight,
  Download,
  Filter,
  FolderOpen,
  Layers,
  Package,
  Plus,
  Puzzle,
  ShieldAlert,
  ShieldCheck,
  Trash2,
  TrendingUp,
  User,
  Zap,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Badge, Button, ConfirmDialog, Input, Modal, Search, Select } from '@/component-library';
import { GalleryDetailModal } from '@/app/components';
import type { SkillInfo, SkillLevel, SkillMarketItem } from '@/infrastructure/config/types';
import {
  buildSkillCoverageSourceMap,
  canDeleteSkill,
  findSkillByKey,
  getSkillSourceLabel,
} from '@/infrastructure/config/skillSourcePresentation';
import { workspaceAPI } from '@/infrastructure/api';
import { usePeerDeviceModeOptional } from '@/infrastructure/peer-device/PeerDeviceContext';
import { isTauriRuntime } from '@/infrastructure/runtime';
import { workspaceManager } from '@/infrastructure/services/business/workspaceManager';
import { useNotification } from '@/shared/notification-system';
import { isRemoteWorkspace } from '@/shared/types';
import { createLogger } from '@/shared/utils/logger';
import { getCardGradient } from '@/shared/utils/cardGradients';
import { useInstalledSkills } from './hooks/useInstalledSkills';
import { useSkillMarket } from './hooks/useSkillMarket';
import SkillCard from './components/SkillCard';
import SkillsSuiteView from './components/SkillsSuiteView';
import './SkillsScene.scss';
import { useSkillsSceneStore, type InstalledFilter } from './skillsSceneStore';
import { useGallerySceneAutoRefresh } from '@/app/hooks/useGallerySceneAutoRefresh';

const log = createLogger('SkillsScene');

type SkillTab = 'installed' | 'discover';

const INSTALLED_PAGE_SIZE = 12;

interface CategoryInfo {
  id: InstalledFilter;
  icon: React.ReactNode;
  labelKey: string;
  descKey: string;
}

const CATEGORIES: CategoryInfo[] = [
  { id: 'all', icon: <Layers size={15} strokeWidth={1.6} />, labelKey: 'filters.all', descKey: 'categories.all' },
  { id: 'builtin', icon: <ShieldCheck size={15} strokeWidth={1.6} />, labelKey: 'filters.builtin', descKey: 'categories.builtin' },
  { id: 'user', icon: <User size={15} strokeWidth={1.6} />, labelKey: 'filters.user', descKey: 'categories.user' },
  { id: 'project', icon: <FolderOpen size={15} strokeWidth={1.6} />, labelKey: 'filters.project', descKey: 'categories.project' },
  { id: 'suite', icon: <Zap size={15} strokeWidth={1.6} />, labelKey: 'filters.suite', descKey: 'categories.suite' },
];

const SkillsScene: React.FC = () => {
  const { t } = useTranslation('scenes/skills');
  const notification = useNotification();
  const peerDevice = usePeerDeviceModeOptional();
  const remoteConnectionActive = peerDevice?.peerMode.active === true;
  const desktopConfigAvailable = isTauriRuntime() && !remoteConnectionActive;
  const {
    searchDraft,
    marketQuery,
    installedFilter,
    hideDuplicates,
    isAddFormOpen,
    setSearchDraft,
    submitMarketQuery,
    setInstalledFilter,
    setHideDuplicates,
    setAddFormOpen,
    toggleAddForm,
  } = useSkillsSceneStore();

  const [activeTab, setActiveTab] = useState<SkillTab>('installed');
  const [deleteTarget, setDeleteTarget] = useState<SkillInfo | null>(null);
  const [installedListPage, setInstalledListPage] = useState(0);
  const [installedSearch, setInstalledSearch] = useState('');
  const [selectedDetail, setSelectedDetail] = useState<
    | { type: 'installed'; skillKey: string }
    | { type: 'market'; skill: SkillMarketItem }
    | null
  >(null);

  const installed = useInstalledSkills({
    searchQuery: installedSearch,
    activeFilter: installedFilter,
    enabled: desktopConfigAvailable,
  });

  const installedSkillNames = useMemo(
    () => new Set(installed.skills.map((skill) => skill.name)),
    [installed.skills],
  );
  const coverageSourceBySkillKey = useMemo(
    () => buildSkillCoverageSourceMap(installed.skills, t('list.item.unknownSource')),
    [installed.skills, t],
  );
  const selectedInstalledSkill = useMemo(
    () => findSkillByKey(
      installed.skills,
      selectedDetail?.type === 'installed' ? selectedDetail.skillKey : null,
    ),
    [installed.skills, selectedDetail],
  );
  const selectedMarketSkill = selectedDetail?.type === 'market' ? selectedDetail.skill : null;

  useEffect(() => {
    if (selectedDetail?.type === 'installed' && !installed.loading && !selectedInstalledSkill) {
      setSelectedDetail(null);
    }
  }, [installed.loading, selectedDetail, selectedInstalledSkill]);

  useEffect(() => {
    if (desktopConfigAvailable) {
      return;
    }
    setActiveTab('installed');
    setAddFormOpen(false);
    setDeleteTarget(null);
    setSelectedDetail(null);
  }, [desktopConfigAvailable, setAddFormOpen]);

  const market = useSkillMarket({
    searchQuery: marketQuery,
    installedSkillNames,
    pageSize: 15,
    enabled: desktopConfigAvailable,
    onInstalledChanged: async () => {
      await installed.loadSkills(true);
    },
  });
  const installedSkillAriaLabel = useCallback((skill: SkillInfo) => {
    const source = getSkillSourceLabel(skill, t('list.item.unknownSource'));
    const scope = market.isRemoteWorkspace
      ? skill.level === 'user'
        ? t('list.item.localUser')
        : t('list.item.remoteProject')
      : skill.level === 'user'
        ? t('list.item.user')
        : t('list.item.project');
    return [
      skill.name,
      source,
      scope,
      skill.isShadowed
        ? t('list.item.shadowedTooltip', {
            source: coverageSourceBySkillKey.get(skill.key) ?? t('list.item.unknownSource'),
          })
        : null,
    ].filter(Boolean).join('. ');
  }, [coverageSourceBySkillKey, market.isRemoteWorkspace, t]);

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

  const installedFiltered = useMemo(() => {
    const list = hideDuplicates
      ? installed.filteredSkills.filter((s) => !s.isShadowed)
      : installed.filteredSkills;
    return list;
  }, [hideDuplicates, installed.filteredSkills]);

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
  }, [installedFilter, installedSearch, hideDuplicates]);

  useEffect(() => {
    setInstalledListPage((p) => Math.min(p, Math.max(0, installedTotalPages - 1)));
  }, [installedTotalPages]);

  return (
    <div className="bitfun-skills-scene" data-testid="agent-skill-panel">
      <div className="skills-tabs-bar" data-testid="skills-tabs">
        <div className="skills-tabs-bar__tabs">
          <button
            type="button"
            className={`skills-tabs-bar__tab ${activeTab === 'installed' ? 'is-active' : ''}`}
            onClick={() => setActiveTab('installed')}
          ><span>{t('installed.titleAll')}</span></button>
          <span className="skills-tabs-bar__divider" />
          <button
            type="button"
            className={`skills-tabs-bar__tab ${activeTab === 'discover' ? 'is-active' : ''}`}
            disabled={!desktopConfigAvailable}
            onClick={() => setActiveTab('discover')}
          ><span>{t('market.title')}</span></button>
        </div>
      </div>

      <div className="skills-page">

        {activeTab === 'installed' && (
          <div className="skills-installed">
            {desktopConfigAvailable && <aside className="skills-sidebar">
              <div className="skills-sidebar__header">
                <h2 className="skills-sidebar__title">{t('installed.titleAll')}</h2>
              </div>
              <nav className="skills-sidebar__nav">
                {CATEGORIES.map((cat) => {
                  const count = installed.counts[cat.id];
                  const isEmpty = count === 0;
                  return (
                    <button
                      key={cat.id}
                      type="button"
                      className={`skills-sidebar__item ${installedFilter === cat.id ? 'is-active' : ''} ${isEmpty ? 'is-empty' : ''}`}
                      onClick={() => setInstalledFilter(cat.id)}
                    >
                      <span className="skills-sidebar__item-icon">{cat.icon}</span>
                      <span className="skills-sidebar__item-label">{t(cat.labelKey)}</span>
                      <span className="skills-sidebar__item-count">{isEmpty ? '—' : count}</span>
                    </button>
                  );
                })}
              </nav>
              <div className="skills-sidebar__footer">
                <p className="skills-sidebar__hint">
                  {t(CATEGORIES.find((c) => c.id === installedFilter)?.descKey ?? 'categories.all')}
                </p>
              </div>
            </aside>}

            <div className="skills-main">
              {!desktopConfigAvailable ? (
                <div className="skills-main__empty" data-testid="skills-management-unavailable">
                  <Package size={28} strokeWidth={1.2} />
                  <span>{t(remoteConnectionActive ? 'list.remoteUnavailable' : 'list.desktopUnavailable')}</span>
                </div>
              ) : installedFilter === 'suite' ? (
                <SkillsSuiteView />
              ) : (
                <>
                  <div className="skills-main__toolbar">
                    <Search
                      className="skills-main__toolbar-search"
                      value={installedSearch}
                      onChange={setInstalledSearch}
                      onClear={() => setInstalledSearch('')}
                      placeholder={t('toolbar.searchPlaceholder')}
                      size="small"
                      clearable
                    />
                    <button
                      type="button"
                      className={`skills-main__chip-btn${hideDuplicates ? ' is-active' : ''}`}
                      onClick={() => setHideDuplicates(!hideDuplicates)}
                    >
                      <Filter size={13} />
                      <span>{t('toolbar.hideDuplicates')}</span>
                    </button>
                    <button type="button" className="skills-main__add-btn" onClick={toggleAddForm}>
                      <Plus size={13} />
                      <span>{t('toolbar.addTooltip')}</span>
                    </button>
                  </div>

                  {installed.loading && (
                    <div className="skills-main__loading" aria-busy="true" aria-label={t('list.loading')}>
                      {Array.from({ length: 8 }).map((_, i) => (
                        <div
                          key={`ins-sk-${i}`}
                          className="skills-card-skeleton"
                          style={{ '--surface-stagger-index': i } as React.CSSProperties}
                        />
                      ))}
                    </div>
                  )}

                  {!installed.loading && installed.error && (
                    <div className="skills-main__empty skills-main__empty--error">
                      <Package size={28} strokeWidth={1.2} />
                      <span>{t('list.loadFailed')}</span>
                      <Button
                        variant="ghost"
                        size="small"
                        onClick={() => void installed.loadSkills(true)}
                      >
                        {t('list.retry')}
                      </Button>
                    </div>
                  )}

                  {!installed.loading && !installed.error && installedFiltered.length === 0 && (
                    <div className="skills-main__empty" data-testid="skill-list-empty">
                      <Package size={28} strokeWidth={1.2} />
                      <span>
                        {installed.skills.length === 0
                          ? t('list.empty.noSkills')
                          : t('list.empty.noMatch')}
                      </span>
                    </div>
                  )}

                  {!installed.loading && !installed.error && (
                    <>
                      <div className="skills-main__grid" data-testid="skill-list">
                        {pagedInstalledSkills.map((skill, index) => (
                          <div
                            key={skill.key}
                            className={[
                              'skills-card',
                              skill.isShadowed && 'is-shadowed',
                            ].filter(Boolean).join(' ')}
                            style={{ '--surface-stagger-index': index } as React.CSSProperties}
                            onClick={() => setSelectedDetail({ type: 'installed', skillKey: skill.key })}
                            role="button"
                            tabIndex={0}
                            onKeyDown={(e) => {
                              if (e.key === 'Enter' || e.key === ' ') {
                                e.preventDefault();
                                setSelectedDetail({ type: 'installed', skillKey: skill.key });
                              }
                            }}
                            aria-label={installedSkillAriaLabel(skill)}
                            data-testid="skill-list-item"
                            data-skill-key={skill.key}
                            data-skill-id={skill.key}
                            data-skill-name={skill.name}
                            data-skill-level={skill.level}
                            data-skill-builtin={skill.isBuiltin ? 'true' : 'false'}
                          >
                            <div className="skills-card__top">
                              <div className="skills-card__icon">
                                <Puzzle size={18} strokeWidth={1.6} />
                              </div>
                              <div className="skills-card__info">
                                <span className="skills-card__name" data-testid="skill-list-item-title">{skill.name}</span>
                                {skill.description?.trim() && (
                                  <span className="skills-card__desc" data-testid="skill-list-item-description">{skill.description}</span>
                                )}
                              </div>
                              {skill.isBuiltin && (
                                <Badge variant="accent">
                                  <ShieldCheck size={11} />
                                  {t('list.item.builtin')}
                                </Badge>
                              )}
                            </div>

                            <div className="skills-card__meta">
                              <Badge variant="neutral">
                                {getSkillSourceLabel(skill, t('list.item.unknownSource'))}
                              </Badge>
                              <Badge
                                variant={skill.level === 'user' ? 'info' : 'purple'}
                              >
                                {skill.level === 'user'
                                  ? <User size={11} />
                                  : <FolderOpen size={11} />}
                                {market.isRemoteWorkspace
                                  ? skill.level === 'user'
                                    ? t('list.item.localUser')
                                    : t('list.item.remoteProject')
                                  : skill.level === 'user'
                                    ? t('list.item.user')
                                    : t('list.item.project')}
                              </Badge>
                              {skill.isShadowed && (
                                <span title={t('list.item.shadowedTooltip', {
                                  source: coverageSourceBySkillKey.get(skill.key)
                                    ?? t('list.item.unknownSource'),
                                })}>
                                  <Badge variant="warning">
                                    <ShieldAlert size={11} />
                                    {t('list.item.shadowed')}
                                  </Badge>
                                </span>
                              )}
                            </div>

                            <div
                              className="skills-card__actions"
                              onClick={(e) => e.stopPropagation()}
                              onKeyDown={(e) => e.stopPropagation()}
                            >
                              <Button
                                variant="ghost"
                                size="small"
                                onClick={() => setSelectedDetail({ type: 'installed', skillKey: skill.key })}
                              >
                                <span>{t('list.item.detail')}</span>
                                <ArrowRight size={12} />
                              </Button>
                              {canDeleteSkill(skill) && (
                                <button
                                  type="button"
                                  className="skills-card__delete"
                                  onClick={() => setDeleteTarget(skill)}
                                  aria-label={t('list.item.deleteTooltip')}
                                  title={t('list.item.deleteTooltip')}
                                >
                                  <Trash2 size={13} />
                                </button>
                              )}
                            </div>
                          </div>
                        ))}
                      </div>

                      {installedFiltered.length > 0 && installedTotalPages > 1 && (
                        <div className="skills-installed__pagination">
                          <button
                            type="button"
                            className="skills-installed__page-btn"
                            onClick={() => setInstalledListPage((p) => Math.max(0, p - 1))}
                            disabled={currentInstalledPage === 0}
                            aria-label={t('market.pagination.prev')}
                          >
                            <ChevronLeft size={14} />
                          </button>
                          <span className="skills-installed__page-info">
                            {t('market.pagination.info', {
                              current: currentInstalledPage + 1,
                              total: installedTotalPages,
                            })}
                          </span>
                          <button
                            type="button"
                            className="skills-installed__page-btn"
                            onClick={() => setInstalledListPage((p) => Math.min(installedTotalPages - 1, p + 1))}
                            disabled={currentInstalledPage >= installedTotalPages - 1}
                            aria-label={t('market.pagination.next')}
                          >
                            <ChevronRight size={14} />
                          </button>
                        </div>
                      )}
                    </>
                  )}
                </>
              )}
            </div>
          </div>
        )}

        {desktopConfigAvailable && activeTab === 'discover' && (
          <div className="skills-discover">
            <div className="skills-discover__hero">
              <div className="skills-discover__hero-content">
                <h1 className="skills-discover__title">{t('market.title')}</h1>
                <p className="skills-discover__subtitle">
                  {t('market.subtitle')}
                </p>
                <div className="skills-discover__search-wrapper">
                  <Search
                    className="skills-discover__search"
                    value={searchDraft}
                    onChange={setSearchDraft}
                    onSearch={submitMarketQuery}
                    onClear={submitMarketQuery}
                    placeholder={t('market.searchPlaceholder')}
                    size="medium"
                    clearable
                    enterToSearch
                  />
                </div>
              </div>
            </div>

            <div className="skills-discover__content">
              {market.marketLoading && (
                <div className="skills-discover__grid" aria-busy="true" aria-label={t('list.loading')}>
                  {Array.from({ length: 12 }).map((_, i) => (
                    <div
                      key={`mkt-sk-${i}`}
                      className="skills-discover__skeleton-card"
                      style={{ '--surface-stagger-index': i } as React.CSSProperties}
                    />
                  ))}
                </div>
              )}

              {!market.marketLoading && market.marketError && (
                <div className="skills-discover__empty skills-discover__empty--error">
                  <Package size={28} strokeWidth={1.5} />
                  <span>{market.marketError}</span>
                </div>
              )}

              {!market.marketLoading && !market.marketError && market.loadingMore && (
                <div className="skills-discover__grid" aria-busy="true" aria-label={t('list.loading')}>
                  {Array.from({ length: 12 }).map((_, i) => (
                    <div
                      key={`mkt-page-sk-${i}`}
                      className="skills-discover__skeleton-card"
                      style={{ '--surface-stagger-index': i } as React.CSSProperties}
                    />
                  ))}
                </div>
              )}

              {!market.marketLoading && !market.marketError && !market.loadingMore && market.marketSkills.length === 0 && (
                <div className="skills-discover__empty" data-testid="skill-list-empty">
                  <Package size={28} strokeWidth={1.5} />
                  <span>{marketQuery ? t('market.empty.noMatch') : t('market.empty.noSkills')}</span>
                </div>
              )}

              {!market.marketLoading && !market.marketError && !market.loadingMore && market.marketSkills.length > 0 && (
                <>
                  {marketQuery && (
                    <div className="skills-discover__results-info">
                      <span>
                        {t('market.resultsInfo', { query: marketQuery, count: market.totalLoaded })}
                      </span>
                    </div>
                  )}

                  <div className="skills-discover__grid" data-testid="skill-list">
                    {market.marketSkills.map((skill, index) => {
                      const isInstalled = installedSkillNames.has(skill.name);
                      const isDownloading = market.downloadingPackage === skill.installId;
                      return (
                        <SkillCard
                          key={skill.installId}
                          data-testid="skills-market-card"
                          data-skill-install-id={skill.installId}
                          data-skill-id={skill.installId}
                          data-skill-name={skill.name}
                          data-skill-installed={isInstalled ? 'true' : 'false'}
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
                              disabled:
                                isDownloading
                                || !market.hasWorkspace
                                || market.isRemoteWorkspace
                                || isInstalled,
                              tone: isInstalled ? 'success' : 'primary',
                              onClick: () => void market.handleDownload(skill, 'project'),
                            },
                          ]}
                          onOpenDetails={() => setSelectedDetail({ type: 'market', skill })}
                        />
                      );
                    })}
                  </div>

                  {(market.totalPages > 1 || market.hasMore) && (
                    <div className="skills-discover__pagination">
                      <button
                        type="button"
                        className="skills-discover__page-btn"
                        onClick={market.goToPrevPage}
                        disabled={market.currentPage === 0 || market.loadingMore}
                        aria-label={t('market.pagination.prev')}
                      >
                        <ChevronLeft size={14} />
                      </button>
                      <span className="skills-discover__page-info">
                        {market.hasMore
                          ? t('market.pagination.infoMore', { current: market.currentPage + 1 })
                          : t('market.pagination.info', { current: market.currentPage + 1, total: market.totalPages })}
                      </span>
                      <button
                        type="button"
                        className="skills-discover__page-btn"
                        onClick={() => void market.goToNextPage()}
                        disabled={(!market.hasMore && market.currentPage >= market.totalPages - 1) || market.loadingMore}
                        aria-label={t('market.pagination.next')}
                      >
                        <ChevronRight size={14} />
                      </button>
                    </div>
                  )}
                </>
              )}
            </div>
          </div>
        )}
      </div>

      <GalleryDetailModal
        isOpen={desktopConfigAvailable && Boolean(selectedDetail)}
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
          <>
            {selectedInstalledSkill.isShadowed && (
              <span title={t('list.item.shadowedTooltip', {
                source: coverageSourceBySkillKey.get(selectedInstalledSkill.key)
                  ?? t('list.item.unknownSource'),
              })}>
                <Badge variant="warning">
                  <ShieldAlert size={11} />
                  {t('list.item.shadowed')}
                </Badge>
              </span>
            )}
            <Badge variant="neutral">
              {getSkillSourceLabel(selectedInstalledSkill, t('list.item.unknownSource'))}
            </Badge>
            <Badge variant={selectedInstalledSkill.isBuiltin ? 'accent' : 'success'}>
              {selectedInstalledSkill.isBuiltin ? t('list.item.builtin') : t('list.item.userInstalled')}
            </Badge>
            <Badge variant={selectedInstalledSkill.level === 'user' ? 'info' : 'purple'}>
              {market.isRemoteWorkspace
                ? selectedInstalledSkill.level === 'user'
                  ? t('list.item.localUser')
                  : t('list.item.remoteProject')
                : selectedInstalledSkill.level === 'user'
                  ? t('list.item.user')
                  : t('list.item.project')}
            </Badge>
          </>
        ) : selectedMarketSkill && installedSkillNames.has(selectedMarketSkill.name) ? (
          <Badge variant="success">
            <CheckCircle2 size={11} />
            {t('market.item.installed')}
          </Badge>
        ) : null}
        description={selectedInstalledSkill?.description ?? selectedMarketSkill?.description}
        testId="skill-detail-panel"
        titleTestId="skill-detail-title"
        descriptionTestId="skill-detail-description"
        closeButtonTestId="skill-detail-close"
        meta={selectedMarketSkill ? (
          <span className="bitfun-skills-scene__market-meta">
            <TrendingUp size={12} />
            {selectedMarketSkill.installs ?? 0}
          </span>
        ) : null}
        actions={selectedInstalledSkill && canDeleteSkill(selectedInstalledSkill) ? (
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
          <>
            {installedSkillNames.has(selectedMarketSkill.name) ? (
              <Button variant="secondary" size="small" disabled>
                {t('market.item.installed')}
              </Button>
            ) : (
              <>
                {!market.isRemoteWorkspace && (
                  <Button
                    variant="primary"
                    size="small"
                    onClick={() => void market.handleDownload(selectedMarketSkill, 'project')}
                    disabled={market.downloadingPackage === selectedMarketSkill.installId || !market.hasWorkspace}
                  >
                    {t('market.item.downloadProject')}
                  </Button>
                )}
                <Button
                  variant={market.isRemoteWorkspace ? 'primary' : 'secondary'}
                  size="small"
                  onClick={() => void market.handleDownload(selectedMarketSkill, 'user')}
                  disabled={market.downloadingPackage === selectedMarketSkill.installId}
                >
                  {t('market.item.downloadUser')}
                </Button>
              </>
            )}
          </>
        ) : null}
      >
        {selectedInstalledSkill ? (
          <>
            <div className="bitfun-skills-scene__detail-row">
              <span className="bitfun-skills-scene__detail-label">{t('list.item.sourceLabel')}</span>
              <span className="bitfun-skills-scene__detail-value">
                {getSkillSourceLabel(selectedInstalledSkill, t('list.item.unknownSource'))}
              </span>
            </div>
            {selectedInstalledSkill.isShadowed && (
              <div className="bitfun-skills-scene__detail-row">
                <span className="bitfun-skills-scene__detail-label">{t('list.item.shadowedLabel')}</span>
                <span className="bitfun-skills-scene__detail-value">
                  {t('list.item.shadowedDetail', {
                    source: coverageSourceBySkillKey.get(selectedInstalledSkill.key)
                      ?? t('list.item.unknownSource'),
                  })}
                </span>
              </div>
            )}
            <div className="bitfun-skills-scene__detail-row" data-testid="skill-detail-capabilities-section">
              <span className="bitfun-skills-scene__detail-label">{t('list.item.pathLabel')}</span>
              {canRevealSkillPath ? (
                <button
                  type="button"
                  className="bitfun-skills-scene__detail-path-btn"
                  title={t('list.item.openPathInExplorer')}
                  onClick={() => void handleRevealSkillPath(selectedInstalledSkill.path)}
                  data-testid="skills-detail-path-btn"
                >
                  {selectedInstalledSkill.path}
                </button>
              ) : (
                <code className="bitfun-skills-scene__detail-value">{selectedInstalledSkill.path}</code>
              )}
            </div>
          </>
        ) : null}

        {selectedMarketSkill?.source ? (
          <div className="bitfun-skills-scene__detail-row" data-testid="skill-detail-capabilities-section">
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
              data-testid="skills-detail-external-link"
            >
              {selectedMarketSkill.url}
            </a>
          </div>
        ) : null}
      </GalleryDetailModal>

      <Modal
        isOpen={desktopConfigAvailable && isAddFormOpen}
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
              {t('form.level.selectedProjectPath', { path: installed.workspacePath })}
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

      <ConfirmDialog
        isOpen={desktopConfigAvailable && Boolean(deleteTarget)}
        onClose={() => setDeleteTarget(null)}
        onConfirm={async () => {
          if (!desktopConfigAvailable || !deleteTarget || !canDeleteSkill(deleteTarget)) {
            setDeleteTarget(null);
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
