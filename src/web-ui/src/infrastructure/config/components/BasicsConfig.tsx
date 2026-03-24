import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { FolderOpen, Upload } from 'lucide-react';
import {
  Alert,
  Select,
  Switch,
  Tooltip,
  ConfigPageLoading,
  ConfigPageMessage,
  ConfigPageRefreshButton,
  IconButton,
} from '@/component-library';
import { configAPI, workspaceAPI } from '@/infrastructure/api';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import { getTerminalService } from '@/tools/terminal';
import type { ShellInfo } from '@/tools/terminal/types/session';
import {
  useTheme,
  useThemeManagement,
  ThemeMetadata,
  ThemeConfig as ThemeConfigType,
  SYSTEM_THEME_ID,
} from '@/infrastructure/theme';
import { themeService } from '@/infrastructure/theme/core/ThemeService';
import { useLanguageSelector } from '@/infrastructure/i18n';
import type { LocaleId } from '@/infrastructure/i18n/types';
import {
  ConfigPageContent,
  ConfigPageHeader,
  ConfigPageLayout,
  ConfigPageSection,
  ConfigPageRow,
} from './common';
import { configManager } from '../services/ConfigManager';
import { createLogger } from '@/shared/utils/logger';
import type { BackendLogLevel, RuntimeLoggingInfo, TerminalConfig as TerminalSettings } from '../types';
import './BasicsConfig.scss';

const log = createLogger('BasicsConfig');

function BasicsAppearanceSection() {
  const { t } = useTranslation('settings/basics');
  const { themeId, themes, setTheme, loading } = useTheme();
  const { importTheme } = useThemeManagement();
  const { currentLanguage, supportedLocales, selectLanguage, isChanging } = useLanguageSelector();
  const [importing, setImporting] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const handleThemeChange = async (newThemeId: string) => {
    await setTheme(newThemeId);
  };

  const handleImportTheme = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;

    setImporting(true);
    try {
      const text = await file.text();
      const themeData = JSON.parse(text);
      await importTheme(themeData);
      alert(t('appearance.importSuccess'));
    } catch (error) {
      log.error('Failed to import theme', error);
      alert(t('appearance.importFailed'));
    } finally {
      setImporting(false);
      event.target.value = '';
    }
  };

  const handleImportClick = () => {
    fileInputRef.current?.click();
  };

  const getThemeDisplayName = (theme: ThemeMetadata) => {
    const i18nKey = `appearance.presets.${theme.id}`;
    return theme.builtin
      ? t(`${i18nKey}.name`, { defaultValue: theme.name })
      : theme.name;
  };

  const getThemeDisplayDescription = (theme: ThemeMetadata) => {
    const i18nKey = `appearance.presets.${theme.id}`;
    return theme.builtin
      ? t(`${i18nKey}.description`, { defaultValue: theme.description || '' })
      : theme.description || '';
  };

  const themeSelectOptions = useMemo(
    () => [
      {
        value: SYSTEM_THEME_ID,
        label: t('appearance.systemTheme'),
        description: t('appearance.systemThemeDescription'),
      },
      ...themes.map((theme) => ({
        value: theme.id,
        label: getThemeDisplayName(theme),
        description: getThemeDisplayDescription(theme),
      })),
    ],
    [themes, t]
  );

  return (
    <div className="theme-config">
      <div className="theme-config__content">
        <ConfigPageSection
          title={t('appearance.title')}
          description={t('appearance.hint')}
          extra={
            <>
              <IconButton
                variant="ghost"
                size="small"
                onClick={handleImportClick}
                disabled={importing || loading}
                isLoading={importing}
                title={importing ? t('appearance.importing') : t('appearance.importTheme')}
              >
                <Upload size={16} />
              </IconButton>
              <input
                ref={fileInputRef}
                type="file"
                accept=".json"
                onChange={handleImportTheme}
                style={{ display: 'none' }}
              />
            </>
          }
        >
          <ConfigPageRow
            label={t('appearance.language')}
            description={t('appearance.languageRowHint', {
              defaultValue: 'Choose one language pack as the active UI language.',
            })}
            align="center"
          >
            <div className="theme-config__language-select">
              <Select
                value={currentLanguage}
                onChange={(value) =>
                  selectLanguage(String(Array.isArray(value) ? value[0] ?? '' : value) as LocaleId)
                }
                options={supportedLocales.map((locale) => ({
                  value: locale.id,
                  label: locale.nativeName,
                }))}
                disabled={isChanging}
                placeholder={t('appearance.language')}
              />
            </div>
          </ConfigPageRow>
          <ConfigPageRow
            label={t('appearance.themes')}
            description={t('appearance.themeRowHint', {
              defaultValue: 'Pick an installed theme or manage custom themes.',
            })}
            align="center"
          >
            <div className="theme-config__theme-picker">
              <div className="theme-config__theme-select">
                <Select
                  value={themeId ?? ''}
                  onChange={(value) => handleThemeChange(value as string)}
                  disabled={loading}
                  options={themeSelectOptions}
                  renderOption={(option) => {
                    const v = String(option.value);
                    const fullTheme =
                      v === SYSTEM_THEME_ID
                        ? themeService.getTheme(themeService.getResolvedThemeId())
                        : (() => {
                            const meta = themes.find((item) => item.id === v);
                            return meta ? themeService.getTheme(meta.id) : null;
                          })();
                    const optionContent = (
                      <div className="theme-config__theme-option">
                        <span className="theme-config__theme-option-name">{option.label}</span>
                        {option.description && (
                          <span className="theme-config__theme-option-desc">{option.description}</span>
                        )}
                      </div>
                    );

                    if (!fullTheme) return optionContent;

                    return (
                      <Tooltip
                        content={<ThemePreviewThumbnail theme={fullTheme} />}
                        placement="right"
                        delay={300}
                        className="theme-preview-tooltip"
                      >
                        {optionContent}
                      </Tooltip>
                    );
                  }}
                />
              </div>
            </div>
          </ConfigPageRow>
        </ConfigPageSection>
      </div>
    </div>
  );
}

