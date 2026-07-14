import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Archive, FolderOpen, Plus, Trash2, Upload } from 'lucide-react';
import {
  Alert,
  Button,
  Input,
  Select,
  Switch,
  Tooltip,
  ConfigPageLoading,
  ConfigPageMessage,
} from '@/component-library';
import { configAPI, workspaceAPI } from '@/infrastructure/api';
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
import './BasicsConfig.scss';

const log = createLogger('BasicsConfig');

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

  const getFormattedLogPath = useCallback(() => {
    if (!runtimeInfo?.sessionLogDir) return '';
    return runtimeInfo.sessionLogDir.replace(
      '/data/storage/el2/base/files/bitfun',
      '/storage/Users/currentUser/appdata/el2/base/com.develop.opensource.ohpcd.bitfun/files/bitfun'
    );
  }, [runtimeInfo?.sessionLogDir]);

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
    const folder = getFormattedLogPath();
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
  }, [getFormattedLogPath, showMessage, t]);

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
                {getFormattedLogPath() || '-'}
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

interface EnvVarRow {
  id: string;
  key: string;
  value: string;
}

let envVarRowSeq = 0;
const newEnvVarRowId = (): string => `envvar-${Date.now()}-${envVarRowSeq++}`;

/**
 * Parse environment-variable text (e.g. a `.env` file) into a key/value map.
 * Supports `KEY=VALUE`, `export KEY=VALUE`, and `KEY: VALUE` lines. Skips blank
 * and `#`-comment lines, strips surrounding quotes, and reports unparseable lines.
 */
function parseEnvText(text: string): { parsed: Record<string, string>; skipped: number } {
  const parsed: Record<string, string> = {};
  let skipped = 0;
  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (line === '' || line.startsWith('#')) continue;
    let work = line;
    if (/^export\s+/.test(work)) {
      work = work.replace(/^export\s+/, '');
    }
    const match = work.match(/^([^=:]+)[=:](.*)$/);
    if (!match) {
      skipped += 1;
      continue;
    }
    const key = match[1].trim();
    let value = match[2].trim();
    if (
      (value.startsWith('"') && value.endsWith('"')) ||
      (value.startsWith("'") && value.endsWith("'"))
    ) {
      value = value.slice(1, -1);
    }
    if (key === '') {
      skipped += 1;
      continue;
    }
    if (key.toUpperCase() === 'PATH') {
      skipped += 1;
      continue;
    }
    parsed[key] = value;
  }
  return { parsed, skipped };
}

