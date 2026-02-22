import React, { useRef, useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Alert, Switch, IconButton } from '@/component-library';
import { Save, X, RefreshCw, Upload } from 'lucide-react';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent, ConfigPageSection, ConfigPageRow } from './common';
import { LspPluginList } from '@/tools/lsp';
import { lspService } from '@/tools/lsp/services/LspService';
import { open } from '@tauri-apps/plugin-dialog';
import { createLogger } from '@/shared/utils/logger';
import './LspConfig.scss';

const log = createLogger('LspConfig');

interface LspSettings {
  autoStartEnabled: boolean;
}

const DEFAULT_LSP_SETTINGS: LspSettings = { autoStartEnabled: true };
const LSP_SETTINGS_KEY = 'bitfun_lsp_settings';

const LspConfig: React.FC = () => {
  const { t } = useTranslation('settings/lsp');
  const pluginReloadRef = useRef<(() => void) | null>(null);
  const [isInstalling, setIsInstalling] = useState(false);
  const [installMessage, setInstallMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);
  const [isInitializing, setIsInitializing] = useState(false);
  const [settings, setSettings] = useState<LspSettings>(DEFAULT_LSP_SETTINGS);
  const [hasSettingsChanges, setHasSettingsChanges] = useState(false);

  useEffect(() => { loadSettings(); }, []);

  const loadSettings = () => {
    try {
      const saved = localStorage.getItem(LSP_SETTINGS_KEY);
      if (saved) setSettings({ ...DEFAULT_LSP_SETTINGS, ...JSON.parse(saved) });
    } catch (error) {
      log.error('Failed to load settings', error);
    }
  };

  const saveSettings = () => {
    try {
      localStorage.setItem(LSP_SETTINGS_KEY, JSON.stringify(settings));
      setHasSettingsChanges(false);
      setInstallMessage({ type: 'success', text: t('messages.settingsSaved') });
      setTimeout(() => setInstallMessage(null), 2000);
    } catch (error) {
      log.error('Failed to save settings', error);
      setInstallMessage({ type: 'error', text: t('messages.saveSettingsFailed') });
    }
  };

  const handleSettingChange = (key: keyof LspSettings, value: boolean) => {
    setSettings(prev => ({ ...prev, [key]: value }));
    setHasSettingsChanges(true);
  };

  const handleInitialize = async () => {
    setIsInitializing(true);
    setInstallMessage(null);
    try {
      await lspService.initialize();
      setInstallMessage({ type: 'success', text: t('messages.initSuccess') });
    } catch (error) {
      setInstallMessage({ type: 'error', text: `${t('messages.initFailed')}: ${error instanceof Error ? error.message : String(error)}` });
    } finally {
      setIsInitializing(false);
    }
  };

  const handleInstallPlugin = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: t('fileDialog.pluginPackage'), extensions: ['vcpkg'] }]
      });
      if (!selected) return;

      setIsInstalling(true);
      setInstallMessage(null);
      await lspService.initialize();
      const pluginId = await lspService.installPlugin(selected as string);
      setInstallMessage({ type: 'success', text: t('messages.installSuccess', { pluginId }) });
      setTimeout(() => setInstallMessage(null), 3000);
    } catch (error) {
      setInstallMessage({ type: 'error', text: `${t('messages.installFailed')}: ${error instanceof Error ? error.message : String(error)}` });
    } finally {
      setIsInstalling(false);
    }
  };

  const saveActions = hasSettingsChanges ? (
    <>
      <IconButton variant="ghost" size="small" onClick={saveSettings} tooltip={t('settings.saveTooltip')}>
        <Save size={16} />
      </IconButton>
      <IconButton variant="ghost" size="small" onClick={loadSettings} tooltip={t('settings.cancelTooltip')}>
        <X size={16} />
      </IconButton>
    </>
  ) : null;

  return (
    <ConfigPageLayout className="bitfun-lsp-config">
      <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />

      <ConfigPageContent>
        {installMessage && (
          <div className="bitfun-lsp-config__message-container">
            <Alert type={installMessage.type === 'success' ? 'success' : 'error'} message={installMessage.text} />
          </div>
        )}

        <ConfigPageSection
          title={t('section.settings.title', { defaultValue: '基础设置' })}
          description={t('section.settings.description', { defaultValue: '语言服务的运行参数配置。' })}
          extra={saveActions}
        >
          <ConfigPageRow
            label={t('settings.autoStart')}
            description={t('settings.autoStartDesc')}
          >
            <Switch
              checked={settings.autoStartEnabled}
              onChange={(e) => handleSettingChange('autoStartEnabled', e.target.checked)}
              size="medium"
            />
          </ConfigPageRow>
        </ConfigPageSection>

        <ConfigPageSection
          title={t('section.plugins.title', { defaultValue: '语言插件' })}
          description={t('section.plugins.description', { defaultValue: '管理已安装的语言服务插件。' })}
          extra={
            <>
              <IconButton
                variant="ghost"
                size="small"
                onClick={handleInitialize}
                disabled={isInitializing}
                tooltip={t('pluginList.initTooltip')}
              >
                <RefreshCw size={16} className={isInitializing ? 'bitfun-lsp-config__spinning' : ''} />
              </IconButton>
              <IconButton
                variant="ghost"
                size="small"
                onClick={handleInstallPlugin}
                disabled={isInstalling}
                tooltip={t('pluginList.installTooltip')}
              >
                <Upload size={16} />
              </IconButton>
              <IconButton
                variant="ghost"
                size="small"
                onClick={() => pluginReloadRef.current?.()}
                tooltip={t('pluginList.refreshTooltip')}
              >
                <RefreshCw size={16} />
              </IconButton>
            </>
          }
        >
          <div className="bitfun-lsp-config__plugins">
            <LspPluginList
              onInitialize={handleInitialize}
              onInstallPlugin={handleInstallPlugin}
              isInitializing={isInitializing}
              isInstalling={isInstalling}
              onMountReload={(fn) => { pluginReloadRef.current = fn; }}
            />
          </div>
        </ConfigPageSection>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default LspConfig;
