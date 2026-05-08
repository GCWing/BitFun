import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { configAPI } from '@/infrastructure/api';
import type { SkillLevel, SkillMarketItem } from '@/infrastructure/config/types';
import { useWorkspaceManagerSync } from '@/infrastructure/hooks/useWorkspaceManagerSync';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('SkillsScene:useSkillMarket');

const DEFAULT_PAGE_SIZE = 10;
const MAX_TOTAL_SKILLS = 500;

interface UseSkillMarketOptions {
  searchQuery: string;
  installedSkillNames: Set<string>;
  onInstalledChanged?: () => Promise<void> | void;
  pageSize?: number;
}

export function useSkillMarket({
  searchQuery,
  installedSkillNames,
  onInstalledChanged,
  pageSize = DEFAULT_PAGE_SIZE,
}: UseSkillMarketOptions) {
  const { t } = useTranslation('scenes/skills');
  const notification = useNotification();
  const { hasWorkspace, workspacePath, isRemoteWorkspace } = useWorkspaceManagerSync();

  const [marketSkills, setMarketSkills] = useState<SkillMarketItem[]>([]);
  const [marketLoading, setMarketLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [marketError, setMarketError] = useState<string | null>(null);
  const [downloadingPackage, setDownloadingPackage] = useState<string | null>(null);
  const [currentPage, setCurrentPage] = useState(0);
  const [hasMore, setHasMore] = useState(true);

  const fetchSkills = useCallback(async (query: string | undefined, limit: number) => {
    const normalized = query?.trim();
    return normalized
      ? await configAPI.searchSkillMarket(normalized, limit)
      : await configAPI.listSkillMarket(undefined, limit);
  }, []);

  const loadFirstPage = useCallback(async (query?: string) => {
    setMarketLoading(true);
    setMarketError(null);
    setCurrentPage(0);
    try {
      const skillList = await fetchSkills(query, pageSize);
      setMarketSkills(skillList);
      setHasMore(skillList.length >= pageSize);
    } catch (err) {
      log.error('Failed to load skill market', err);
      setMarketError(err instanceof Error ? err.message : String(err));
    } finally {
      setMarketLoading(false);
    }
  }, [fetchSkills, pageSize]);

  useEffect(() => {
    loadFirstPage(searchQuery || undefined);
  }, [loadFirstPage, searchQuery]);

  const refresh = useCallback(async () => {
    await loadFirstPage(searchQuery || undefined);
  }, [loadFirstPage, searchQuery]);

  const displayMarketSkills = useMemo(() => {
    const entries = marketSkills.map((skill, index) => ({
      skill,
      index,
      installed: installedSkillNames.has(skill.name),
    }));

    entries.sort((a, b) => {
      if (a.installed !== b.installed) {
        return a.installed ? -1 : 1;
      }
      const installDelta = (b.skill.installs ?? 0) - (a.skill.installs ?? 0);
      if (installDelta !== 0) {
        return installDelta;
      }
      return a.index - b.index;
    });

    return entries.map((entry) => entry.skill);
  }, [installedSkillNames, marketSkills]);

  const loadedPages = Math.ceil(displayMarketSkills.length / pageSize);
  const totalPages = hasMore ? loadedPages + 1 : Math.max(1, loadedPages);

  const paginatedSkills = useMemo(() => displayMarketSkills.slice(
    currentPage * pageSize,
    (currentPage + 1) * pageSize,
  ), [currentPage, displayMarketSkills, pageSize]);

  const goToPrevPage = useCallback(() => {
    setCurrentPage((page) => Math.max(0, page - 1));
  }, []);

  const goToNextPage = useCallback(async () => {
    const nextPage = currentPage + 1;
    const neededCount = Math.min((nextPage + 1) * pageSize, MAX_TOTAL_SKILLS);

    if (displayMarketSkills.length >= neededCount) {
      setCurrentPage(nextPage);
      return;
    }

    if (!hasMore) {
      return;
    }

    setCurrentPage(nextPage);

    try {
      setLoadingMore(true);
      const skillList = await fetchSkills(searchQuery || undefined, neededCount);
      setMarketSkills(skillList);
      const hitCap = neededCount >= MAX_TOTAL_SKILLS;
      setHasMore(!hitCap && skillList.length >= neededCount);
    } catch (err) {
      log.error('Failed to load more skills', err);
      setCurrentPage(currentPage);
    } finally {
      setLoadingMore(false);
    }
  }, [currentPage, displayMarketSkills.length, fetchSkills, hasMore, pageSize, searchQuery]);

  const handleDownload = useCallback(async (skill: SkillMarketItem, targetLevel: SkillLevel = 'project') => {
    const resolvedLevel: SkillLevel = isRemoteWorkspace ? 'user' : targetLevel;
    if (resolvedLevel === 'project' && !hasWorkspace) {
      notification.warning(t('messages.noWorkspace'));
      return;
    }
    try {
      setDownloadingPackage(skill.installId);
      const result = await configAPI.downloadSkillMarket({
        packageId: skill.installId,
        level: resolvedLevel,
        workspacePath: resolvedLevel === 'project' ? workspacePath || undefined : undefined,
      });
      const installedName = result.installedSkills[0] ?? skill.name;
      notification.success(t('messages.marketDownloadSuccess', { name: installedName }));
      await onInstalledChanged?.();
    } catch (err) {
      notification.error(
        t('messages.marketDownloadFailed', {
          error: err instanceof Error ? err.message : String(err),
        }),
      );
    } finally {
      setDownloadingPackage(null);
    }
  }, [hasWorkspace, isRemoteWorkspace, notification, onInstalledChanged, t, workspacePath]);

  return {
    marketSkills: paginatedSkills,
    marketLoading,
    loadingMore,
    marketError,
    downloadingPackage,
    hasMore,
    currentPage,
    totalPages,
    refresh,
    goToPrevPage,
    goToNextPage,
    handleDownload,
    hasWorkspace,
    isRemoteWorkspace,
    totalLoaded: displayMarketSkills.length,
  };
}