function BasicsLaunchAtLoginSection() {
  const { t } = useTranslation('settings/basics');
  const isTauri = typeof window !== 'undefined' && '__TAURI__' in window;
  const [enabled, setEnabled] = useState(false);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error' | 'info'; text: string } | null>(null);

  const showMessage = useCallback((type: 'success' | 'error' | 'info', text: string) => {
    setMessage({ type, text });
    setTimeout(() => setMessage(null), 3000);
  }, []);

  useEffect(() => {
    if (!isTauri) {
      setLoading(false);
      return;
    }

    let cancelled = false;
    void (async () => {
      try {
        setLoading(true);
        const v = await systemAPI.getLaunchAtLoginEnabled();
        if (!cancelled) {
          setEnabled(v);
        }
      } catch (error) {
        log.error('Failed to load launch-at-login state', error);
        if (!cancelled) {
          showMessage('error', t('launchAtLogin.messages.loadFailed'));
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [isTauri, showMessage, t]);

  const handleToggle = useCallback(
    async (next: boolean) => {
      const previous = enabled;
      setEnabled(next);
      setSaving(true);
      try {
        await systemAPI.setLaunchAtLoginEnabled(next);
      } catch (error) {
        setEnabled(previous);
        log.error('Failed to set launch-at-login', { next, error });
        showMessage('error', t('launchAtLogin.messages.saveFailed'));
      } finally {
        setSaving(false);
      }
    },
    [enabled, showMessage, t]
  );

  if (!isTauri) {
    return null;
  }

  if (loading) {
    return <ConfigPageLoading text={t('launchAtLogin.messages.loading')} />;
  }

  return (
    <div className="bitfun-launch-at-login-config">
      <div className="bitfun-launch-at-login-config__content">
        <ConfigPageMessage message={message} />
        <ConfigPageSection
          title={t('launchAtLogin.sections.title')}
          description={t('launchAtLogin.sections.hint')}
        >
          <ConfigPageRow
            label={t('launchAtLogin.toggleLabel')}
            description={t('launchAtLogin.toggleDescription')}
            align="center"
          >
            <Switch
              checked={enabled}
              onChange={(e) => {
                void handleToggle(e.target.checked);
              }}
              disabled={saving}
            />
          </ConfigPageRow>
        </ConfigPageSection>
      </div>
    </div>
  );
}

function BasicsLoggingSection() {
  const { t } = useTranslation('settings/basics');
  const [configLevel, setConfigLevel] = useState<BackendLogLevel>('info');
  const [runtimeInfo, setRuntimeInfo] = useState<RuntimeLoggingInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [openingFolder, setOpeningFolder] = useState(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error' | 'info'; text: string } | null>(null);

  const levelOptions = useMemo(
    () => [
      { value: 'trace', label: t('logging.levels.trace') },
      { value: 'debug', label: t('logging.levels.debug') },
      { value: 'info', label: t('logging.levels.info') },
      { value: 'warn', label: t('logging.levels.warn') },
      { value: 'error', label: t('logging.levels.error') },
      { value: 'off', label: t('logging.levels.off') },
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
      showMessage('error', t('logging.messages.loadFailed'));
    } finally {
      setLoading(false);
    }
  }, [showMessage, t]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  const handleLevelChange = useCallback(
    async (value: string) => {
      const nextLevel = value as BackendLogLevel;
      const previousLevel = configLevel;
      setConfigLevel(nextLevel);
      setSaving(true);

      try {
        await configManager.setConfig('app.logging.level', nextLevel);
        configManager.clearCache();

        const info = await configAPI.getRuntimeLoggingInfo();
        setRuntimeInfo(info);
        showMessage('success', t('logging.messages.levelUpdated'));
      } catch (error) {
        setConfigLevel(previousLevel);
        log.error('Failed to update logging level', { nextLevel, error });
        showMessage('error', t('logging.messages.saveFailed'));
      } finally {
        setSaving(false);
      }
    },
    [configLevel, showMessage, t]
  );

  const handleRefresh = useCallback(async () => {
    await loadData();
    showMessage('info', t('logging.messages.refreshed'));
  }, [loadData, showMessage, t]);

  const handleOpenFolder = useCallback(async () => {
    const folder = runtimeInfo?.sessionLogDir;
    if (!folder) {
      showMessage('error', t('logging.messages.pathUnavailable'));
      return;
    }

    try {
      setOpeningFolder(true);
      await workspaceAPI.revealInExplorer(folder);
    } catch (error) {
      log.error('Failed to open log folder', { folder, error });
      showMessage('error', t('logging.messages.openFailed'));
    } finally {
      setOpeningFolder(false);
    }
  }, [runtimeInfo?.sessionLogDir, showMessage, t]);

  if (loading) {
    return <ConfigPageLoading text={t('logging.messages.loading')} />;
  }

  return (
    <div className="bitfun-logging-config">
      <div className="bitfun-logging-config__content">
        <ConfigPageMessage message={message} />

        <ConfigPageSection
          title={t('logging.sections.logging')}
          description={t('logging.sections.loggingHint')}
          extra={
            <ConfigPageRefreshButton
              tooltip={t('logging.actions.refreshTooltip')}
              onClick={handleRefresh}
              loading={loading}
              disabled={loading || saving}
            />
          }
        >
          <ConfigPageRow
            label={t('logging.sections.level')}
            description={t('logging.level.description')}
            align="center"
          >
            <div className="bitfun-logging-config__select-wrapper">
              <Select
                value={configLevel}
                onChange={(v) => handleLevelChange(v as string)}
                options={levelOptions}
                disabled={saving}
              />
            </div>
          </ConfigPageRow>
          <ConfigPageRow
            label={t('logging.sections.path')}
            description={t('logging.path.description')}
            multiline
          >
            <div className="bitfun-logging-config__path-row">
              <div className="bitfun-logging-config__path-box">
                {runtimeInfo?.sessionLogDir || '-'}
              </div>
              <Tooltip content={t('logging.actions.openFolderTooltip')} placement="top">
                <button
                  type="button"
                  className="bitfun-logging-config__open-btn"
                  onClick={handleOpenFolder}
                  disabled={openingFolder || !runtimeInfo?.sessionLogDir}
                >
                  <FolderOpen size={14} />
                </button>
              </Tooltip>
            </div>
          </ConfigPageRow>
        </ConfigPageSection>
      </div>
    </div>
  );
}

function BasicsTerminalSection() {
  const { t } = useTranslation('settings/basics');
  const [defaultShell, setDefaultShell] = useState<string>('');
  const [availableShells, setAvailableShells] = useState<ShellInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error' | 'info'; text: string } | null>(null);
  const [platform, setPlatform] = useState<string>('');

  const showMessage = useCallback((type: 'success' | 'error' | 'info', text: string) => {
    setMessage({ type, text });
    setTimeout(() => setMessage(null), 3000);
  }, []);

  const loadData = useCallback(async () => {
    try {
      setLoading(true);

      const [terminalConfig, shells, systemInfo] = await Promise.all([
        configManager.getConfig<TerminalSettings>('terminal'),
        getTerminalService().getAvailableShells(),
        systemAPI.getSystemInfo().catch(() => ({ platform: '' })),
      ]);

      setDefaultShell(terminalConfig?.default_shell || '');

      const availableOnly = shells.filter((s) => s.available);
      setAvailableShells(availableOnly);

      setPlatform(systemInfo.platform || '');
    } catch (error) {
      log.error('Failed to load terminal config data', error);
      showMessage('error', t('terminal.messages.loadFailed'));
    } finally {
      setLoading(false);
    }
  }, [showMessage, t]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  const handleShellChange = useCallback(
    async (value: string) => {
      try {
        setSaving(true);
        setDefaultShell(value);

        await configManager.setConfig('terminal.default_shell', value);

        configManager.clearCache();

        showMessage('success', t('terminal.messages.updated'));
      } catch (error) {
        log.error('Failed to save terminal config', { shell: value, error });
        showMessage('error', t('terminal.messages.saveFailed'));
      } finally {
        setSaving(false);
      }
    },
    [showMessage, t]
  );

  const handleRefresh = useCallback(async () => {
    await loadData();
    showMessage('info', t('terminal.messages.refreshed'));
  }, [loadData, showMessage, t]);

  const shouldShowPowerShellCoreRecommendation = useMemo(() => {
    const isWindows = platform === 'windows';
    if (!isWindows) return false;

    const hasPowerShellCore = availableShells.some((shell) => shell.shellType === 'PowerShellCore');

    return !hasPowerShellCore;
  }, [availableShells, platform]);

  const shellOptions = useMemo(
    () => [
      { value: '', label: t('terminal.controls.autoDetect') },
      ...availableShells.map((shell) => ({
        value: shell.shellType,
        label: `${shell.name}${shell.version ? ` (${shell.version})` : ''}`,
      })),
    ],
    [availableShells, t]
  );

  const terminalSectionDescription = useMemo(() => {
    const hint = t('terminal.sections.terminalHint');
    if (!shouldShowPowerShellCoreRecommendation) {
      return hint;
    }
    return (
      <>
        {hint}
        <span className="bitfun-terminal-config__section-hint-sep"> · </span>
        <span className="bitfun-terminal-config__section-hint-extra">
          {t('terminal.recommendations.pwsh.prefix')}{' '}
          <span className="bitfun-terminal-config__section-hint-extra-name">
            {t('terminal.recommendations.pwsh.name')}
          </span>
          {t('terminal.recommendations.pwsh.suffix')}{' '}
          <a
            href="https://aka.ms/PSWindows"
            target="_blank"
            rel="noopener noreferrer"
            className="bitfun-terminal-config__section-hint-link"
          >
            {t('terminal.recommendations.pwsh.link')}
          </a>
        </span>
      </>
    );
  }, [shouldShowPowerShellCoreRecommendation, t]);

  if (loading) {
    return <ConfigPageLoading text={t('terminal.messages.loading')} />;
  }

  return (
    <div className="bitfun-terminal-config">
      <div className="bitfun-terminal-config__content">
        <ConfigPageMessage message={message} />

        <ConfigPageSection
          title={t('terminal.sections.terminal')}
          description={terminalSectionDescription}
          extra={
            <ConfigPageRefreshButton
              tooltip={t('terminal.controls.refreshTooltip')}
              onClick={handleRefresh}
              loading={loading}
              disabled={loading}
            />
          }
        >
          <ConfigPageRow
            label={t('terminal.sections.defaultTerminal')}
            description={t('terminal.controls.description')}
            align="center"
          >
            <div className="bitfun-terminal-config__select-wrapper">
              {availableShells.length > 0 ? (
                <Select
                  value={defaultShell}
                  onChange={(v) => handleShellChange(v as string)}
                  options={shellOptions}
                  placeholder={t('terminal.controls.placeholder')}
                  disabled={saving}
                />
              ) : (
                <div className="bitfun-terminal-config__no-shells">{t('terminal.controls.noShells')}</div>
              )}
            </div>
          </ConfigPageRow>

          {platform === 'windows' && defaultShell === 'Cmd' && (
            <div className="bitfun-terminal-config__inline-alert">
              <Alert type="warning" message={t('terminal.warnings.cmd')} />
            </div>
          )}
          {platform === 'windows' && defaultShell === 'Bash' && (
            <div className="bitfun-terminal-config__inline-alert">
              <Alert type="warning" message={t('terminal.warnings.gitBash')} />
            </div>
          )}
        </ConfigPageSection>
      </div>
    </div>
  );
}

interface ThemePreviewThumbnailProps {
  theme: ThemeConfigType;
}

function ThemePreviewThumbnail({ theme }: ThemePreviewThumbnailProps) {
  const { colors } = theme;

  return (
    <div
      className="theme-preview-thumbnail"
      style={{
        background: colors.background.primary,
        borderColor: colors.border.base,
      }}
    >
      <div
        className="theme-preview-thumbnail__titlebar"
        style={{
          background: colors.background.secondary,
          borderColor: colors.border.subtle,
        }}
      >
        <div className="theme-preview-thumbnail__menu">
          <span
            className="theme-preview-thumbnail__menu-dot"
            style={{ background: colors.accent['500'] }}
          />
        </div>

        <div className="theme-preview-thumbnail__title" style={{ color: colors.text.muted }}>
          BitFun
        </div>

        <div className="theme-preview-thumbnail__window-controls">
          <span className="theme-preview-thumbnail__window-btn" style={{ color: colors.text.secondary }}>
            <svg width="8" height="8" viewBox="0 0 14 14" fill="none">
              <line x1="3" y1="7" x2="11" y2="7" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
            </svg>
          </span>

          <span className="theme-preview-thumbnail__window-btn" style={{ color: colors.text.secondary }}>
            <svg width="8" height="8" viewBox="0 0 12 12" fill="none">
              <rect
                x="2"
                y="2"
                width="8"
                height="8"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          </span>

          <span
            className="theme-preview-thumbnail__window-btn theme-preview-thumbnail__window-btn--close"
            style={{ color: colors.text.secondary }}
          >
            <svg width="8" height="8" viewBox="0 0 14 14" fill="none">
              <line x1="3" y1="3" x2="11" y2="11" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
              <line x1="11" y1="3" x2="3" y2="11" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
            </svg>
          </span>
        </div>
      </div>

      <div className="theme-preview-thumbnail__main">
        <div
          className="theme-preview-thumbnail__sidebar"
          style={{
            background: colors.background.secondary,
            borderColor: colors.border.subtle,
          }}
        >
          <div className="theme-preview-thumbnail__tree-item">
            <span
              className="theme-preview-thumbnail__folder-icon"
              style={{ background: colors.accent['500'] }}
            />
            <span
              className="theme-preview-thumbnail__tree-text"
              style={{ background: colors.text.secondary }}
            />
          </div>

          {[1, 2, 3].map((i) => (
            <div key={i} className="theme-preview-thumbnail__tree-item theme-preview-thumbnail__tree-item--file">
              <span
                className="theme-preview-thumbnail__file-icon"
                style={{ background: colors.semantic.info }}
              />
              <span
                className="theme-preview-thumbnail__tree-text theme-preview-thumbnail__tree-text--short"
                style={{ background: colors.text.muted }}
              />
            </div>
          ))}
        </div>

        <div className="theme-preview-thumbnail__chat" style={{ background: colors.background.scene }}>
          <div
            className="theme-preview-thumbnail__message theme-preview-thumbnail__message--user"
            style={{
              background: colors.accent['200'],
              borderColor: colors.accent['400'],
            }}
          >
            <div
              className="theme-preview-thumbnail__message-line"
              style={{ background: colors.text.primary }}
            />
          </div>

          <div
            className="theme-preview-thumbnail__message theme-preview-thumbnail__message--ai"
            style={{
              background: colors.element.subtle,
              borderColor: colors.border.subtle,
            }}
          >
            <div
              className="theme-preview-thumbnail__message-line"
              style={{ background: colors.text.secondary }}
            />
            <div
              className="theme-preview-thumbnail__message-line theme-preview-thumbnail__message-line--short"
              style={{ background: colors.text.muted }}
            />
          </div>

          <div
            className="theme-preview-thumbnail__code-block"
            style={{
              background: colors.background.tertiary,
              borderColor: colors.border.base,
            }}
          >
            <div
              className="theme-preview-thumbnail__code-line"
              style={{ background: colors.purple?.['500'] || colors.accent['500'] }}
            />
            <div
              className="theme-preview-thumbnail__code-line theme-preview-thumbnail__code-line--long"
              style={{ background: colors.semantic.success }}
            />
          </div>
        </div>

        <div
          className="theme-preview-thumbnail__editor"
          style={{
            background: colors.background.workbench,
            borderColor: colors.border.subtle,
          }}
        >
          <div
            className="theme-preview-thumbnail__tabs"
            style={{
              background: colors.background.secondary,
              borderColor: colors.border.subtle,
            }}
          >
            <span
              className="theme-preview-thumbnail__tab theme-preview-thumbnail__tab--active"
              style={{
                background: colors.background.primary,
                borderColor: colors.accent['500'],
              }}
            />
            <span
              className="theme-preview-thumbnail__tab"
              style={{ background: colors.element.subtle }}
            />
          </div>

          <div className="theme-preview-thumbnail__code-content">
            {[1, 2, 3, 4, 5].map((i) => (
              <div key={i} className="theme-preview-thumbnail__editor-line">
                <span
                  className="theme-preview-thumbnail__line-number"
                  style={{ background: colors.text.disabled }}
                />
                <span
                  className="theme-preview-thumbnail__line-code"
                  style={{
                    background: i % 2 === 0 ? colors.accent['500'] : colors.text.secondary,
                    width: `${30 + (i * 8) % 40}%`,
                  }}
                />
              </div>
            ))}
          </div>
        </div>
      </div>

      <div
        className="theme-preview-thumbnail__statusbar"
        style={{
          background: colors.background.secondary,
          borderColor: colors.border.subtle,
        }}
      >
        <div className="theme-preview-thumbnail__status-section">
          <span
            className="theme-preview-thumbnail__status-icon"
            style={{ background: colors.accent['500'] }}
          />
          <span
            className="theme-preview-thumbnail__status-text"
            style={{ background: colors.text.muted }}
          />
        </div>

        <div className="theme-preview-thumbnail__status-section">
          <span className="theme-preview-thumbnail__git-icon" style={{ color: colors.git.branch }}>
            <svg width="7" height="7" viewBox="0 0 16 16" fill="none">
              <circle cx="4" cy="4" r="2" stroke="currentColor" strokeWidth="1.5" />
              <circle cx="12" cy="12" r="2" stroke="currentColor" strokeWidth="1.5" />
              <circle cx="12" cy="4" r="2" stroke="currentColor" strokeWidth="1.5" />
              <path d="M4 6v4c0 1.1.9 2 2 2h4" stroke="currentColor" strokeWidth="1.5" />
            </svg>
          </span>
          <span
            className="theme-preview-thumbnail__status-text theme-preview-thumbnail__status-text--branch"
            style={{ background: colors.git.branch }}
          />
        </div>

        <span
          className="theme-preview-thumbnail__status-icon theme-preview-thumbnail__status-icon--notification"
          style={{ background: colors.semantic.info }}
        />
      </div>
    </div>
  );
}

const BasicsConfig: React.FC = () => {
  const { t } = useTranslation('settings/basics');

  return (
    <ConfigPageLayout className="bitfun-basics-config">
      <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
      <ConfigPageContent className="bitfun-basics-config__content">
        <BasicsAppearanceSection />
        <BasicsLaunchAtLoginSection />
        <BasicsLoggingSection />
        <BasicsTerminalSection />
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default BasicsConfig;
