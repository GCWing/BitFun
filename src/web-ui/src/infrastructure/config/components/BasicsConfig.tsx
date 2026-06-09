import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Archive, FolderOpen } from 'lucide-react';
import {
  Alert,
  Button,
  Select,
  Switch,
  Tooltip,
  ConfigPageLoading,
  ConfigPageMessage,
} from '@/component-library';
import { configAPI, workspaceAPI } from '@/infrastructure/api';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import type { CloseBehavior } from '@/infrastructure/api/service-api/SystemAPI';
import {
  getTerminalService,
  refreshTerminalPanelPosition,
  setTerminalPanelPosition,
} from '@/tools/terminal';
import type { ShellInfo } from '@/tools/terminal/types/session';
import {
  ConfigPageContent,
  ConfigPageHeader,
  ConfigPageLayout,
  ConfigPageSection,
  ConfigPageRow,
} from './common';
import { configManager } from '../services/ConfigManager';
import { createLogger } from '@/shared/utils/logger';
import type {
  BackendLogLevel,
  RuntimeLoggingInfo,
  TerminalConfig as TerminalSettings,
  TerminalPanelPosition,
} from '../types';
import './BasicsConfig.scss';

const log = createLogger('BasicsConfig');

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

function BasicsAutoUpdateSection() {
  const { t } = useTranslation('settings/basics');
  const isTauri = typeof window !== 'undefined' && '__TAURI__' in window;
  const [enabled, setEnabled] = useState(true);
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
        const v = await configManager.getConfig<boolean>('app.auto_update');
        if (!cancelled) {
          setEnabled(v !== false);
        }
      } catch (error) {
        log.error('Failed to load app.auto_update', error);
        if (!cancelled) {
          showMessage('error', t('autoUpdate.messages.loadFailed'));
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
        await configManager.setConfig('app.auto_update', next);
        configManager.clearCache();
        showMessage('success', t('autoUpdate.messages.saved'));
      } catch (error) {
        setEnabled(previous);
        log.error('Failed to set app.auto_update', { next, error });
        showMessage('error', t('autoUpdate.messages.saveFailed'));
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
    return <ConfigPageLoading text={t('autoUpdate.messages.loading')} />;
  }

  return (
    <div className="bitfun-auto-update-config">
      <div className="bitfun-auto-update-config__content">
        <ConfigPageMessage message={message} />
        <ConfigPageSection
          title={t('autoUpdate.sections.title')}
          description={t('autoUpdate.sections.hint')}
        >
          <ConfigPageRow
            label={t('autoUpdate.toggleLabel')}
            description={t('autoUpdate.toggleDescription')}
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
  const [includeSensitiveDiagnostics, setIncludeSensitiveDiagnostics] = useState(true);
  const [runtimeInfo, setRuntimeInfo] = useState<RuntimeLoggingInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [openingFolder, setOpeningFolder] = useState(false);
  const [exportingDiagnostics, setExportingDiagnostics] = useState(false);
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

      const [savedLevel, savedIncludeSensitiveDiagnostics, info] = await Promise.all([
        configManager.getConfig<BackendLogLevel>('app.logging.level'),
        configManager.getConfig<boolean>('app.logging.include_sensitive_diagnostics'),
        configAPI.getRuntimeLoggingInfo(),
      ]);

      setConfigLevel(savedLevel || info.effectiveLevel || 'info');
      setIncludeSensitiveDiagnostics(savedIncludeSensitiveDiagnostics ?? true);
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

  const handleSensitiveDiagnosticsChange = useCallback(
    async (checked: boolean) => {
      const previousValue = includeSensitiveDiagnostics;
      setIncludeSensitiveDiagnostics(checked);
      setSaving(true);

      try {
        await configManager.setConfig('app.logging.include_sensitive_diagnostics', checked);
        configManager.clearCache();
        showMessage('success', t('logging.messages.sensitiveDiagnosticsUpdated'));
      } catch (error) {
        setIncludeSensitiveDiagnostics(previousValue);
        log.error('Failed to update sensitive diagnostics logging preference', { checked, error });
        showMessage('error', t('logging.messages.saveFailed'));
      } finally {
        setSaving(false);
      }
    },
    [includeSensitiveDiagnostics, showMessage, t]
  );

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

  const handleExportDiagnostics = useCallback(async () => {
    try {
      setExportingDiagnostics(true);
      const result = await configAPI.exportDiagnosticsBundle();
      showMessage('success', t('logging.messages.diagnosticsExported'));
      await workspaceAPI.revealInExplorer(result.bundlePath);
    } catch (error) {
      log.error('Failed to export diagnostics bundle', { error });
      showMessage('error', t('logging.messages.diagnosticsExportFailed'));
    } finally {
      setExportingDiagnostics(false);
    }
  }, [showMessage, t]);

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
        >
          {runtimeInfo?.previousUnexpectedExit?.detected && (
            <Alert
              type="warning"
              message={t('logging.previousCrash.title')}
              description={t('logging.previousCrash.description', {
                path: runtimeInfo.previousUnexpectedExit.sessionLogDir || '-',
              })}
            />
          )}
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
            label={t('logging.sensitiveDiagnostics.label')}
            description={t('logging.sensitiveDiagnostics.description')}
            align="center"
          >
            <Switch
              checked={includeSensitiveDiagnostics}
              onChange={(e) => {
                void handleSensitiveDiagnosticsChange(e.target.checked);
              }}
              disabled={saving}
            />
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
          <ConfigPageRow
            label={t('logging.diagnostics.label')}
            description={t('logging.diagnostics.description')}
            align="center"
          >
            <Button
              type="button"
              variant="secondary"
              size="small"
              onClick={() => {
                void handleExportDiagnostics();
              }}
              isLoading={exportingDiagnostics}
              disabled={exportingDiagnostics}
            >
              <Archive size={14} />
              {t('logging.actions.exportDiagnostics')}
            </Button>
          </ConfigPageRow>
        </ConfigPageSection>
      </div>
    </div>
  );
}

