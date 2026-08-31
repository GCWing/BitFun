import { Alert, Button, Combobox, Select, Switch, Tooltip, type ComboboxOption } from '@bitfun/ui';
import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Archive, FolderOpen } from 'lucide-react';
import { ConfigLoadingState, ConfigMessage } from '@/infrastructure/config/components/common';
import { configAPI, workspaceAPI } from '@/infrastructure/api';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import type { CloseBehavior } from '@/infrastructure/api/service-api/SystemAPI';
import {
  getTerminalService,
  refreshTerminalPanelPosition,
  setTerminalPanelPosition,
} from '@/tools/terminal/services';
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
import './ApplicationSettingsPages.scss';

const log = createLogger('ApplicationSettings');

type TerminalShellOption = ComboboxOption & {
  shell?: ShellInfo;
};

const formatShellLabel = (shell: ShellInfo): string =>
  `${shell.name}${shell.version ? ` (${shell.version})` : ''}`;

function LaunchAtLoginSection() {
  const { t } = useTranslation('settings/application');
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
    return <ConfigLoadingState label={t('launchAtLogin.messages.loading')} />;
  }

  return (
    <div className="bitfun-launch-at-login-config" data-bf-component="application-settings" data-bf-part="launchAtLogin">
      <div className="bitfun-launch-at-login-config__content">
        <ConfigMessage message={message} />
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

function AutoUpdateSection() {
  const { t } = useTranslation('settings/application');
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
    return <ConfigLoadingState label={t('autoUpdate.messages.loading')} />;
  }

  return (
    <div className="bitfun-auto-update-config" data-bf-component="application-settings" data-bf-part="autoUpdate">
      <div className="bitfun-auto-update-config__content">
        <ConfigMessage message={message} />
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

function PreventSleepSection() {
  const { t } = useTranslation('settings/application');
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
        const value = await systemAPI.getPreventSleepEnabled();
        if (!cancelled) {
          setEnabled(value);
        }
      } catch (error) {
        log.error('Failed to load prevent-sleep preference', error);
        if (!cancelled) {
          showMessage('error', t('preventSleep.messages.loadFailed'));
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
        await systemAPI.setPreventSleepEnabled(next);
        showMessage('success', t('preventSleep.messages.saved'));
      } catch (error) {
        setEnabled(previous);
        log.error('Failed to set prevent-sleep preference', { next, error });
        showMessage('error', t('preventSleep.messages.saveFailed'));
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
    return <ConfigLoadingState label={t('preventSleep.messages.loading')} />;
  }

  return (
    <ConfigPageSection
      title={t('preventSleep.sections.title')}
      description={t('preventSleep.sections.hint')}
    >
      <ConfigMessage message={message} />
      <ConfigPageRow
        label={t('preventSleep.toggleLabel')}
        description={t('preventSleep.toggleDescription')}
        align="center"
      >
        <Switch
          checked={enabled}
          onChange={(event) => {
            void handleToggle(event.target.checked);
          }}
          disabled={saving}
        />
      </ConfigPageRow>
    </ConfigPageSection>
  );
}

function LoggingSection() {
  const { t } = useTranslation('settings/application');
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
    return <ConfigLoadingState label={t('logging.messages.loading')} />;
  }

  return (
    <div className="bitfun-logging-config" data-bf-component="application-settings" data-bf-part="logging">
      <div className="bitfun-logging-config__content">
        <ConfigMessage message={message} />

        <ConfigPageSection
          title={t('logging.sections.logging')}
          description={t('logging.sections.loggingHint')}
        >
          {runtimeInfo?.previousUnexpectedExit?.detected && (
            <Alert
              tone={runtimeInfo.previousUnexpectedExit.category === 'crash' ? 'warning' : 'info'}
              message={t(
                runtimeInfo.previousUnexpectedExit.category === 'crash'
                  ? 'logging.previousCrash.title'
                  : 'logging.previousUncleanShutdown.title'
              )}
              description={t(
                runtimeInfo.previousUnexpectedExit.category === 'crash'
                  ? 'logging.previousCrash.description'
                  : 'logging.previousUncleanShutdown.description',
                {
                  path: runtimeInfo.previousUnexpectedExit.sessionLogDir || '-',
                }
              )}
            />
          )}
          <ConfigPageRow
            label={t('logging.sections.level')}
            description={t('logging.level.description')}
            align="center"
          >
            <Select
              value={configLevel}
              onValueChange={(v) => handleLevelChange(v as string)}
              options={levelOptions}
              disabled={saving}
            />
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
            <div className="bitfun-logging-config__path-row" data-bf-component="application-settings" data-bf-part="logPath">
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
              variant="outline"
              size="md"
              leadingIcon={<Archive />}
              data-testid="diagnostics-export-button"
              onClick={() => {
                void handleExportDiagnostics();
              }}
              loading={exportingDiagnostics}
              disabled={exportingDiagnostics}
            >
              {t('logging.actions.exportDiagnostics')}
            </Button>
          </ConfigPageRow>
        </ConfigPageSection>
      </div>
    </div>
  );
}

function TerminalSection() {
  const { t } = useTranslation('settings/application');
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

  const shellOptions = useMemo<TerminalShellOption[]>(
    () => [
      { value: '', label: t('terminal.controls.autoDetect') },
      ...availableShells.map((shell) => ({
        description: shell.path,
        value: shell.path,
        label: formatShellLabel(shell),
        shell,
      })),
    ],
    [availableShells, t],
  );

  const selectedShell = useMemo(
    () =>
      availableShells.find((shell) => shell.path === defaultShell) ??
      availableShells.find((shell) => shell.shellType === defaultShell),
    [availableShells, defaultShell],
  );
  const selectedShellValue = selectedShell?.path ?? defaultShell;

  const terminalPanelPositionOptions = useMemo(
    () => [
      { value: 'right', label: t('terminal.panelPosition.options.right') },
      { value: 'bottom', label: t('terminal.panelPosition.options.bottom') },
    ],
    [t],
  );
  const shouldShowCmdFallbackNotice = selectedShell?.shellType === 'Cmd' || defaultShell === 'Cmd';

  if (loading) {
    return <ConfigLoadingState label={t('terminal.messages.loading')} />;
  }

  return (
    <div className="bitfun-terminal-config" data-bf-component="application-settings" data-bf-part="terminal">
      <div className="bitfun-terminal-config__content">
        <ConfigMessage message={message} />

        <ConfigPageSection
          title={t('terminal.sections.terminal')}
          description={t('terminal.sections.terminalHint')}
        >
          {shouldShowCmdFallbackNotice && (
            <Alert
              tone="info"
              message={t('terminal.controls.cmdFallbackMessage')}
            />
          )}
          <ConfigPageRow
            label={t('terminal.sections.defaultTerminal')}
            description={t('terminal.controls.description')}
            align="center"
          >
            {availableShells.length > 0 ? (
              <Combobox
                value={selectedShellValue}
                onValueChange={(v) => handleShellChange(v as string)}
                options={shellOptions}
                placeholder={t('terminal.controls.placeholder')}
                disabled={saving}
              />
            ) : (
              <div className="bitfun-terminal-config__no-shells">{t('terminal.controls.noShells')}</div>
            )}
          </ConfigPageRow>

          <ConfigPageRow
            label={t('terminal.panelPosition.label')}
            description={t('terminal.panelPosition.description')}
            align="center"
          >
            <Select
              value={terminalPanelPosition}
              onValueChange={(v) => handleTerminalPanelPositionChange(v as TerminalPanelPosition)}
              options={terminalPanelPositionOptions}
              placeholder={t('terminal.panelPosition.placeholder')}
              disabled={saving}
            />
          </ConfigPageRow>
        </ConfigPageSection>
      </div>
    </div>
  );
}

function WindowBehaviorSection() {
  const { t } = useTranslation('settings/application');
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
    return <ConfigLoadingState label={t('windowBehavior.messages.loading')} />;
  }

  return (
    <div className="bitfun-window-behavior-config" data-bf-component="application-settings" data-bf-part="windowBehavior">
      <div className="bitfun-window-behavior-config__content">
        <ConfigMessage message={message} />
        <ConfigPageSection
          title={t('windowBehavior.sections.title')}
          description={t('windowBehavior.sections.hint')}
        >
          <ConfigPageRow
            label={t('windowBehavior.closeButtonLabel')}
            description={t('windowBehavior.closeButtonDescription')}
            align="center"
          >
            <Select
              value={behavior}
              onValueChange={(v) => { void handleChange(v as string); }}
              options={behaviorOptions}
              disabled={saving}
            />
          </ConfigPageRow>
        </ConfigPageSection>
      </div>
    </div>
  );
}

