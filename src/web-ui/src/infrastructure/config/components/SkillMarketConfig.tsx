import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Download } from 'lucide-react';
import { Button, Card, CardBody, Search, Tooltip } from '@/component-library';
import { ConfigPageContent, ConfigPageHeader, ConfigPageLayout } from './common';
import { useCurrentWorkspace } from '../../hooks/useWorkspace';
import { useNotification } from '@/shared/notification-system';
import { configAPI } from '../../api/service-api/ConfigAPI';
import type { SkillInfo, SkillMarketItem } from '../types';
import { createLogger } from '@/shared/utils/logger';
import './SkillsConfig.scss';

const log = createLogger('SkillMarketConfig');

const SkillMarketConfig: React.FC = () => {
  const { t } = useTranslation('settings/skills');
  const { hasWorkspace, workspacePath } = useCurrentWorkspace();
  const notification = useNotification();

  const [keyword, setKeyword] = useState('');
  const [marketSkills, setMarketSkills] = useState<SkillMarketItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [downloading, setDownloading] = useState<string | null>(null);
  const [installedSkills, setInstalledSkills] = useState<SkillInfo[]>([]);

  const loadInstalledSkills = useCallback(async (forceRefresh?: boolean) => {
    try {
      const skillList = await configAPI.getSkillConfigs(forceRefresh);
      setInstalledSkills(skillList);
    } catch (err) {
      log.error('Failed to load installed skills', err);
    }
  }, []);

  const loadMarketSkills = useCallback(async (query?: string) => {
    try {
      setLoading(true);
      setError(null);

      const normalized = query?.trim();
      const skillList = normalized
        ? await configAPI.searchSkillMarket(normalized, 20)
        : await configAPI.listSkillMarket(undefined, 20);

      setMarketSkills(skillList);
    } catch (err) {
      log.error('Failed to load skill market', err);
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadInstalledSkills();
  }, [loadInstalledSkills]);

  useEffect(() => {
    if (hasWorkspace) {
      loadInstalledSkills();
    }
  }, [hasWorkspace, workspacePath, loadInstalledSkills]);

  useEffect(() => {
    loadMarketSkills();
  }, [loadMarketSkills]);

  const installedSkillNames = useMemo(
    () => new Set(installedSkills.map((skill) => skill.name)),
    [installedSkills]
  );

  const handleSearch = useCallback(() => {
    loadMarketSkills(keyword);
  }, [keyword, loadMarketSkills]);

  const handleDownload = async (skill: SkillMarketItem) => {
    if (!hasWorkspace) {
      notification.warning(t('messages.noWorkspace'));
      return;
    }

    try {
      setDownloading(skill.installId);
      const result = await configAPI.downloadSkillMarket(skill.installId, 'project');
      const installedName = result.installedSkills[0] ?? skill.name;
      notification.success(t('messages.marketDownloadSuccess', { name: installedName }));
      await loadInstalledSkills(true);
    } catch (err) {
      notification.error(t('messages.marketDownloadFailed', { error: err instanceof Error ? err.message : String(err) }));
    } finally {
      setDownloading(null);
    }
  };

  const renderMarketList = () => {
    if (loading) {
      return <div className="bitfun-skills-config__loading">{t('market.loading')}</div>;
    }

    if (error) {
      return <div className="bitfun-skills-config__error">{t('market.errorPrefix')}{error}</div>;
    }

    if (marketSkills.length === 0) {
      return (
        <div className="bitfun-skills-config__empty">
          {keyword.trim() ? t('market.empty.noMatch') : t('market.empty.noSkills')}
        </div>
      );
    }

    return (
      <div className="bitfun-skills-config__market-list">
        {marketSkills.map((skill) => {
          const isDownloading = downloading === skill.installId;
          const isInstalled = installedSkillNames.has(skill.name);
          const tooltipText = !hasWorkspace
            ? t('messages.noWorkspace')
            : isInstalled
              ? t('market.item.installedTooltip')
              : t('market.item.downloadProject');

          return (
            <Card
              key={skill.installId}
              variant="default"
              padding="none"
              className="bitfun-skills-config__market-item"
            >
              <CardBody className="bitfun-skills-config__market-item-body">
                <div className="bitfun-skills-config__market-item-main">
                  <div className="bitfun-skills-config__market-item-name">{skill.name}</div>
                  <div className="bitfun-skills-config__market-item-description">
                    {skill.description?.trim() || t('market.item.noDescription')}
                  </div>
                  <div className="bitfun-skills-config__market-item-meta">
                    {skill.source ? (
                      <span className="bitfun-skills-config__market-item-source">
                        {t('market.item.sourceLabel')}{skill.source}
                      </span>
                    ) : null}
                    <span className="bitfun-skills-config__market-item-installs">
                      {t('market.item.installs', { count: skill.installs.toLocaleString() })}
                    </span>
                  </div>
                </div>

                <Tooltip content={tooltipText}>
                  <span>
                    <Button
                      variant="primary"
                      size="small"
                      onClick={() => handleDownload(skill)}
                      disabled={isDownloading || !hasWorkspace || isInstalled}
                    >
                      <Download size={14} />
                      {isDownloading
                        ? t('market.item.downloading')
                        : isInstalled
                          ? t('market.item.installed')
                          : t('market.item.downloadProject')}
                    </Button>
                  </span>
                </Tooltip>
              </CardBody>
            </Card>
          );
        })}
      </div>
    );
  };

  return (
    <ConfigPageLayout className="bitfun-skills-config">
      <ConfigPageHeader
        title={t('market.title')}
        subtitle={t('market.subtitle')}
      />

      <ConfigPageContent className="bitfun-skills-config__content">
        <div className="bitfun-skills-config__toolbar">
          <div className="bitfun-skills-config__search-box">
            <Search
              placeholder={t('market.searchPlaceholder')}
              value={keyword}
              onChange={(value) => setKeyword(value)}
              onSearch={handleSearch}
              showSearchButton
              clearable
              size="small"
            />
          </div>
        </div>

        {renderMarketList()}
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default SkillMarketConfig;
