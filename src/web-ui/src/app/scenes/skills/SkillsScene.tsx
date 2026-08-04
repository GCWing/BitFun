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
import { Badge, Button, ConfirmDialog, Input, Modal, Search, Select, Switch } from '@/component-library';
import { GalleryDetailModal } from '@/app/components';
import type { SkillInfo, SkillLevel, SkillMarketItem } from '@/infrastructure/config/types';
import {
  buildSkillCoverageSourceMap,
  canDeleteSkill,
  findSkillByKey,
  getSkillSourceLabel,
} from '@/infrastructure/config/skillSourcePresentation';
import { workspaceAPI } from '@/infrastructure/api';
import { usePeerDeviceModeOptional } from '@/infrastructure/peer-device/peerDeviceContextState';
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
      skill.level === 'user'
        ? installed.globallyDisabledSkillKeys.has(skill.key)
          ? t('list.item.globalDisabled')
          : t('list.item.globalEnabled')
        : null,
      skill.isShadowed
        ? t('list.item.shadowedTooltip', {
            source: coverageSourceBySkillKey.get(skill.key) ?? t('list.item.unknownSource'),
          })
        : null,
    ].filter(Boolean).join('. ');
  }, [coverageSourceBySkillKey, installed.globallyDisabledSkillKeys, market.isRemoteWorkspace, t]);

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
    <div className="bitfun-skills-scene" data-testid="agent-skill-panel" data-bf-scene="skills" data-bf-part="root" data-bf-tab={activeTab}>
      <div className="skills-tabs-bar" data-testid="skills-tabs" data-bf-scene="skills" data-bf-part="header" data-bf-tab={activeTab}>
        <div className="skills-tabs-bar__tabs" data-bf-scene="skills" data-bf-part="tabs">
          <button
            type="button"
            className={`skills-tabs-bar__tab ${activeTab === 'installed' ? 'is-active' : ''}`}
            onClick={() => setActiveTab('installed')}
            data-bf-scene="skills"
            data-bf-part="tab"
            data-bf-tab="installed"
            data-bf-state={activeTab === 'installed' ? 'active' : undefined}
          ><span>{t('installed.titleAll')}</span></button>
          <span className="skills-tabs-bar__divider" data-bf-scene="skills" data-bf-part="tabDivider" />
          <button
            type="button"
            className={`skills-tabs-bar__tab ${activeTab === 'discover' ? 'is-active' : ''}`}
            disabled={!desktopConfigAvailable}
            onClick={() => setActiveTab('discover')}
            data-bf-scene="skills"
            data-bf-part="tab"
            data-bf-tab="discover"
            data-bf-state={activeTab === 'discover' ? 'active' : undefined}
          ><span>{t('market.title')}</span></button>
        </div>
      </div>

      <div className="skills-page" data-bf-scene="skills" data-bf-part="content" data-bf-tab={activeTab}>

        {activeTab === 'installed' && (
          <div className="skills-installed" data-bf-scene="skills" data-bf-part="installed">
            {desktopConfigAvailable && <aside className="skills-sidebar" data-bf-scene="skills" data-bf-part="sidebar">
              <div className="skills-sidebar__header" data-bf-scene="skills" data-bf-part="sidebarHeader">
                <h2 className="skills-sidebar__title" data-bf-scene="skills" data-bf-part="sidebarTitle">{t('installed.titleAll')}</h2>
              </div>
              <nav className="skills-sidebar__nav" data-bf-scene="skills" data-bf-part="sidebarNav">
                {CATEGORIES.map((cat) => {
                  const count = installed.counts[cat.id];
                  const isEmpty = count === 0;
                  return (
                    <button
                      key={cat.id}
                      type="button"
                      className={`skills-sidebar__item ${installedFilter === cat.id ? 'is-active' : ''} ${isEmpty ? 'is-empty' : ''}`}
                      onClick={() => setInstalledFilter(cat.id)}
                      data-bf-scene="skills"
                      data-bf-part="sidebarItem"
                      data-bf-category={cat.id}
                      data-bf-state={[
                        installedFilter === cat.id && 'active',
                        isEmpty && 'empty',
                      ].filter(Boolean).join(' ') || undefined}
                    >
                      <span className="skills-sidebar__item-icon" data-bf-scene="skills" data-bf-part="sidebarItemIcon">{cat.icon}</span>
                      <span className="skills-sidebar__item-label" data-bf-scene="skills" data-bf-part="sidebarItemLabel">{t(cat.labelKey)}</span>
                      <span className="skills-sidebar__item-count" data-bf-scene="skills" data-bf-part="sidebarItemCount">{isEmpty ? '—' : count}</span>
                    </button>
                  );
                })}
              </nav>
              <div className="skills-sidebar__footer" data-bf-scene="skills" data-bf-part="sidebarFooter">
                <p className="skills-sidebar__hint" data-bf-scene="skills" data-bf-part="sidebarHint">
                  {t(CATEGORIES.find((c) => c.id === installedFilter)?.descKey ?? 'categories.all')}
                </p>
              </div>
            </aside>}

            <div className="skills-main" data-bf-scene="skills" data-bf-part="main">
              {!desktopConfigAvailable ? (
                <div className="skills-main__empty" data-testid="skills-management-unavailable" data-bf-scene="skills" data-bf-part="empty">
                  <Package size={28} strokeWidth={1.2} />
                  <span>{t(remoteConnectionActive ? 'list.remoteUnavailable' : 'list.desktopUnavailable')}</span>
                </div>
              ) : installedFilter === 'suite' ? (
                <SkillsSuiteView />
              ) : (
                <>
                  <div className="skills-main__toolbar" data-bf-scene="skills" data-bf-part="toolbar">
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
                      data-bf-scene="skills"
                      data-bf-part="filterAction"
                      data-bf-state={hideDuplicates ? 'active' : undefined}
                    >
                      <Filter size={13} />
                      <span>{t('toolbar.hideDuplicates')}</span>
                    </button>
                    <button
                      type="button"
                      className="skills-main__add-btn"
                      onClick={toggleAddForm}
                      data-bf-scene="skills"
                      data-bf-part="addAction"
                    >
                      <Plus size={13} />
                      <span>{t('toolbar.addTooltip')}</span>
                    </button>
                  </div>

                  {installed.loading && (
                    <div className="skills-main__loading" aria-busy="true" aria-label={t('list.loading')} data-bf-scene="skills" data-bf-part="loading">
                      {Array.from({ length: 8 }).map((_, i) => (
                        <div
                          key={`ins-sk-${i}`}
                          className="skills-card-skeleton"
                          style={{ '--surface-stagger-index': i } as React.CSSProperties}
                          data-bf-scene="skills"
                          data-bf-part="skeleton"
                        />
                      ))}
                    </div>
                  )}

                  {!installed.loading && installed.error && (
                    <div className="skills-main__empty skills-main__empty--error" data-bf-scene="skills" data-bf-part="error">
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
                    <div className="skills-main__empty" data-testid="skill-list-empty" data-bf-scene="skills" data-bf-part="empty">
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
                      <div className="skills-main__grid" data-testid="skill-list" data-bf-scene="skills" data-bf-part="list">
                        {pagedInstalledSkills.map((skill, index) => (
                          <div
                            key={skill.key}
                            className={[
                              'skills-card',
                              skill.isShadowed && 'is-shadowed',
                              skill.level === 'user'
                                && installed.globallyDisabledSkillKeys.has(skill.key)
                                && 'is-globally-disabled',
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
                            data-bf-scene="skills"
                            data-bf-part="installedCard"
                            data-bf-level={skill.level}
                            data-bf-state={[
                              skill.isShadowed && 'shadowed',
                              skill.isBuiltin && 'builtin',
                            ].filter(Boolean).join(' ') || undefined}
                          >
                            <div className="skills-card__top" data-bf-scene="skills" data-bf-part="installedCardTop">
                              <div className="skills-card__icon" data-bf-scene="skills" data-bf-part="installedCardIcon">
                                <Puzzle size={18} strokeWidth={1.6} />
                              </div>
                              <div className="skills-card__info" data-bf-scene="skills" data-bf-part="installedCardInfo">
                                <span className="skills-card__name" data-testid="skill-list-item-title" data-bf-scene="skills" data-bf-part="installedCardName">{skill.name}</span>
                                {skill.description?.trim() && (
                                  <span className="skills-card__desc" data-testid="skill-list-item-description" data-bf-scene="skills" data-bf-part="installedCardDescription">{skill.description}</span>
                                )}
                              </div>
                              <div className="skills-card__status-badges">
                                {skill.isBuiltin && (
                                  <Badge variant="accent">
                                    <ShieldCheck size={11} />
                                    {t('list.item.builtin')}
                                  </Badge>
                                )}
                                {skill.level === 'user'
                                  && installed.globallyDisabledSkillKeys.has(skill.key) && (
                                  <Badge variant="neutral">
                                    {t('list.item.globalDisabled')}
                                  </Badge>
                                )}
                              </div>
                            </div>

                            <div className="skills-card__meta" data-bf-scene="skills" data-bf-part="installedCardMeta">
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
                              data-bf-scene="skills"
                              data-bf-part="installedCardActions"
                            >
                              {skill.level === 'user' && (
                                <div className="skills-card__global-toggle">
                                  <Switch
                                    size="small"
                                    checked={!installed.globallyDisabledSkillKeys.has(skill.key)}
                                    loading={installed.savingGlobalSkillKey === skill.key}
                                    disabled={installed.savingGlobalSkillKey !== null}
                                    aria-label={t('list.item.globalToggleLabel', { name: skill.name })}
                                    onChange={(event) => {
                                      void installed.handleGlobalSkillToggle(skill, event.target.checked);
                                    }}
                                  />
                                </div>
                              )}
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
                                  data-bf-scene="skills"
                                  data-bf-part="installedCardDelete"
                                >
                                  <Trash2 size={13} />
                                </button>
                              )}
                            </div>
                          </div>
                        ))}
                      </div>

                      {installedFiltered.length > 0 && installedTotalPages > 1 && (
                        <div className="skills-installed__pagination" data-bf-scene="skills" data-bf-part="pagination">
                          <button
                            type="button"
                            className="skills-installed__page-btn"
                            onClick={() => setInstalledListPage((p) => Math.max(0, p - 1))}
                            disabled={currentInstalledPage === 0}
                            aria-label={t('market.pagination.prev')}
                            data-bf-scene="skills"
                            data-bf-part="pageButton"
                          >
                            <ChevronLeft size={14} />
                          </button>
                          <span className="skills-installed__page-info" data-bf-scene="skills" data-bf-part="pageInfo">
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
                            data-bf-scene="skills"
                            data-bf-part="pageButton"
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
          <div className="skills-discover" data-bf-scene="skills" data-bf-part="discover">
            <div className="skills-discover__hero" data-bf-scene="skills" data-bf-part="discoverHero">
              <div className="skills-discover__hero-content" data-bf-scene="skills" data-bf-part="discoverHeroContent">
                <h1 className="skills-discover__title" data-bf-scene="skills" data-bf-part="discoverTitle">{t('market.title')}</h1>
                <p className="skills-discover__subtitle" data-bf-scene="skills" data-bf-part="discoverSubtitle">
                  {t('market.subtitle')}
                </p>
                <div className="skills-discover__search-wrapper" data-bf-scene="skills" data-bf-part="discoverSearch">
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

            <div className="skills-discover__content" data-bf-scene="skills" data-bf-part="discoverContent">
              {market.marketLoading && (
                <div className="skills-discover__grid" aria-busy="true" aria-label={t('list.loading')} data-bf-scene="skills" data-bf-part="loading">
                  {Array.from({ length: 12 }).map((_, i) => (
                    <div
                      key={`mkt-sk-${i}`}
                      className="skills-discover__skeleton-card"
                      style={{ '--surface-stagger-index': i } as React.CSSProperties}
                      data-bf-scene="skills"
                      data-bf-part="skeleton"
                    />
                  ))}
                </div>
              )}

              {!market.marketLoading && market.marketError && (
                <div className="skills-discover__empty skills-discover__empty--error" data-bf-scene="skills" data-bf-part="error">
                  <Package size={28} strokeWidth={1.5} />
                  <span>{market.marketError}</span>
                </div>
              )}

              {!market.marketLoading && !market.marketError && market.loadingMore && (
                <div className="skills-discover__grid" aria-busy="true" aria-label={t('list.loading')} data-bf-scene="skills" data-bf-part="loading">
                  {Array.from({ length: 12 }).map((_, i) => (
                    <div
                      key={`mkt-page-sk-${i}`}
                      className="skills-discover__skeleton-card"
                      style={{ '--surface-stagger-index': i } as React.CSSProperties}
                      data-bf-scene="skills"
                      data-bf-part="skeleton"
                    />
                  ))}
                </div>
              )}

              {!market.marketLoading && !market.marketError && !market.loadingMore && market.marketSkills.length === 0 && (
                <div className="skills-discover__empty" data-testid="skill-list-empty" data-bf-scene="skills" data-bf-part="empty">
                  <Package size={28} strokeWidth={1.5} />
                  <span>{marketQuery ? t('market.empty.noMatch') : t('market.empty.noSkills')}</span>
                </div>
              )}

              {!market.marketLoading && !market.marketError && !market.loadingMore && market.marketSkills.length > 0 && (
                <>
                  {marketQuery && (
                    <div className="skills-discover__results-info" data-bf-scene="skills" data-bf-part="resultsInfo">
                      <span>
                        {t('market.resultsInfo', { query: marketQuery, count: market.totalLoaded })}
                      </span>
                    </div>
                  )}

                  <div className="skills-discover__grid" data-testid="skill-list" data-bf-scene="skills" data-bf-part="list">
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
                    <div className="skills-discover__pagination" data-bf-scene="skills" data-bf-part="pagination">
                      <button
                        type="button"
                        className="skills-discover__page-btn"
                        onClick={market.goToPrevPage}
                        disabled={market.currentPage === 0 || market.loadingMore}
                        aria-label={t('market.pagination.prev')}
                        data-bf-scene="skills"
                        data-bf-part="pageButton"
                      >
                        <ChevronLeft size={14} />
                      </button>
                      <span className="skills-discover__page-info" data-bf-scene="skills" data-bf-part="pageInfo">
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
                        data-bf-scene="skills"
                        data-bf-part="pageButton"
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
