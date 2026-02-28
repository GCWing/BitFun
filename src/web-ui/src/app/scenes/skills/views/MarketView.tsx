/**
 * MarketView — skill marketplace browser.
 * List layout with prev/next pagination.
 */

import React, { useState, useEffect, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { RefreshCw, Download, CheckCircle2, TrendingUp, Store, ChevronLeft, ChevronRight, Search as SearchIcon } from 'lucide-react';
import { Search, Button, IconButton, Tooltip, Badge } from '@/component-library';
import { configAPI } from '@/infrastructure/api';
import { useCurrentWorkspace } from '@/infrastructure/hooks/useWorkspace';
import { useNotification } from '@/shared/notification-system';
import type { SkillMarketItem, SkillInfo } from '@/infrastructure/config/types';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('SkillsScene:MarketView');

const PAGE_SIZE = 10;

const MarketView: React.FC = () => {
  const { t } = useTranslation('scenes/skills');
  const [marketKeyword, setMarketKeyword] = useState('');
  const [marketSkills, setMarketSkills] = useState<SkillMarketItem[]>([]);
  const [marketLoading, setMarketLoading] = useState(false);
  const [marketError, setMarketError] = useState<string | null>(null);
  const [downloadingPackage, setDownloadingPackage] = useState<string | null>(null);
  const [installedSkills, setInstalledSkills] = useState<SkillInfo[]>([]);
  const [currentPage, setCurrentPage] = useState(0);
  const [expandedItems, setExpandedItems] = useState<Set<string>>(new Set());

  const toggleExpand = useCallback((installId: string) => {
    setExpandedItems((prev) => {
      const next = new Set(prev);
      if (next.has(installId)) next.delete(installId);
      else next.add(installId);
      return next;
    });
  }, []);

  const { hasWorkspace } = useCurrentWorkspace();
  const notification = useNotification();

  const loadMarketSkills = useCallback(async (query?: string) => {
    try {
      setMarketLoading(true);
      setMarketError(null);
      const normalized = query?.trim();
      const skillList = normalized
        ? await configAPI.searchSkillMarket(normalized, 50)
        : await configAPI.listSkillMarket(undefined, 50);
      setMarketSkills(skillList);
    } catch (err) {
      log.error('Failed to load skill market', err);
      setMarketError(err instanceof Error ? err.message : String(err));
    } finally {
      setMarketLoading(false);
    }
  }, []);

  const loadInstalledSkills = useCallback(async () => {
    try {
      const list = await configAPI.getSkillConfigs();
      setInstalledSkills(list);
    } catch {
      // silent
    }
  }, []);

  useEffect(() => {
    loadMarketSkills();
    loadInstalledSkills();
  }, [loadMarketSkills, loadInstalledSkills]);

  // Reset to first page whenever results change
  useEffect(() => {
    setCurrentPage(0);
  }, [marketSkills]);


  const handleDownload = async (skill: SkillMarketItem) => {
    if (!hasWorkspace) {
      notification.warning(t('messages.noWorkspace'));
      return;
    }
    try {
      setDownloadingPackage(skill.installId);
      const result = await configAPI.downloadSkillMarket(skill.installId, 'project');
      const installedName = result.installedSkills[0] ?? skill.name;
      notification.success(t('messages.marketDownloadSuccess', { name: installedName }));
      await loadInstalledSkills();
    } catch (err) {
      notification.error(t('messages.marketDownloadFailed', { error: err instanceof Error ? err.message : String(err) }));
    } finally {
      setDownloadingPackage(null);
    }
  };

  const handleMarketSearch = useCallback(() => {
    loadMarketSkills(marketKeyword);
  }, [loadMarketSkills, marketKeyword]);

  const installedSkillNames = useMemo(
    () => new Set(installedSkills.map((s) => s.name)),
    [installedSkills]
  );

  const displayMarketSkills = useMemo(() => {
    const entries = marketSkills.map((skill, index) => ({
      skill,
      index,
      installed: installedSkillNames.has(skill.name),
    }));
    entries.sort((a, b) => {
      if (a.installed !== b.installed) return a.installed ? -1 : 1;
      const installDelta = (b.skill.installs ?? 0) - (a.skill.installs ?? 0);
      if (installDelta !== 0) return installDelta;
      return a.index - b.index;
    });
    return entries.map((e) => e.skill);
  }, [marketSkills, installedSkillNames]);

  const totalPages = Math.max(1, Math.ceil(displayMarketSkills.length / PAGE_SIZE));
  const paginatedSkills = displayMarketSkills.slice(
    currentPage * PAGE_SIZE,
    (currentPage + 1) * PAGE_SIZE
  );

  const renderSkeletonList = () => (
    <div className="bitfun-market__list" aria-busy="true">
      {Array.from({ length: 8 }).map((_, i) => (
        <div
          key={i}
          className="bitfun-market__list-item bitfun-market__list-item--skeleton"
          style={{ '--item-index': i } as React.CSSProperties}
        >
          <div className="bitfun-market__list-item-row">
            <div className="bitfun-market__list-item-info">
              <div className="bitfun-market__skeleton-line bitfun-market__skeleton-line--title" />
              <div className="bitfun-market__skeleton-line bitfun-market__skeleton-line--body" />
            </div>
            <div className="bitfun-market__list-item-meta">
              <div className="bitfun-market__skeleton-chip bitfun-market__skeleton-chip--sm" />
              <div className="bitfun-market__skeleton-chip" />
            </div>
            <div className="bitfun-market__list-item-action">
              <div className="bitfun-market__skeleton-btn" />
            </div>
          </div>
        </div>
      ))}
    </div>
  );

  const renderContent = () => {
    if (marketLoading) {
      return renderSkeletonList();
    }

    if (marketError) {
      return (
        <div className="bitfun-market__empty bitfun-market__empty--error">
          <Store size={32} strokeWidth={1.5} />
          <span>{t('market.errorPrefix')}{marketError}</span>
        </div>
      );
    }

    if (displayMarketSkills.length === 0) {
      return (
        <div className="bitfun-market__empty">
          <Store size={32} strokeWidth={1.5} />
          <span>{marketKeyword.trim() ? t('market.empty.noMatch') : t('market.empty.noSkills')}</span>
        </div>
      );
    }

    return (
      <>
        <div className="bitfun-market__list">
          {paginatedSkills.map((skill, index) => {
            const isDownloading = downloadingPackage === skill.installId;
            const isInstalled = installedSkillNames.has(skill.name);
            const isExpanded = expandedItems.has(skill.installId);
            const tooltipText = !hasWorkspace
              ? t('messages.noWorkspace')
              : isInstalled
                ? t('market.item.installedTooltip')
                : t('market.item.downloadProject');

            return (
              <div
                key={skill.installId}
                className={[
                  'bitfun-market__list-item',
                  isInstalled && 'is-installed',
                  isExpanded && 'is-expanded',
                ].filter(Boolean).join(' ')}
                style={{ '--item-index': index } as React.CSSProperties}
              >
                <div
                  className="bitfun-market__list-item-row"
                  onClick={() => toggleExpand(skill.installId)}
                  role="button"
                  tabIndex={0}
                  onKeyDown={(e) => e.key === 'Enter' && toggleExpand(skill.installId)}
                >
                  <div className="bitfun-market__list-item-info">
                    <div className="bitfun-market__card-name-row">
                      <span className="bitfun-market__card-name">{skill.name}</span>
                      {isInstalled && (
                        <Badge variant="success">
                          <CheckCircle2 size={11} />
                          {t('market.item.installed')}
                        </Badge>
                      )}
                    </div>
                    <p className="bitfun-market__list-item-desc">
                      {skill.description?.trim() || t('market.item.noDescription')}
                    </p>
                  </div>

                  <div className="bitfun-market__list-item-meta">
                    <span className="bitfun-market__installs">
                      <TrendingUp size={11} />
                      {skill.installs ?? 0}
                    </span>
                  </div>

                  <div className="bitfun-market__list-item-action" onClick={(e) => e.stopPropagation()}>
                    <Tooltip content={tooltipText}>
                      <span>
                        <Button
                          variant={isInstalled ? 'secondary' : 'primary'}
                          size="small"
                          onClick={() => handleDownload(skill)}
                          disabled={isDownloading || !hasWorkspace || isInstalled}
                        >
                          {isInstalled ? (
                            <CheckCircle2 size={13} />
                          ) : (
                            <Download size={13} />
                          )}
                          {isDownloading
                            ? t('market.item.downloading')
                            : isInstalled
                              ? t('market.item.installed')
                              : t('market.item.downloadProject')}
                        </Button>
                      </span>
                    </Tooltip>
                  </div>

                </div>

                {isExpanded && (
                  <div className="bitfun-market__list-item-details">
                    {skill.description?.trim() && (
                      <p className="bitfun-market__detail-desc">
                        {skill.description.trim()}
                      </p>
                    )}
                    {skill.source && (
                      <div className="bitfun-market__detail-row">
                        <span className="bitfun-market__detail-label">{t('market.item.sourceLabel')}</span>
                        <span className="bitfun-market__detail-value">{skill.source}</span>
                      </div>
                    )}
                    <div className="bitfun-market__detail-row">
                      <span className="bitfun-market__detail-label">{t('market.item.installs', { count: skill.installs ?? 0 })}</span>
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>

        {totalPages > 1 && (
          <div className="bitfun-market__pagination">
            <IconButton
              variant="ghost"
              size="small"
              onClick={() => setCurrentPage((p) => p - 1)}
              disabled={currentPage === 0}
              tooltip={t('market.pagination.prev')}
            >
              <ChevronLeft size={16} />
            </IconButton>
            <span className="bitfun-market__pagination-info">
              {t('market.pagination.info', { current: currentPage + 1, total: totalPages })}
            </span>
            <IconButton
              variant="ghost"
              size="small"
              onClick={() => setCurrentPage((p) => p + 1)}
              disabled={currentPage >= totalPages - 1}
              tooltip={t('market.pagination.next')}
            >
              <ChevronRight size={16} />
            </IconButton>
          </div>
        )}
      </>
    );
  };

  return (
    <div className="bitfun-skills-scene__view">
      <div className="bitfun-skills-scene__view-header">
        <div className="bitfun-skills-scene__view-header-inner">
          <div className="bitfun-skills-scene__view-title-row">
            <div>
              <h2 className="bitfun-skills-scene__view-title">{t('market.title')}</h2>
              <p className="bitfun-skills-scene__view-subtitle">{t('market.subtitle')}</p>
            </div>
            <IconButton
              variant="ghost"
              size="small"
              onClick={() => loadMarketSkills(marketKeyword)}
              tooltip={t('market.refreshTooltip')}
            >
              <RefreshCw size={16} />
            </IconButton>
          </div>
          <div className="bitfun-skills-scene__market-toolbar">
            <Search
              placeholder={t('market.searchPlaceholder')}
              value={marketKeyword}
              onChange={(value) => setMarketKeyword(value)}
              onSearch={handleMarketSearch}
              clearable
              size="small"
              prefixIcon={<></>}
              suffixContent={
                <button
                  type="button"
                  className="bitfun-market__search-icon-btn"
                  onClick={handleMarketSearch}
                  aria-label={t('market.searchPlaceholder')}
                >
                  <SearchIcon size={14} />
                </button>
              }
            />
          </div>
        </div>
      </div>
      <div className="bitfun-skills-scene__view-content">
        <div className="bitfun-skills-scene__view-content-inner">
          {renderContent()}
        </div>
      </div>
    </div>
  );
};

export default MarketView;
