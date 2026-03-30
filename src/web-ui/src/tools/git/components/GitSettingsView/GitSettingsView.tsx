/** Git settings view. */

import React, { useState, useCallback, useEffect } from 'react';
import { 
  Settings, 
  User, 
  Mail,
  Key,
  Globe,
  Save,
  RefreshCw,
  Check,
  X
} from 'lucide-react';
import { Button, IconButton, Tabs, TabPane, Select, Checkbox, Input } from '@/component-library';
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
    <div className="bitfun-git-settings-view__content">
      <div className="bitfun-git-settings-view__section">
        <h4 className="bitfun-git-settings-view__section-title">{t('settingsView.sections.user.title')}</h4>
        <p className="bitfun-git-settings-view__section-description">
          {t('settingsView.sections.user.description')}
        </p>
        
        <div className="bitfun-git-settings-view__form-group">
          <label className="bitfun-git-settings-view__form-label">
            <User size={16} />
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
        
        <div className="bitfun-git-settings-view__form-group">
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
    </div>
  ), [config.user, updateUserConfig, t]);

  const renderRepositoryTab = useCallback(() => (
    <div className="bitfun-git-settings-view__content">
      <div className="bitfun-git-settings-view__section">
        <h4 className="bitfun-git-settings-view__section-title">{t('settingsView.sections.editor.title')}</h4>
        
        <div className="bitfun-git-settings-view__form-group">
          <Select
            label={t('settingsView.sections.editor.defaultEditorLabel')}
            options={[
              { label: 'Visual Studio Code', value: 'code --wait' },
              { label: 'Vim', value: 'vim' },
              { label: 'Nano', value: 'nano' },
              { label: 'Emacs', value: 'emacs' },
              { label: 'Sublime Text', value: 'subl -w' },
            ]}
            value={config.core.editor}
            onChange={(value) => updateCoreConfig('editor', value as string)}
          />
        </div>
      </div>

      <div className="bitfun-git-settings-view__section">
        <h4 className="bitfun-git-settings-view__section-title">{t('settingsView.sections.lineEndings.title')}</h4>
        
        <div className="bitfun-git-settings-view__form-group">
          <Select
            label={t('settingsView.sections.lineEndings.autocrlfLabel')}
            options={[
              { label: t('settingsView.sections.lineEndings.options.auto'), value: 'true' },
              { label: t('settingsView.sections.lineEndings.options.input'), value: 'input' },
              { label: t('settingsView.sections.lineEndings.options.disabled'), value: 'false' },
            ]}
            value={config.core.autocrlf}
            onChange={(value) => updateCoreConfig('autocrlf', value as string)}
          />
        </div>
      </div>

      <div className="bitfun-git-settings-view__section">
        <h4 className="bitfun-git-settings-view__section-title">{t('settingsView.sections.remotes.title')}</h4>
        
        {Object.entries(config.remote).map(([name, remote]) => (
          <div key={name} className="bitfun-git-settings-view__config-item">
            <div className="bitfun-git-settings-view__config-info">
              <div className="bitfun-git-settings-view__config-key">{name}</div>
            </div>
            <div className="bitfun-git-settings-view__form-group">
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
    </div>
  ), [config.core, config.remote, updateCoreConfig, updateRemoteConfig, t]);

  const renderAdvancedTab = useCallback(() => (
    <div className="bitfun-git-settings-view__content">
      <div className="bitfun-git-settings-view__section">
        <h4 className="bitfun-git-settings-view__section-title">{t('settingsView.sections.core.title')}</h4>
        
        <div className="bitfun-git-settings-view__form-group">
          <Checkbox
            checked={config.core.ignorecase}
            onChange={(e) => updateCoreConfig('ignorecase', e.target.checked)}
            label={t('settingsView.sections.core.ignoreCaseLabel')}
            description={t('settingsView.sections.core.ignoreCaseDescription')}
          />
        </div>
      </div>

      <div className="bitfun-git-settings-view__section">
        <h4 className="bitfun-git-settings-view__section-title">{t('settingsView.sections.branch.title')}</h4>
        
        {Object.entries(config.branch).map(([branchName, branchConfig]) => (
          <div key={branchName} className="bitfun-git-settings-view__config-item">
            <div className="bitfun-git-settings-view__config-info">
              <div className="bitfun-git-settings-view__config-key">
                {t('settingsView.sections.branch.branchLabel', { branch: branchName })}
              </div>
            </div>
            <div className="bitfun-git-settings-view__form-group">
              <label className="bitfun-git-settings-view__form-label">{t('settingsView.sections.branch.remoteLabel')}</label>
              <Input
                className="bitfun-git-settings-view__form-input"
                type="text"
                value={branchConfig.remote || ''}
                readOnly
              />
            </div>
            <div className="bitfun-git-settings-view__form-group">
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
    </div>
  ), [config.core.ignorecase, config.branch, updateCoreConfig, t]);


  useEffect(() => {
    if (repositoryPath) {
      loadConfig();
    }
  }, [repositoryPath, loadConfig]);

  if (loading) {
    return (
      <div className={`bitfun-git-settings-view bitfun-git-settings-view--loading ${className}`}>
        <div className="bitfun-git-settings-view__empty-state">
          <RefreshCw size={24} className="bitfun-git-settings-view__loading-spinner" />
          <p>{t('settingsView.loading')}</p>
        </div>
      </div>
    );
  }

  if (error && !config.user.name) {
    return (
      <div className={`bitfun-git-settings-view bitfun-git-settings-view--error ${className}`}>
        <div className="bitfun-git-settings-view__empty-state">
          <Settings size={48} />
          <h3>{t('settingsView.loadFailedTitle')}</h3>
          <p className="bitfun-git-settings-view__error-message">{error}</p>
          <Button onClick={loadConfig} variant="primary">
            {t('settingsView.retry')}
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className={`bitfun-git-settings-view ${className}`}>
      <div className="bitfun-git-settings-view__header">
        <div className="bitfun-git-settings-view__header-left">
          <Settings size={20} />
          <h3>{t('settingsView.title')}</h3>
        </div>
        
        <div className="bitfun-git-settings-view__header-right">
          <IconButton 
            onClick={loadConfig}
            disabled={loading}
            title={t('settingsView.refresh')}
            size="small"
          >
            <RefreshCw size={16} />
          </IconButton>
          
          <Button 
            onClick={saveConfig}
            disabled={saving}
            variant="primary"
          >
            <Save size={16} />
            {saving ? t('settingsView.saving') : t('settingsView.save')}
          </Button>
        </div>
      </div>

      {error && (
        <div className="bitfun-git-settings-view__status-banner bitfun-git-settings-view__status-banner--error">
          <X size={14} />
          <span>{error}</span>
          <IconButton 
            onClick={() => setError(null)} 
            className="bitfun-git-settings-view__close-btn"
            size="xs"
            variant="ghost"
          >
            <X size={12} />
          </IconButton>
        </div>
      )}
      
      {success && (
        <div className="bitfun-git-settings-view__status-banner bitfun-git-settings-view__status-banner--success">
          <Check size={14} />
          <span>{success}</span>
          <IconButton 
            onClick={() => setSuccess(null)} 
            className="bitfun-git-settings-view__close-btn"
            size="xs"
            variant="ghost"
          >
            <X size={12} />
          </IconButton>
        </div>
      )}

      <Tabs 
        activeKey={activeTab} 
        onChange={(key: string) => setActiveTab(key as 'user' | 'repository' | 'advanced')}
      >
        <TabPane 
          tabKey="user"
          label={
            <span className="bitfun-git-settings-view__tab-label">
              <User size={16} />
              {t('settingsView.tabs.user')}
            </span>
          }
        >
          {renderUserTab()}
        </TabPane>
        <TabPane 
          tabKey="repository"
          label={
            <span className="bitfun-git-settings-view__tab-label">
              <Globe size={16} />
              {t('settingsView.tabs.repository')}
            </span>
          }
        >
          {renderRepositoryTab()}
        </TabPane>
        <TabPane 
          tabKey="advanced"
          label={
            <span className="bitfun-git-settings-view__tab-label">
              <Key size={16} />
              {t('settingsView.tabs.advanced')}
            </span>
          }
        >
          {renderAdvancedTab()}
        </TabPane>
      </Tabs>
    </div>
  );
};

export default GitSettingsView;
