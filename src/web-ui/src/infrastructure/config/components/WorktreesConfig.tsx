import React, { useCallback, useEffect, useState } from 'react';
import { GitBranch, RotateCcw, Save } from 'lucide-react';
import {
  Button,
  ConfigPageLoading,
  ConfigPageMessage,
  Input,
  Switch,
} from '@/component-library';
import { configAPI } from '@/infrastructure/api';
import type { WorktreeSettings } from '@/infrastructure/api/service-api/WorktreeAPI';
import { useI18n } from '@/infrastructure/i18n';
import {
  ConfigPageContent,
  ConfigPageHeader,
  ConfigPageLayout,
  ConfigPageRow,
  ConfigPageSection,
} from './common';
import './WorktreesConfig.scss';

const DEFAULT_SETTINGS: WorktreeSettings = {
  rootPath: '~/.bitfun/worktrees',
  branchPrefix: 'bitfun/',
  copyLocalChanges: false,
};

const WorktreesConfig: React.FC = () => {
  const { t } = useI18n('worktrees');
  const [settings, setSettings] = useState(DEFAULT_SETTINGS);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{
    type: 'success' | 'error' | 'info';
    text: string;
  } | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setMessage(null);
    try {
      const configured = await configAPI.getConfig('app.worktrees', {
        skipRetryOnNotFound: true,
      });
      setSettings({
        ...DEFAULT_SETTINGS,
        ...(configured && typeof configured === 'object' ? configured : {}),
      });
    } catch {
      setMessage({ type: 'error', text: t('settings.loadFailed') });
    } finally {
      setLoading(false);
    }
  }, [t]);

  useEffect(() => {
    void load();
  }, [load]);

  const save = async () => {
    if (!settings.rootPath.trim() || !settings.branchPrefix.trim()) {
      setMessage({ type: 'error', text: t('settings.required') });
      return;
    }
    setSaving(true);
    setMessage(null);
    try {
      await configAPI.setConfig('app.worktrees', {
        ...settings,
        rootPath: settings.rootPath.trim(),
        branchPrefix: settings.branchPrefix.trim(),
      });
      setMessage({ type: 'success', text: t('settings.saved') });
    } catch {
      setMessage({ type: 'error', text: t('settings.saveFailed') });
    } finally {
      setSaving(false);
    }
  };

  if (loading) {
    return <ConfigPageLoading text={t('settings.loading')} />;
  }

  return (
    <ConfigPageLayout className="bitfun-worktrees-config">
      <ConfigPageHeader
        icon={<GitBranch size={20} aria-hidden />}
        title={t('settings.title')}
        subtitle={t('settings.description')}
      />
      <ConfigPageContent>
        <ConfigPageMessage message={message} />
        <ConfigPageSection
          title={t('settings.isolation.title')}
          description={t('settings.isolation.description')}
        >
          <ConfigPageRow
            label={t('settings.rootPath.label')}
            description={t('settings.rootPath.description')}
          >
            <Input
              value={settings.rootPath}
              onChange={event => setSettings(current => ({
                ...current,
                rootPath: event.target.value,
              }))}
              disabled={saving}
            />
          </ConfigPageRow>
          <ConfigPageRow
            label={t('settings.branchPrefix.label')}
            description={t('settings.branchPrefix.description')}
          >
            <Input
              value={settings.branchPrefix}
              onChange={event => setSettings(current => ({
                ...current,
                branchPrefix: event.target.value,
              }))}
              disabled={saving}
            />
          </ConfigPageRow>
          <ConfigPageRow
            label={t('settings.copyChanges.label')}
            description={t('settings.copyChanges.description')}
            align="center"
          >
            <Switch
              checked={settings.copyLocalChanges}
              onChange={event => setSettings(current => ({
                ...current,
                copyLocalChanges: event.target.checked,
              }))}
              disabled={saving}
            />
          </ConfigPageRow>
        </ConfigPageSection>
        <div className="bitfun-worktrees-config__actions">
          <Button
            variant="ghost"
            size="small"
            onClick={() => setSettings(DEFAULT_SETTINGS)}
            disabled={saving}
          >
            <RotateCcw size={14} aria-hidden />
            {t('settings.reset')}
          </Button>
          <Button
            variant="primary"
            size="small"
            onClick={() => void save()}
            isLoading={saving}
          >
            <Save size={14} aria-hidden />
            {t('settings.save')}
          </Button>
        </div>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default WorktreesConfig;