function NotificationsSection() {
  const { t } = useTranslation('settings/application');
  const [dialogNotify, setDialogNotify] = useState(true);
  const [permissionRequestNotify, setPermissionRequestNotify] = useState(true);
  const [startupTips, setStartupTips] = useState(true);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);

  useEffect(() => {
    void (async () => {
      try {
        const [notify, permissionNotify, tips] = await Promise.all([
          configManager.getConfig<boolean>('app.notifications.dialog_completion_notify'),
          configManager.getConfig<boolean>('app.notifications.permission_request_notify'),
          configManager.getConfig<boolean>('app.notifications.enable_startup_tips'),
        ]);
        setDialogNotify(notify !== false);
        setPermissionRequestNotify(permissionNotify !== false);
        setStartupTips(tips !== false);
      } catch {
        setDialogNotify(true);
        setPermissionRequestNotify(true);
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

  const handlePermissionRequestNotifyToggle = async (checked: boolean) => {
    setSaving(true);
    try {
      await configManager.setConfig('app.notifications.permission_request_notify', checked);
      setPermissionRequestNotify(checked);
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
      data-bf-component="application-settings"
      data-bf-part="notifications"
    >
      <ConfigMessage message={message} />
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
        label={t('notifications.permissionRequest.label')}
        description={t('notifications.permissionRequest.description')}
        align="center"
      >
        <Switch
          checked={permissionRequestNotify}
          onChange={(e) => { void handlePermissionRequestNotifyToggle(e.target.checked); }}
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

interface ApplicationSettingsPageProps {
  page: 'general' | 'terminal' | 'diagnostics';
}

const ApplicationSettingsPage: React.FC<ApplicationSettingsPageProps> = ({ page }) => {
  const { t } = useTranslation('settings');

  const title = t(`navigation.pages.${page}.label`);
  const subtitle = t(`navigation.pages.${page}.description`);

  return (
    <ConfigPageLayout
      className="bitfun-application-settings"
      data-bf-component="application-settings"
      data-bf-part="root"
      data-bf-view={page}
    >
      <ConfigPageHeader title={title} subtitle={subtitle} />
      <ConfigPageContent
        className="bitfun-application-settings__content"
        data-bf-component="application-settings"
        data-bf-part="content"
      >
        {page === 'general' ? (
          <>
            <LaunchAtLoginSection />
            <PreventSleepSection />
            <AutoUpdateSection />
            <WindowBehaviorSection />
            <NotificationsSection />
          </>
        ) : null}
        {page === 'terminal' ? <TerminalSection /> : null}
        {page === 'diagnostics' ? <LoggingSection /> : null}
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export function GeneralSettingsPage(): React.ReactElement {
  return <ApplicationSettingsPage page="general" />;
}

export function TerminalSettingsPage(): React.ReactElement {
  return <ApplicationSettingsPage page="terminal" />;
}

export function DiagnosticsSettingsPage(): React.ReactElement {
  return <ApplicationSettingsPage page="diagnostics" />;
}
