/** Git settings view. */

import { Button, Checkbox, Field, Icon, IconButton, Input, Select, TabGroup, ScrollArea } from '@bitfun/ui';
import React, { useState, useCallback, useEffect } from 'react';
import { Mail, Key, Save } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';
import './GitSettingsView.scss';

interface GitSettingsViewProps {
  /** Repository path */
  repositoryPath: string;
  /** Class name */
  className?: string;
}

interface GitConfig {
  user: {
    name: string;
    email: string;
  };
  core: {
    editor: string;
    autocrlf: string;
    ignorecase: boolean;
  };
  remote: {
    [key: string]: {
      url: string;
      fetch: string;
    } | undefined;
  };
  branch: {
    [key: string]: {
      remote?: string;
      merge?: string;
    };
  };
}

const GitSettingsView: React.FC<GitSettingsViewProps> = ({
  repositoryPath,
  className = ''
}) => {
  const [config, setConfig] = useState<GitConfig>({
    user: { name: '', email: '' },
    core: { editor: 'code', autocrlf: 'input', ignorecase: true },
    remote: {},
    branch: {}
  });
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<'user' | 'repository' | 'advanced'>('user');
  const { t } = useI18n('panels/git');
  const tabItems = [
    {
      icon: <Icon name="user" size="md" />,
      id: 'git-settings-user-tab',
      label: t('settingsView.tabs.user'),
      panelId: 'git-settings-user-panel',
      value: 'user',
    },
    {
      icon: <Icon name="browser" size="md" />,
      id: 'git-settings-repository-tab',
      label: t('settingsView.tabs.repository'),
      panelId: 'git-settings-repository-panel',
      value: 'repository',
    },
    {
      icon: <Key size={16} />,
      id: 'git-settings-advanced-tab',
      label: t('settingsView.tabs.advanced'),
      panelId: 'git-settings-advanced-panel',
      value: 'advanced',
    },
  ] as const;

  const loadConfig = useCallback(async () => {
    setLoading(true);
    setError(null);

    try {

      const mockConfig: GitConfig = {
        user: {
          name: 'Developer Name',
          email: 'developer@example.com'
        },
        core: {
          editor: 'code --wait',
          autocrlf: 'input',
          ignorecase: true
        },
        remote: {
          origin: {
            url: 'https://github.com/user/repo.git',
            fetch: '+refs/heads/*:refs/remotes/origin/*'
          }
        },
        branch: {
          main: {
            remote: 'origin',
            merge: 'refs/heads/main'
          }
        }
      };

      setConfig(mockConfig);
    } catch (err) {
      setError(err instanceof Error ? err.message : t('settingsView.errors.loadConfigFailed'));
    } finally {
      setLoading(false);
    }
  }, [t]);

  const saveConfig = useCallback(async () => {
    setSaving(true);
    setError(null);
    setSuccess(null);

    try {

      await new Promise(resolve => setTimeout(resolve, 1000));
      
      setSuccess(t('settingsView.success.saveConfig'));
      setTimeout(() => setSuccess(null), 3000);
    } catch (err) {
      setError(err instanceof Error ? err.message : t('settingsView.errors.saveConfigFailed'));
    } finally {
      setSaving(false);
    }
  }, [t]);

  const updateUserConfig = useCallback((field: 'name' | 'email', value: string) => {
    setConfig(prev => ({
      ...prev,
      user: {
        ...prev.user,
        [field]: value
      }
    }));
  }, []);

  const updateCoreConfig = useCallback((field: keyof GitConfig['core'], value: string | boolean) => {
    setConfig(prev => ({
      ...prev,
      core: {
        ...prev.core,
        [field]: value
      }
    }));
  }, []);

  const updateRemoteConfig = useCallback((remoteName: string, field: 'url' | 'fetch', value: string) => {
    setConfig(prev => {
      const existingRemote = prev.remote[remoteName] || { url: '', fetch: '' };
      return {
        ...prev,
        remote: {
          ...prev.remote,
          [remoteName]: {
            ...existingRemote,
            [field]: value
          }
        }
      };
    });
  }, []);

  const renderUserTab = useCallback(() => (
    <ScrollArea data-bf-component="git-settings-view" data-bf-part="content" className="bitfun-git-settings-view__content">
      <div data-bf-component="git-settings-view" data-bf-part="section" className="bitfun-git-settings-view__section">
        <h4 className="bitfun-git-settings-view__section-title">{t('settingsView.sections.user.title')}</h4>
        <p className="bitfun-git-settings-view__section-description">
          {t('settingsView.sections.user.description')}
        </p>
        
        <div data-bf-component="git-settings-view" data-bf-part="formGroup" className="bitfun-git-settings-view__form-group">
          <label className="bitfun-git-settings-view__form-label">
            <Icon name="user" size="md" />
            {t('settingsView.sections.user.nameLabel')}
          </label>
          <Input
            className="bitfun-git-settings-view__form-input"
            type="text"
            value={config.user.name}
            onChange={(e) => updateUserConfig('name', e.target.value)}
            placeholder={t('settingsView.sections.user.namePlaceholder')}
          />
        </div>
        
        <div data-bf-component="git-settings-view" data-bf-part="formGroup" className="bitfun-git-settings-view__form-group">
          <label className="bitfun-git-settings-view__form-label">
            <Mail size={16} />
            {t('settingsView.sections.user.emailLabel')}
          </label>
          <Input
            className="bitfun-git-settings-view__form-input"
            type="email"
            value={config.user.email}
            onChange={(e) => updateUserConfig('email', e.target.value)}
            placeholder={t('settingsView.sections.user.emailPlaceholder')}
          />
        </div>
      </div>
    </ScrollArea>
  ), [config.user, updateUserConfig, t]);

  const renderRepositoryTab = useCallback(() => (
    <ScrollArea data-bf-component="git-settings-view" data-bf-part="content" className="bitfun-git-settings-view__content">
      <div data-bf-component="git-settings-view" data-bf-part="section" className="bitfun-git-settings-view__section">
        <h4 className="bitfun-git-settings-view__section-title">{t('settingsView.sections.editor.title')}</h4>
        
        <div data-bf-component="git-settings-view" data-bf-part="formGroup" className="bitfun-git-settings-view__form-group">
          <Field label={t('settingsView.sections.editor.defaultEditorLabel')} controlWidth="fill">
            <Select
              options={[
                { label: 'Visual Studio Code', value: 'code --wait' },
                { label: 'Vim', value: 'vim' },
                { label: 'Nano', value: 'nano' },
                { label: 'Emacs', value: 'emacs' },
                { label: 'Sublime Text', value: 'subl -w' },
              ]}
              value={config.core.editor}
              onValueChange={(value) => updateCoreConfig('editor', value as string)}
            />
          </Field>
        </div>
      </div>

      <div data-bf-component="git-settings-view" data-bf-part="section" className="bitfun-git-settings-view__section">
        <h4 className="bitfun-git-settings-view__section-title">{t('settingsView.sections.lineEndings.title')}</h4>
        
        <div data-bf-component="git-settings-view" data-bf-part="formGroup" className="bitfun-git-settings-view__form-group">
          <Field label={t('settingsView.sections.lineEndings.autocrlfLabel')} controlWidth="fill">
            <Select
              options={[
                { label: t('settingsView.sections.lineEndings.options.auto'), value: 'true' },
                { label: t('settingsView.sections.lineEndings.options.input'), value: 'input' },
                { label: t('settingsView.sections.lineEndings.options.disabled'), value: 'false' },
              ]}
              value={config.core.autocrlf}
              onValueChange={(value) => updateCoreConfig('autocrlf', value as string)}
            />
          </Field>
        </div>
      </div>

      <div data-bf-component="git-settings-view" data-bf-part="section" className="bitfun-git-settings-view__section">
        <h4 className="bitfun-git-settings-view__section-title">{t('settingsView.sections.remotes.title')}</h4>
        
        {Object.entries(config.remote).map(([name, remote]) => (
          <div data-bf-component="git-settings-view" data-bf-part="configItem" key={name} className="bitfun-git-settings-view__config-item">
            <div className="bitfun-git-settings-view__config-info">
              <div className="bitfun-git-settings-view__config-key">{name}</div>
            </div>
            <div data-bf-component="git-settings-view" data-bf-part="formGroup" className="bitfun-git-settings-view__form-group">
              <label className="bitfun-git-settings-view__form-label">{t('settingsView.sections.remotes.urlLabel')}</label>
              <Input
                className="bitfun-git-settings-view__form-input"
                type="text"
                value={remote?.url || ''}
                onChange={(e) => updateRemoteConfig(name, 'url', e.target.value)}
                placeholder={t('settingsView.sections.remotes.urlPlaceholder')}
              />
            </div>
          </div>
        ))}
      </div>
    </ScrollArea>
  ), [config.core, config.remote, updateCoreConfig, updateRemoteConfig, t]);

  const renderAdvancedTab = useCallback(() => (
    <ScrollArea data-bf-component="git-settings-view" data-bf-part="content" className="bitfun-git-settings-view__content">
      <div data-bf-component="git-settings-view" data-bf-part="section" className="bitfun-git-settings-view__section">
        <h4 className="bitfun-git-settings-view__section-title">{t('settingsView.sections.core.title')}</h4>
        
        <div data-bf-component="git-settings-view" data-bf-part="formGroup" className="bitfun-git-settings-view__form-group">
          <Checkbox
            checked={config.core.ignorecase}
            onChange={(e) => updateCoreConfig('ignorecase', e.target.checked)}
            label={t('settingsView.sections.core.ignoreCaseLabel')}
            description={t('settingsView.sections.core.ignoreCaseDescription')}
          />
        </div>
      </div>

      <div data-bf-component="git-settings-view" data-bf-part="section" className="bitfun-git-settings-view__section">
        <h4 className="bitfun-git-settings-view__section-title">{t('settingsView.sections.branch.title')}</h4>
        
        {Object.entries(config.branch).map(([branchName, branchConfig]) => (
          <div data-bf-component="git-settings-view" data-bf-part="configItem" key={branchName} className="bitfun-git-settings-view__config-item">
            <div className="bitfun-git-settings-view__config-info">
              <div className="bitfun-git-settings-view__config-key">
                {t('settingsView.sections.branch.branchLabel', { branch: branchName })}
              </div>
            </div>
            <div data-bf-component="git-settings-view" data-bf-part="formGroup" className="bitfun-git-settings-view__form-group">
              <label className="bitfun-git-settings-view__form-label">{t('settingsView.sections.branch.remoteLabel')}</label>
              <Input
                className="bitfun-git-settings-view__form-input"
                type="text"
                value={branchConfig.remote || ''}
                readOnly
              />
            </div>
            <div data-bf-component="git-settings-view" data-bf-part="formGroup" className="bitfun-git-settings-view__form-group">
              <label className="bitfun-git-settings-view__form-label">{t('settingsView.sections.branch.mergeLabel')}</label>
              <Input
                className="bitfun-git-settings-view__form-input"
                type="text"
                value={branchConfig.merge || ''}
                readOnly
              />
            </div>
          </div>
        ))}
      </div>
    </ScrollArea>
  ), [config.core.ignorecase, config.branch, updateCoreConfig, t]);


  useEffect(() => {
    if (repositoryPath) {
      loadConfig();
    }
  }, [repositoryPath, loadConfig]);

  if (loading) {
    return (
      <div className={`bitfun-git-settings-view bitfun-git-settings-view--loading ${className}`} data-bf-component="git-settings-view" data-bf-part="root" data-bf-state="loading">
        <div data-bf-component="git-settings-view" data-bf-part="loading" className="bitfun-git-settings-view__empty-state">
          <Icon name="refresh" size="lg" className="bitfun-git-settings-view__loading-spinner" />
          <p>{t('settingsView.loading')}</p>
        </div>
      </div>
    );
  }

  if (error && !config.user.name) {
    return (
      <div className={`bitfun-git-settings-view bitfun-git-settings-view--error ${className}`} data-bf-component="git-settings-view" data-bf-part="root" data-bf-state="error">
        <div data-bf-component="git-settings-view" data-bf-part="error" className="bitfun-git-settings-view__empty-state">
          <Icon name="settings" size="lg" />
          <h3>{t('settingsView.loadFailedTitle')}</h3>
          <p className="bitfun-git-settings-view__error-message">{error}</p>
          <Button onClick={loadConfig} variant="fill">
            {t('settingsView.retry')}
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className={`bitfun-git-settings-view ${className}`} data-bf-component="git-settings-view" data-bf-part="root">
      <div className="bitfun-git-settings-view__header" data-bf-component="git-settings-view" data-bf-part="header">
        <div data-bf-component="git-settings-view" data-bf-part="headerLeft" className="bitfun-git-settings-view__header-left">
          <Icon name="settings" size="lg" />
          <h3>{t('settingsView.title')}</h3>
        </div>
        
        <div data-bf-component="git-settings-view" data-bf-part="headerRight" className="bitfun-git-settings-view__header-right">
          <IconButton
            aria-label={t('settingsView.refresh')}
            onClick={loadConfig}
            disabled={loading}
            title={t('settingsView.refresh')}
            size="sm"
            icon={<Icon name="refresh" size="md" />}
          />
          
          <Button 
            onClick={saveConfig}
            disabled={saving}
            variant="fill"
            leadingIcon={<Save size={16} />}
          >

            {saving ? t('settingsView.saving') : t('settingsView.save')}
          </Button>
        </div>
      </div>

      {error && (
        <div data-bf-component="git-settings-view" data-bf-part="status" data-bf-state="error" className="bitfun-git-settings-view__status-banner bitfun-git-settings-view__status-banner--error">
          <Icon name="xmark" size="sm" />
          <span>{error}</span>
          <IconButton
            aria-label={t('settingsView.dismissError')}
            onClick={() => setError(null)}
            className="bitfun-git-settings-view__close-btn"
            size="sm"
            icon={<Icon name="xmark" size="xs" />}
          />
        </div>
      )}
      
      {success && (
        <div data-bf-component="git-settings-view" data-bf-part="status" className="bitfun-git-settings-view__status-banner bitfun-git-settings-view__status-banner--success">
          <Icon name="check-line" size="sm" />
          <span>{success}</span>
          <IconButton
            aria-label={t('settingsView.dismissSuccess')}
            onClick={() => setSuccess(null)}
            className="bitfun-git-settings-view__close-btn"
            size="sm"
            icon={<Icon name="xmark" size="xs" />}
          />
        </div>
      )}

      <div className="bitfun-git-settings-view__tabs" data-bf-component="git-settings-view" data-bf-part="tabs">
        <TabGroup
          className="bitfun-git-settings-view__tab-list"
          items={tabItems}
          onValueChange={(value) => setActiveTab(value as typeof activeTab)}
          value={activeTab}
        />
        <div
          aria-labelledby={`git-settings-${activeTab}-tab`}
          id={`git-settings-${activeTab}-panel`}
          role="tabpanel"
        >
          {activeTab === 'user' && renderUserTab()}
          {activeTab === 'repository' && renderRepositoryTab()}
          {activeTab === 'advanced' && renderAdvancedTab()}
        </div>
      </div>
    </div>
  );
};

export default GitSettingsView;