function BasicsTerminalSection() {
  const { t } = useTranslation('settings/basics');
  const [defaultShell, setDefaultShell] = useState<string>('');
  const [terminalPanelPosition, setTerminalPanelPositionState] = useState<TerminalPanelPosition>('right');
  const [availableShells, setAvailableShells] = useState<ShellInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error' | 'info'; text: string } | null>(null);

  const showMessage = useCallback((type: 'success' | 'error' | 'info', text: string) => {
    setMessage({ type, text });
    setTimeout(() => setMessage(null), 3000);
  }, []);

  const loadData = useCallback(async () => {
    try {
      setLoading(true);

      const [terminalConfig, shells] = await Promise.all([
        configManager.getConfig<TerminalSettings>('terminal'),
        getTerminalService().getAvailableShells(),
      ]);

      setDefaultShell(terminalConfig?.default_shell || '');
      setTerminalPanelPositionState(terminalConfig?.terminal_panel_position === 'bottom' ? 'bottom' : 'right');
      void refreshTerminalPanelPosition();

      const availableOnly = shells.filter((s) => s.available);
      setAvailableShells(availableOnly);
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

  const handleTerminalPanelPositionChange = useCallback(
    async (value: TerminalPanelPosition) => {
      try {
        setSaving(true);
        setTerminalPanelPositionState(value);

        await setTerminalPanelPosition(value);
        configManager.clearCache();

        showMessage('success', t('terminal.messages.panelPositionUpdated'));
      } catch (error) {
        log.error('Failed to save terminal panel position', { value, error });
        showMessage('error', t('terminal.messages.saveFailed'));
      } finally {
        setSaving(false);
      }
    },
    [showMessage, t],
  );

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

  const terminalPanelPositionOptions = useMemo(
    () => [
      { value: 'right', label: t('terminal.panelPosition.options.right') },
      { value: 'bottom', label: t('terminal.panelPosition.options.bottom') },
    ],
    [t],
  );
  const shouldShowCmdFallbackNotice = defaultShell === 'Cmd';

  if (loading) {
    return <ConfigPageLoading text={t('terminal.messages.loading')} />;
  }

  return (
    <div className="bitfun-terminal-config">
      <div className="bitfun-terminal-config__content">
        <ConfigPageMessage message={message} />

        <ConfigPageSection
          title={t('terminal.sections.terminal')}
          description={t('terminal.sections.terminalHint')}
        >
          {shouldShowCmdFallbackNotice && (
            <Alert
              type="info"
              message={t('terminal.controls.cmdFallbackMessage')}
            />
          )}
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

          <ConfigPageRow
            label={t('terminal.panelPosition.label')}
            description={t('terminal.panelPosition.description')}
            align="center"
          >
            <div className="bitfun-terminal-config__select-wrapper">
              <Select
                value={terminalPanelPosition}
                onChange={(v) => handleTerminalPanelPositionChange(v as TerminalPanelPosition)}
                options={terminalPanelPositionOptions}
                placeholder={t('terminal.panelPosition.placeholder')}
                disabled={saving}
              />
            </div>
          </ConfigPageRow>
        </ConfigPageSection>
      </div>
    </div>
  );
}

function BasicsWindowBehaviorSection() {
  const { t } = useTranslation('settings/basics');
  const isTauri = typeof window !== 'undefined' && '__TAURI__' in window;
  const [behavior, setBehavior] = useState<CloseBehavior>('quit');
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error' | 'info'; text: string } | null>(null);

  const showMessage = useCallback((type: 'success' | 'error' | 'info', text: string) => {
    setMessage({ type, text });
    setTimeout(() => setMessage(null), 3000);
  }, []);

  const behaviorOptions = useMemo(
    () => [
      { value: 'quit', label: t('windowBehavior.options.quit') },
      { value: 'minimize_to_tray', label: t('windowBehavior.options.minimizeToTray') },
      { value: 'ask', label: t('windowBehavior.options.ask') },
    ],
    [t]
  );

  useEffect(() => {
    if (!isTauri) {
      setLoading(false);
      return;
    }
    let cancelled = false;
    void (async () => {
      try {
        setLoading(true);
        const v = await configManager.getConfig<CloseBehavior>('app.close_button_behavior');
        if (!cancelled) setBehavior(v ?? 'minimize_to_tray');
      } catch {
        // Key absent on first launch — fall back to default silently.
        if (!cancelled) setBehavior('minimize_to_tray');
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => { cancelled = true; };
  }, [isTauri, showMessage, t]);

  const handleChange = useCallback(
    async (value: string) => {
      const previous = behavior;
      const next = value as CloseBehavior;
      setBehavior(next);
      setSaving(true);
      try {
        await configManager.setConfig('app.close_button_behavior', next);
        configManager.clearCache();
        showMessage('success', t('windowBehavior.messages.saved'));
      } catch (error) {
        setBehavior(previous);
        log.error('Failed to save close behavior', { next, error });
        showMessage('error', t('windowBehavior.messages.saveFailed'));
      } finally {
        setSaving(false);
      }
    },
    [behavior, showMessage, t]
  );

  if (!isTauri) return null;

  if (loading) {
    return <ConfigPageLoading text={t('windowBehavior.messages.loading')} />;
  }

  return (
    <div className="bitfun-window-behavior-config">
      <div className="bitfun-window-behavior-config__content">
        <ConfigPageMessage message={message} />
        <ConfigPageSection
          title={t('windowBehavior.sections.title')}
          description={t('windowBehavior.sections.hint')}
        >
          <ConfigPageRow
            label={t('windowBehavior.closeButtonLabel')}
            description={t('windowBehavior.closeButtonDescription')}
            align="center"
          >
            <div className="bitfun-window-behavior-config__select-wrapper">
              <Select
                value={behavior}
                onChange={(v) => { void handleChange(v as string); }}
                options={behaviorOptions}
                disabled={saving}
              />
            </div>
          </ConfigPageRow>
        </ConfigPageSection>
      </div>
    </div>
  );
}

function BasicsNotificationsSection() {  const { t } = useTranslation('settings/basics');
  const [dialogNotify, setDialogNotify] = useState(true);
  const [startupTips, setStartupTips] = useState(true);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);

  useEffect(() => {
    void (async () => {
      try {
        const [notify, tips] = await Promise.all([
          configManager.getConfig<boolean>('app.notifications.dialog_completion_notify'),
          configManager.getConfig<boolean>('app.notifications.enable_startup_tips'),
        ]);
        setDialogNotify(notify !== false);
        setStartupTips(tips !== false);
      } catch {
        setDialogNotify(true);
        setStartupTips(true);
      }
    })();
  }, []);

  const handleDialogNotifyToggle = async (checked: boolean) => {
    setSaving(true);
    try {
      await configAPI.setConfig('app.notifications.dialog_completion_notify', checked);
      setDialogNotify(checked);
      setMessage({ type: 'success', text: t('notifications.messages.saveSuccess') });
    } catch {
      setMessage({ type: 'error', text: t('notifications.messages.saveFailed') });
    } finally {
      setSaving(false);
    }
  };

  const handleStartupTipsToggle = async (checked: boolean) => {
    setSaving(true);
    try {
      await configAPI.setConfig('app.notifications.enable_startup_tips', checked);
      setStartupTips(checked);
      setMessage({ type: 'success', text: t('notifications.messages.saveSuccess') });
    } catch {
      setMessage({ type: 'error', text: t('notifications.messages.saveFailed') });
    } finally {
      setSaving(false);
    }
  };

  return (
    <ConfigPageSection
      title={t('notifications.title')}
      description={t('notifications.hint')}
    >
      <ConfigPageMessage message={message} />
      <ConfigPageRow
        label={t('notifications.dialogCompletion.label')}
        description={t('notifications.dialogCompletion.description')}
        align="center"
      >
        <Switch
          checked={dialogNotify}
          onChange={(e) => { void handleDialogNotifyToggle(e.target.checked); }}
          disabled={saving}
        />
      </ConfigPageRow>
      <ConfigPageRow
        label={t('notifications.startupTips.label')}
        description={t('notifications.startupTips.description')}
        align="center"
      >
        <Switch
          checked={startupTips}
          onChange={(e) => { void handleStartupTipsToggle(e.target.checked); }}
          disabled={saving}
        />
      </ConfigPageRow>
    </ConfigPageSection>
  );
}

const BasicsConfig: React.FC = () => {
  const { t } = useTranslation('settings/basics');

  return (
    <ConfigPageLayout className="bitfun-basics-config">
      <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
      <ConfigPageContent className="bitfun-basics-config__content">
        <BasicsLaunchAtLoginSection />
        <BasicsAutoUpdateSection />
        <BasicsWindowBehaviorSection />
        <BasicsLoggingSection />
        <BasicsTerminalSection />
        <BasicsNotificationsSection />
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default BasicsConfig;