function BasicsEnvVarsSection() {
  const { t } = useTranslation('settings/basics');
  const isTauri = typeof window !== 'undefined' && '__TAURI__' in window;
  const [rows, setRows] = useState<EnvVarRow[]>([]);
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
      const terminalConfig = await configManager.getConfig<TerminalSettings>('terminal');
      const envVars = terminalConfig?.env_vars ?? {};
      const next: EnvVarRow[] = Object.keys(envVars)
        .sort((a, b) => a.localeCompare(b))
        .map((key) => ({ id: newEnvVarRowId(), key, value: envVars[key] ?? '' }));
      setRows(next);
    } catch (error) {
      log.error('Failed to load terminal env vars', error);
      showMessage('error', t('terminal.envVars.messages.loadFailed'));
    } finally {
      setLoading(false);
    }
  }, [showMessage, t]);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  const handleAddRow = useCallback(() => {
    setRows((prev) => [...prev, { id: newEnvVarRowId(), key: '', value: '' }]);
  }, []);

  const handleRemoveRow = useCallback((id: string) => {
    setRows((prev) => prev.filter((r) => r.id !== id));
  }, []);

  const handleRowChange = useCallback((id: string, field: 'key' | 'value', val: string) => {
    setRows((prev) => prev.map((r) => (r.id === id ? { ...r, [field]: val } : r)));
  }, []);

  const buildRecord = useCallback((source: EnvVarRow[]): Record<string, string> => {
    const record: Record<string, string> = {};
    for (const row of source) {
      const key = row.key.trim();
      if (key === '') continue;
      record[key] = row.value;
    }
    return record;
  }, []);

  const persist = useCallback(
    async (source: EnvVarRow[]): Promise<void> => {
      const record = buildRecord(source);
      await configManager.setConfig<Record<string, string>>('terminal.env_vars', record);
      configManager.clearCache();
    },
    [buildRecord]
  );

  const handleSave = useCallback(async () => {
    try {
      setSaving(true);
      await persist(rows);
      showMessage('success', t('terminal.envVars.messages.saved'));
    } catch (error) {
      log.error('Failed to save terminal env vars', error);
      showMessage('error', t('terminal.envVars.messages.saveFailed'));
    } finally {
      setSaving(false);
    }
  }, [persist, rows, showMessage, t]);

  const handleImportFile = useCallback(async () => {
    try {
      const filePath = await workspaceAPI.open_oh_file_dialog();
      if (!filePath) return;
      const text = await workspaceAPI.readFileContent(filePath);
      const { parsed, skipped } = parseEnvText(text);
      const parsedCount = Object.keys(parsed).length;
      if (parsedCount === 0) {
        showMessage('error', t('terminal.envVars.messages.importFailed'));
        return;
      }
      const merged = (() => {
        const byKey = new Map<string, EnvVarRow>();
        for (const r of rows) byKey.set(r.key.trim(), r);
        for (const [k, v] of Object.entries(parsed)) {
          const existing = byKey.get(k);
          if (existing) {
            existing.value = v;
          } else {
            byKey.set(k, { id: newEnvVarRowId(), key: k, value: v });
          }
        }
        return Array.from(byKey.values()).sort((a, b) => a.key.localeCompare(b.key));
      })();
      setRows(merged);
      await persist(merged);
      showMessage(
        'success',
        skipped > 0
          ? t('terminal.envVars.messages.importedWithSkipped', { count: parsedCount, skipped })
          : t('terminal.envVars.messages.imported', { count: parsedCount })
      );
    } catch (error) {
      log.error('Failed to import env vars from file', error);
      showMessage('error', t('terminal.envVars.messages.importFailed'));
    }
  }, [persist, rows, showMessage, t]);

  if (!isTauri) return null;

  if (loading) {
    return <ConfigPageLoading text={t('terminal.envVars.messages.loading')} />;
  }

  return (
    <div className="bitfun-env-vars-config">
      <div className="bitfun-env-vars-config__content">
        <ConfigPageMessage message={message} />
        <ConfigPageSection
          title={t('terminal.envVars.title')}
          description={t('terminal.envVars.hint')}
          extra={
            <div className="bitfun-env-vars-config__actions">
              <Button
                variant="secondary"
                size="small"
                onClick={() => { void handleImportFile(); }}
                disabled={saving}
              >
                <Upload size={14} />
                <span>{t('terminal.envVars.importFile.button')}</span>
              </Button>
              <Button
                variant="primary"
                size="small"
                onClick={() => { void handleSave(); }}
                disabled={saving}
              >
                {t('terminal.envVars.save')}
              </Button>
            </div>
          }
        >
          {rows.length === 0 ? (
            <div className="bitfun-env-vars-config__empty">{t('terminal.envVars.empty')}</div>
          ) : (
            <div className="bitfun-env-vars-config__table">
              <div className="bitfun-env-vars-config__row bitfun-env-vars-config__row--header">
                <div className="bitfun-env-vars-config__cell bitfun-env-vars-config__cell--key">
                  {t('terminal.envVars.columns.key')}
                </div>
                <div className="bitfun-env-vars-config__cell bitfun-env-vars-config__cell--value">
                  {t('terminal.envVars.columns.value')}
                </div>
                <div className="bitfun-env-vars-config__cell bitfun-env-vars-config__cell--action" />
              </div>
              {rows.map((row) => (
                <div className="bitfun-env-vars-config__row" key={row.id}>
                  <div className="bitfun-env-vars-config__cell bitfun-env-vars-config__cell--key">
                    <Input
                      value={row.key}
                      onChange={(e) => handleRowChange(row.id, 'key', e.target.value)}
                      placeholder={t('terminal.envVars.columns.keyPlaceholder')}
                      disabled={saving}
                      inputSize="small"
                    />
                  </div>
                  <div className="bitfun-env-vars-config__cell bitfun-env-vars-config__cell--value">
                    <Input
                      value={row.value}
                      onChange={(e) => handleRowChange(row.id, 'value', e.target.value)}
                      placeholder={t('terminal.envVars.columns.valuePlaceholder')}
                      disabled={saving}
                      inputSize="small"
                    />
                  </div>
                  <div className="bitfun-env-vars-config__cell bitfun-env-vars-config__cell--action">
                    <Tooltip content={t('terminal.envVars.actions.delete')}>
                      <Button
                        variant="ghost"
                        size="small"
                        onClick={() => handleRemoveRow(row.id)}
                        disabled={saving}
                        aria-label={t('terminal.envVars.actions.delete')}
                      >
                        <Trash2 size={14} />
                      </Button>
                    </Tooltip>
                  </div>
                </div>
              ))}
            </div>
          )}
          <div className="bitfun-env-vars-config__footer">
            <Button
              variant="ghost"
              size="small"
              onClick={handleAddRow}
              disabled={saving}
            >
              <Plus size={14} />
              <span>{t('terminal.envVars.actions.addRow')}</span>
            </Button>
          </div>
        </ConfigPageSection>
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
        <BasicsAutoUpdateSection />
        <BasicsWindowBehaviorSection />
        <BasicsLoggingSection />
        <BasicsTerminalSection />
        <BasicsEnvVarsSection />
        <BasicsNotificationsSection />
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default BasicsConfig;
