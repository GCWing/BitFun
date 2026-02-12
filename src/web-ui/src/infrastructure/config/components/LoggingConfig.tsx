import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { FolderOpen, RefreshCw } from 'lucide-react';
import { Alert, Button, Select, Tooltip } from '@/component-library';
import { configAPI, workspaceAPI } from '@/infrastructure/api';
import { ConfigPageContent, ConfigPageHeader, ConfigPageLayout } from './common';
import { configManager } from '../services/ConfigManager';
import { createLogger } from '@/shared/utils/logger';
import type { BackendLogLevel, RuntimeLoggingInfo } from '../types';
import './LoggingConfig.scss';

const log = createLogger('LoggingConfig');

const LoggingConfig: React.FC = () => {
  const { t } = useTranslation('settings/logging');
  const [configLevel, setConfigLevel] = useState<BackendLogLevel>('info');
  const [runtimeInfo, setRuntimeInfo] = useState<RuntimeLoggingInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [openingFolder, setOpeningFolder] = useState(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error' | 'info'; text: string } | null>(null);

  const levelOptions = useMemo(
    () => [
      { value: 'trace', label: t('levels.trace') },
      { value: 'debug', label: t('levels.debug') },
      { value: 'info', label: t('levels.info') },
      { value: 'warn', label: t('levels.warn') },
      { value: 'error', label: t('levels.error') },
      { value: 'off', label: t('levels.off') },
    ],
    [t]
  );

  const showMessage = useCallback((type: 'success' | 'error' | 'info', text: string) => {
    setMessage({ type, text });
    setTimeout(() => setMessage(null), 3000);
  }, []);

  const loadData = useCallback(async () => {
    try {
      setLoading(true);

      const [savedLevel, info] = await Promise.all([
        configManager.getConfig<BackendLogLevel>('app.logging.level'),
        configAPI.getRuntimeLoggingInfo(),
      ]);

      setConfigLevel(savedLevel || info.effectiveLevel || 'info');
      setRuntimeInfo(info);
    } catch (error) {
      log.error('Failed to load logging config', error);
      showMessage('error', t('messages.loadFailed'));
    } finally {
      setLoading(false);
    }
  }, [showMessage, t]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  const handleLevelChange = useCallback(async (value: string) => {
    const nextLevel = value as BackendLogLevel;
    const previousLevel = configLevel;
    setConfigLevel(nextLevel);
    setSaving(true);

    try {
      await configManager.setConfig('app.logging.level', nextLevel);
      configManager.clearCache();

      const info = await configAPI.getRuntimeLoggingInfo();
      setRuntimeInfo(info);
      showMessage('success', t('messages.levelUpdated'));
    } catch (error) {
      setConfigLevel(previousLevel);
      log.error('Failed to update logging level', { nextLevel, error });
      showMessage('error', t('messages.saveFailed'));
    } finally {
      setSaving(false);
    }
  }, [configLevel, showMessage, t]);

  const handleRefresh = useCallback(async () => {
    await loadData();
    showMessage('info', t('messages.refreshed'));
  }, [loadData, showMessage, t]);

  const handleOpenFolder = useCallback(async () => {
    const folder = runtimeInfo?.sessionLogDir;
    if (!folder) {
      showMessage('error', t('messages.pathUnavailable'));
      return;
    }

    try {
      setOpeningFolder(true);
      await workspaceAPI.revealInExplorer(folder);
      showMessage('success', t('messages.openedFolder'));
    } catch (error) {
      log.error('Failed to open log folder', { folder, error });
      showMessage('error', t('messages.openFailed'));
    } finally {
      setOpeningFolder(false);
    }
  }, [runtimeInfo?.sessionLogDir, showMessage, t]);

  if (loading) {
    return (
      <ConfigPageLayout className="bitfun-logging-config">
        <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
        <ConfigPageContent>
          <div className="bitfun-logging-config__loading">{t('messages.loading')}</div>
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  return (
    <ConfigPageLayout className="bitfun-logging-config">
      <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
      <ConfigPageContent className="bitfun-logging-config__content">
        {message && (
          <div className="bitfun-logging-config__message-container">
            <Alert
              type={message.type === 'success' ? 'success' : message.type === 'error' ? 'error' : 'info'}
              message={message.text}
            />
          </div>
        )}

        <div className="bitfun-logging-config__section">
          <div className="bitfun-logging-config__section-header">
            <div className="bitfun-logging-config__section-title">
              <h3>{t('sections.level')}</h3>
            </div>
            <Tooltip content={t('actions.refreshTooltip')}>
              <button
                className="bitfun-logging-config__refresh-btn"
                onClick={handleRefresh}
                disabled={loading || saving}
              >
                <RefreshCw size={14} className={loading ? 'spinning' : ''} />
              </button>
            </Tooltip>
          </div>

          <div className="bitfun-logging-config__section-content">
            <p className="bitfun-logging-config__description">{t('level.description')}</p>
            <div className="bitfun-logging-config__select-wrapper">
              <Select
                value={configLevel}
                onChange={(v) => handleLevelChange(v as string)}
                options={levelOptions}
                disabled={saving}
              />
            </div>
          </div>
        </div>

        <div className="bitfun-logging-config__section">
          <div className="bitfun-logging-config__section-header">
            <div className="bitfun-logging-config__section-title">
              <h3>{t('sections.path')}</h3>
            </div>
            <div className="bitfun-logging-config__header-actions">
              <Button
                size="small"
                variant="secondary"
                onClick={handleOpenFolder}
                disabled={openingFolder || !runtimeInfo?.sessionLogDir}
              >
                <FolderOpen size={14} />
                {t('actions.openFolder')}
              </Button>
            </div>
          </div>

          <div className="bitfun-logging-config__section-content">
            <div className="bitfun-logging-config__path-box">
              {runtimeInfo?.sessionLogDir || '-'}
            </div>
          </div>
        </div>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default LoggingConfig;
