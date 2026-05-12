import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Bot,
  Download,
  ExternalLink,
  FileJson,
  LoaderCircle,
  Plus,
  RefreshCw,
  Save,
  Search,
  Terminal,
} from 'lucide-react';
import { Button, Input, Select, Textarea } from '@/component-library';
import {
  ConfigPageContent,
  ConfigPageHeader,
  ConfigPageLayout,
  ConfigPageSection,
} from './common';
import {
  ACPClientAPI,
  type AcpClientInfo,
  type AcpClientPermissionMode,
  type AcpClientRequirementProbe,
  type AcpRequirementProbeItem,
} from '../../api/service-api/ACPClientAPI';
import { systemAPI } from '../../api/service-api/SystemAPI';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import './AcpAgentsConfig.scss';

const log = createLogger('AcpAgentsConfig');

interface AcpClientConfig {
  name?: string;
  command: string;
  args: string[];
  env: Record<string, string>;
  enabled: boolean;
  readonly: boolean;
  permissionMode: AcpClientPermissionMode;
}

interface AcpClientConfigFile {
  acpClients: Record<string, AcpClientConfig>;
}

interface AcpClientPreset {
  id: string;
  name: string;
  description: string;
  version?: string;
  command: string;
  args: string[];
}

const PRESETS: AcpClientPreset[] = [
  {
    id: 'opencode',
    name: 'opencode',
    description: 'AI coding agent with native ACP support.',
    command: 'opencode',
    args: ['acp'],
  },
  {
    id: 'claude-code',
    name: 'Claude Code',
    description: 'Claude Code connected through the Zed ACP adapter.',
    command: 'npx',
    args: ['--yes', '@zed-industries/claude-code-acp@latest'],
  },
  {
    id: 'codex',
    name: 'Codex',
    description: 'OpenAI Codex CLI connected through the Zed ACP adapter.',
    command: 'npx',
    args: ['--yes', '@zed-industries/codex-acp@latest'],
  },
];

const PRESET_BY_ID = new Map(PRESETS.map(preset => [preset.id, preset]));
let requirementProbeCache: AcpClientRequirementProbe[] | null = null;
let requirementProbeInFlight: Promise<AcpClientRequirementProbe[]> | null = null;

function loadRequirementProbes(options: { force?: boolean } = {}): Promise<AcpClientRequirementProbe[]> {
  if (!options.force && requirementProbeCache) {
    return Promise.resolve(requirementProbeCache);
  }

  if (!options.force && requirementProbeInFlight) {
    return requirementProbeInFlight;
  }

  requirementProbeInFlight = ACPClientAPI.probeClientRequirements({ force: options.force })
    .then((probes) => {
      requirementProbeCache = probes;
      return probes;
    })
    .finally(() => {
      requirementProbeInFlight = null;
    });

  return requirementProbeInFlight;
}

function defaultConfigForPreset(preset: AcpClientPreset): AcpClientConfig {
  return {
    name: preset.name,
    command: preset.command,
    args: preset.args,
    env: {},
    enabled: true,
    readonly: false,
    permissionMode: 'ask',
  };
}

function normalizeConfigValue(value: unknown): AcpClientConfigFile {
  const candidate = value && typeof value === 'object' ? value as Record<string, unknown> : {};
  const rawClients = (
    candidate.acpClients && typeof candidate.acpClients === 'object' && !Array.isArray(candidate.acpClients)
  )
    ? candidate.acpClients as Record<string, unknown>
    : candidate;

  const acpClients: Record<string, AcpClientConfig> = {};
  for (const [id, rawConfig] of Object.entries(rawClients)) {
    if (!rawConfig || typeof rawConfig !== 'object' || Array.isArray(rawConfig)) {
      continue;
    }

    const item = rawConfig as Record<string, unknown>;
    const command = typeof item.command === 'string' ? item.command.trim() : '';
    if (!command) {
      continue;
    }

    acpClients[id] = {
      name: typeof item.name === 'string' ? item.name : undefined,
      command,
      args: Array.isArray(item.args) ? item.args.map(String) : [],
      env: normalizeEnvObject(item.env),
      enabled: item.enabled !== false,
      readonly: item.readonly === true,
      permissionMode: normalizePermissionMode(item.permissionMode),
    };
  }

  return { acpClients };
}

function normalizeEnvObject(value: unknown): Record<string, string> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {};
  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>).map(([key, envValue]) => [key, String(envValue)])
  );
}

function normalizePermissionMode(value: unknown): AcpClientPermissionMode {
  return value === 'allow_once' || value === 'reject_once' ? value : 'ask';
}

function formatConfig(config: AcpClientConfigFile): string {
  return JSON.stringify(config, null, 2);
}

function parseEnvText(value: string): Record<string, string> {
  const env: Record<string, string> = {};
  for (const rawLine of value.split('\n')) {
    const line = rawLine.trim();
    if (!line) continue;
    const separator = line.indexOf('=');
    if (separator <= 0) {
      throw new Error(`Invalid env line: ${line}`);
    }
    env[line.slice(0, separator).trim()] = line.slice(separator + 1);
  }
  return env;
}

function formatEnv(env: Record<string, string>): string {
  return Object.entries(env).map(([key, value]) => `${key}=${value}`).join('\n');
}

function requirementTone(item?: AcpRequirementProbeItem): 'ok' | 'error' | 'muted' {
  if (!item) return 'muted';
  return item.installed ? 'ok' : 'error';
}

type RegistryFilter = 'all' | 'installed' | 'not_installed' | 'invalid';
type AgentRowStatus = 'enabled' | 'ready' | 'not_installed' | 'invalid' | 'checking';

function getAgentRowStatus({
  configured,
  enabled,
  runnable,
  probePending,
}: {
  configured: boolean;
  enabled: boolean;
  runnable?: boolean;
  probePending: boolean;
}): AgentRowStatus {
  if (probePending) return 'checking';
  if (!configured) return runnable ? 'ready' : 'not_installed';
  if (!enabled) return 'invalid';
  return runnable === false ? 'invalid' : 'enabled';
}

function CapabilityBadge({
  icon,
  item,
  label,
  checking,
  installedText,
  missingText,
  checkingText,
}: {
  icon: React.ReactNode;
  item?: AcpRequirementProbeItem;
  label: string;
  checking?: boolean;
  installedText: string;
  missingText: string;
  checkingText: string;
}) {
  const tone = item ? requirementTone(item) : 'muted';
  const title = item
    ? [label, item.installed ? installedText : missingText, item.path, item.version, item.error]
      .filter(Boolean)
      .join('\n')
    : checking ? `${label}\n${checkingText}` : label;

  return (
    <span
      className={`bitfun-acp-agents__capability is-${tone}`}
      title={title}
    >
      {icon}
      <span>{label}</span>
    </span>
  );
}

function AgentStatusBadge({
  status,
  label,
}: {
  status: AgentRowStatus;
  label: string;
}) {
  return (
    <span className={`bitfun-acp-agents__status is-${status}`}>
      {status === 'checking' && <LoaderCircle size={12} />}
      <span>{label}</span>
    </span>
  );
}

const AcpAgentsConfig: React.FC = () => {
  const { t } = useTranslation('settings/acp-agents');
  const { error: notifyError, success: notifySuccess } = useNotification();
  const jsonEditorRef = useRef<HTMLTextAreaElement>(null);

  const [config, setConfig] = useState<AcpClientConfigFile>({ acpClients: {} });
  const [clients, setClients] = useState<AcpClientInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [showJsonEditor, setShowJsonEditor] = useState(false);
  const [jsonConfig, setJsonConfig] = useState('');
  const [envDrafts, setEnvDrafts] = useState<Record<string, string>>({});
  const [requirementProbes, setRequirementProbes] = useState<AcpClientRequirementProbe[]>(
    requirementProbeCache ?? []
  );
  const [probingRequirements, setProbingRequirements] = useState(false);
  const [registrySearch, setRegistrySearch] = useState('');
  const [registryFilter, setRegistryFilter] = useState<RegistryFilter>('all');
  const [installingClientIds, setInstallingClientIds] = useState<Set<string>>(() => new Set());
  const requirementProbeRequestIdRef = useRef(0);

  const clientsById = useMemo(() => new Map(clients.map(client => [client.id, client])), [clients]);
  const probesById = useMemo(
    () => new Map(requirementProbes.map(probe => [probe.id, probe])),
    [requirementProbes]
  );
  const customClientRows = useMemo(() => {
    const ids = new Set<string>([
      ...Object.keys(config.acpClients),
      ...clients.map(client => client.id),
    ]);

    return Array.from(ids)
      .filter(id => !PRESET_BY_ID.has(id))
      .sort((a, b) => a.localeCompare(b));
  }, [clients, config.acpClients]);

  const registryPresets = useMemo(() => {
    const search = registrySearch.trim().toLowerCase();
    return PRESETS.filter(preset => {
      const probe = probesById.get(preset.id);
      const probePending = probingRequirements && !probe;
      const configured = Boolean(config.acpClients[preset.id]);
      const enabled = config.acpClients[preset.id]?.enabled ?? clientsById.get(preset.id)?.enabled ?? false;
      const status = getAgentRowStatus({
        configured,
        enabled,
        runnable: probe?.runnable === true,
        probePending,
      });
      if (registryFilter === 'installed' && status !== 'enabled' && status !== 'ready') return false;
      if (registryFilter === 'not_installed' && status !== 'not_installed') return false;
      if (registryFilter === 'invalid' && status !== 'invalid') return false;
      if (!search) return true;
      return [
        preset.name,
        preset.id,
        preset.description,
        preset.command,
        ...preset.args,
      ].join(' ').toLowerCase().includes(search);
    });
  }, [clientsById, config.acpClients, probesById, probingRequirements, registryFilter, registrySearch]);

  const visibleCustomClientRows = useMemo(() => {
    const search = registrySearch.trim().toLowerCase();
    return customClientRows.filter(clientId => {
      const clientConfig = config.acpClients[clientId];
      const clientInfo = clientsById.get(clientId);
      const requirementProbe = probesById.get(clientId);
      const probePending = probingRequirements && !requirementProbe;
      const configured = Boolean(clientConfig || clientInfo);
      const enabled = clientConfig?.enabled ?? clientInfo?.enabled ?? false;
      const status = getAgentRowStatus({
        configured,
        enabled,
        runnable: requirementProbe?.runnable === true,
        probePending,
      });
      if (registryFilter === 'installed' && status !== 'enabled' && status !== 'ready') return false;
      if (registryFilter === 'not_installed' && status !== 'not_installed') return false;
      if (registryFilter === 'invalid' && status !== 'invalid') return false;
      if (!search) return true;
      return [
        clientId,
        clientConfig?.name,
        clientInfo?.name,
        clientConfig?.command,
        ...(clientConfig?.args ?? []),
      ].filter(Boolean).join(' ').toLowerCase().includes(search);
    });
  }, [clientsById, config.acpClients, customClientRows, probesById, probingRequirements, registryFilter, registrySearch]);

  const refreshRequirementProbes = useCallback(async (
    options: { force?: boolean; notifyOnError?: boolean } = {}
  ) => {
    if (!options.force && requirementProbeCache) {
      setRequirementProbes(requirementProbeCache);
      setProbingRequirements(false);
      return;
    }

    const requestId = ++requirementProbeRequestIdRef.current;
    setProbingRequirements(true);
    try {
      const nextRequirementProbes = await loadRequirementProbes({ force: options.force });
      if (requirementProbeRequestIdRef.current === requestId) {
        setRequirementProbes(nextRequirementProbes);
      }
    } catch (error) {
      log.error('Failed to probe ACP agent requirements', error);
      if (options.notifyOnError ?? true) {
        notifyError(error instanceof Error ? error.message : String(error), {
          title: t('notifications.probeFailed'),
        });
      }
    } finally {
      if (requirementProbeRequestIdRef.current === requestId) {
        setProbingRequirements(false);
      }
    }
  }, [notifyError, t]);

  const loadConfig = useCallback(async (
    options: { showLoading?: boolean; refreshRequirements?: boolean } = {}
  ) => {
    const showLoading = options.showLoading ?? true;
    const refreshRequirements = options.refreshRequirements ?? false;
    try {
      if (showLoading) {
        setLoading(true);
      }
      const [rawConfig, nextClients] = await Promise.all([
        ACPClientAPI.loadJsonConfig(),
        ACPClientAPI.getClients(),
      ]);
      const parsed = normalizeConfigValue(JSON.parse(rawConfig || '{}'));
      setConfig(parsed);
      setJsonConfig(formatConfig(parsed));
      setEnvDrafts(
        Object.fromEntries(
          Object.entries(parsed.acpClients).map(([clientId, clientConfig]) => [
            clientId,
            formatEnv(clientConfig.env),
          ])
        )
      );
      setClients(nextClients);
      setDirty(false);
      if (refreshRequirements) {
        void refreshRequirementProbes({ notifyOnError: false });
      }
    } catch (error) {
      log.error('Failed to load ACP agent config', error);
      notifyError(error instanceof Error ? error.message : String(error), {
        title: t('notifications.loadFailed'),
      });
    } finally {
      if (showLoading) {
        setLoading(false);
      }
    }
  }, [notifyError, refreshRequirementProbes, t]);

  useEffect(() => {
    void loadConfig();
  }, [loadConfig]);

  useEffect(() => {
    const handleAcpClientsChanged = () => {
      void loadConfig({ showLoading: false });
    };
    window.addEventListener('bitfun:acp-clients-changed', handleAcpClientsChanged);
    return () => {
      window.removeEventListener('bitfun:acp-clients-changed', handleAcpClientsChanged);
    };
  }, [loadConfig]);

  const patchClientConfig = (clientId: string, patch: Partial<AcpClientConfig>) => {
    setConfig(prev => {
      const preset = PRESET_BY_ID.get(clientId);
      const current = prev.acpClients[clientId] ??
        (preset ? defaultConfigForPreset(preset) : undefined);
      if (!current) return prev;

      const next = {
        acpClients: {
          ...prev.acpClients,
          [clientId]: {
            ...current,
            ...patch,
          },
        },
      };
      setJsonConfig(formatConfig(next));
      return next;
    });
    setDirty(true);
  };

  const installPresetClient = async (preset: AcpClientPreset) => {
    setInstallingClientIds(prev => new Set(prev).add(preset.id));
    try {
      await ACPClientAPI.installClientCli({ clientId: preset.id });
      requirementProbeCache = null;
      await refreshRequirementProbes({ force: true, notifyOnError: false });
      notifySuccess(t('notifications.downloadSuccess'));
    } catch (error) {
      log.error('Failed to download ACP agent CLI', error);
      notifyError(error instanceof Error ? error.message : String(error), {
        title: t('notifications.downloadFailed'),
      });
    } finally {
      setInstallingClientIds(prev => {
        const next = new Set(prev);
        next.delete(preset.id);
        return next;
      });
    }
  };

  const mergeEnvDrafts = (baseConfig: AcpClientConfigFile): AcpClientConfigFile => ({
    acpClients: Object.fromEntries(
      Object.entries(baseConfig.acpClients).map(([clientId, clientConfig]) => [
        clientId,
        {
          ...clientConfig,
          env: envDrafts[clientId] !== undefined
            ? parseEnvText(envDrafts[clientId])
            : clientConfig.env,
        },
      ])
    ),
  });

  const saveConfig = async (nextConfig = config, options: { mergeEnvDrafts?: boolean } = {}) => {
    try {
      setSaving(true);
      const configToSave = options.mergeEnvDrafts === false
        ? nextConfig
        : mergeEnvDrafts(nextConfig);
      await ACPClientAPI.saveJsonConfig(formatConfig(configToSave));
      const nextClients = await ACPClientAPI.getClients();
      setClients(nextClients);
      setConfig(configToSave);
      setJsonConfig(formatConfig(configToSave));
      setDirty(false);
      requirementProbeCache = null;
      setRequirementProbes([]);
      notifySuccess(t('notifications.saveSuccess'));
    } catch (error) {
      log.error('Failed to save ACP agent config', error);
      notifyError(error instanceof Error ? error.message : String(error), {
        title: t('notifications.saveFailed'),
      });
    } finally {
      setSaving(false);
    }
  };

  const addPresetClient = async (preset: AcpClientPreset) => {
    const nextClient = defaultConfigForPreset(preset);
    const next = {
      acpClients: {
        ...config.acpClients,
        [preset.id]: nextClient,
      },
    };
    setConfig(next);
    setJsonConfig(formatConfig(next));
    setEnvDrafts(prev => ({
      ...prev,
      [preset.id]: formatEnv(nextClient.env),
    }));
    setDirty(true);
    await saveConfig(next, { mergeEnvDrafts: false });
  };

  const saveJsonConfig = async () => {
    try {
      const parsed = normalizeConfigValue(JSON.parse(jsonConfig));
      await saveConfig(parsed, { mergeEnvDrafts: false });
      setConfig(parsed);
      setEnvDrafts(
        Object.fromEntries(
          Object.entries(parsed.acpClients).map(([clientId, clientConfig]) => [
            clientId,
            formatEnv(clientConfig.env),
          ])
        )
      );
      setShowJsonEditor(false);
    } catch (error) {
      notifyError(error instanceof Error ? error.message : String(error), {
        title: t('notifications.invalidJson'),
      });
    }
  };

  const permissionOptions = useMemo(() => [
    { value: 'ask', label: t('permissionMode.ask') },
    { value: 'allow_once', label: t('permissionMode.allowOnce') },
    { value: 'reject_once', label: t('permissionMode.rejectOnce') },
  ], [t]);

  const registryFilterOptions = useMemo(() => [
    { value: 'all', label: t('registry.filters.all') },
    { value: 'installed', label: t('registry.filters.enabled') },
    { value: 'not_installed', label: t('registry.filters.notInstalled') },
    { value: 'invalid', label: t('registry.filters.configInvalid') },
  ], [t]);

  const getStatusLabel = useCallback((status: AgentRowStatus) => {
    if (status === 'enabled') return t('registry.enabled');
    if (status === 'ready') return t('registry.ready');
    if (status === 'not_installed') return t('registry.notInstalled');
    if (status === 'checking') return t('registry.checking');
    return t('registry.configInvalid');
  }, [t]);

  const openLearnMore = useCallback(() => {
    void systemAPI.openExternal('https://agentclientprotocol.com/get-started/introduction').catch((error) => {
      log.error('Failed to open ACP documentation', error);
      notifyError(error instanceof Error ? error.message : String(error), {
        title: t('notifications.openLinkFailed'),
      });
    });
  }, [notifyError, t]);

  return (
    <ConfigPageLayout className="bitfun-acp-agents">
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
      />

      <ConfigPageContent>
        <div className="bitfun-acp-agents__manager">
          <div className="bitfun-acp-agents__toolbar">
            <Input
              className="bitfun-acp-agents__search"
              value={registrySearch}
              onChange={(event) => setRegistrySearch(event.target.value)}
              placeholder={t('registry.searchPlaceholder')}
              prefix={<Search size={15} />}
              size="medium"
              variant="outlined"
            />
            <div className="bitfun-acp-agents__toolbar-actions">
              <Select
                className="bitfun-acp-agents__filter-select"
                options={registryFilterOptions}
                value={registryFilter}
                onChange={(value) => setRegistryFilter(value as RegistryFilter)}
                size="small"
              />
              <Button
                variant="secondary"
                size="small"
                onClick={() => setShowJsonEditor(prev => !prev)}
              >
                <FileJson size={14} />
                {showJsonEditor ? t('actions.closeJson') : t('actions.editJson')}
              </Button>
              <Button
                variant="secondary"
                size="small"
                onClick={() => { void refreshRequirementProbes({ force: true }); }}
                isLoading={probingRequirements}
              >
                <RefreshCw size={14} />
                {t('actions.refresh')}
              </Button>
              <Button
                variant="secondary"
                size="small"
                onClick={openLearnMore}
              >
                {t('actions.learnMore')}
                <ExternalLink size={14} />
              </Button>
              {dirty && (
                <Button
                  variant="primary"
                  size="small"
                  onClick={() => { void saveConfig(); }}
                  isLoading={saving}
                >
                  <Save size={14} />
                  {t('actions.save')}
                </Button>
              )}
            </div>
          </div>

          {showJsonEditor && (
            <ConfigPageSection
              title={t('json.title')}
              description={t('json.description')}
            >
              <Textarea
                ref={jsonEditorRef}
                className="bitfun-acp-agents__json-textarea"
                value={jsonConfig}
                onChange={(event) => {
                  setJsonConfig(event.target.value);
                  setDirty(true);
                }}
                onKeyDown={(event) => {
                  if (event.key !== 'Tab') return;
                  event.preventDefault();
                  const target = event.currentTarget;
                  const start = target.selectionStart ?? 0;
                  const end = target.selectionEnd ?? 0;
                  const nextValue = jsonConfig.slice(0, start) + '  ' + jsonConfig.slice(end);
                  setJsonConfig(nextValue);
                  setDirty(true);
                  requestAnimationFrame(() => {
                    jsonEditorRef.current?.focus();
                    jsonEditorRef.current?.setSelectionRange(start + 2, start + 2);
                  });
                }}
                rows={16}
                spellCheck={false}
              />
              <div className="bitfun-acp-agents__json-actions">
                <Button variant="secondary" size="small" onClick={() => setJsonConfig(formatConfig(config))}>
                  {t('actions.revert')}
                </Button>
                <Button variant="primary" size="small" onClick={() => { void saveJsonConfig(); }} isLoading={saving}>
                  {t('actions.saveJson')}
                </Button>
              </div>
            </ConfigPageSection>
          )}

          <ConfigPageSection title={t('registry.title')} description={t('registry.description')}>
          {loading ? (
            <div className="bitfun-acp-agents__empty">{t('clients.loading')}</div>
          ) : registryPresets.length === 0 && visibleCustomClientRows.length === 0 ? (
            <div className="bitfun-acp-agents__empty">{t('registry.empty')}</div>
          ) : (
            <div className="bitfun-acp-agents__registry-list">
              {registryPresets.map(preset => {
                const clientConfig = config.acpClients[preset.id] ?? defaultConfigForPreset(preset);
                const requirementProbe = probesById.get(preset.id);
                const probePending = probingRequirements && !requirementProbe;
                const hasConfigEntry = Boolean(config.acpClients[preset.id]);
                const configured = hasConfigEntry;
                const enabled = clientConfig.enabled;
                const runnable = requirementProbe?.runnable === true;
                const status = getAgentRowStatus({ configured, enabled, runnable, probePending });
                const installing = installingClientIds.has(preset.id);

                return (
                  <div key={preset.id} className="bitfun-acp-agents__registry-row">
                    <div className="bitfun-acp-agents__registry-main">
                      <span className="bitfun-acp-agents__registry-icon">
                        <Bot size={16} />
                      </span>
                      <div className="bitfun-acp-agents__registry-copy">
                        <span className="bitfun-acp-agents__registry-name">{preset.name}</span>
                        <p className="bitfun-acp-agents__registry-description">{preset.description}</p>
                      </div>
                    </div>
                    <div className="bitfun-acp-agents__capabilities">
                      <CapabilityBadge
                        icon={<Terminal size={12} />}
                        item={requirementProbe?.tool}
                        label={t('requirements.tool')}
                        installedText={t('requirements.installed')}
                        missingText={t('requirements.missing')}
                        checking={probePending}
                        checkingText={t('requirements.checking')}
                      />
                    </div>
                    <div className="bitfun-acp-agents__status-cell">
                      <AgentStatusBadge status={status} label={getStatusLabel(status)} />
                    </div>
                    <div className="bitfun-acp-agents__confirmation-cell">
                      {hasConfigEntry ? (
                        <Select
                          className="bitfun-acp-agents__confirmation-select"
                          options={permissionOptions}
                          value={clientConfig.permissionMode}
                          onChange={(value) => patchClientConfig(preset.id, {
                            permissionMode: normalizePermissionMode(value),
                          })}
                          size="small"
                        />
                      ) : status === 'not_installed' ? (
                        <Button
                          className="bitfun-acp-agents__add-button"
                          variant="secondary"
                          size="small"
                          onClick={() => { void installPresetClient(preset); }}
                          isLoading={installing}
                        >
                          <Download size={14} />
                          {t('actions.download')}
                        </Button>
                      ) : (
                        <Button
                          className="bitfun-acp-agents__add-button"
                          variant="secondary"
                          size="small"
                          onClick={() => addPresetClient(preset)}
                        >
                          <Plus size={14} />
                          {t('actions.add')}
                        </Button>
                      )}
                    </div>
                  </div>
                );
              })}
              {visibleCustomClientRows.map(clientId => {
                const clientInfo = clientsById.get(clientId);
                const clientConfig = config.acpClients[clientId];
                if (!clientConfig) return null;

                const requirementProbe = probesById.get(clientId);
                const probePending = probingRequirements && !requirementProbe;
                const runnable = requirementProbe?.runnable === true;
                const status = getAgentRowStatus({
                  configured: true,
                  enabled: clientConfig.enabled !== false,
                  runnable,
                  probePending,
                });
                const displayName = clientConfig.name || clientInfo?.name || clientId;

                return (
                  <div
                    key={clientId}
                    className="bitfun-acp-agents__registry-row"
                  >
                    <div className="bitfun-acp-agents__registry-main">
                      <span className="bitfun-acp-agents__registry-icon">
                        <Bot size={16} />
                      </span>
                      <div className="bitfun-acp-agents__registry-copy">
                        <span className="bitfun-acp-agents__registry-name">{displayName}</span>
                        <p className="bitfun-acp-agents__registry-description bitfun-acp-agents__registry-command">
                          {[clientConfig.command, ...clientConfig.args].join(' ')}
                        </p>
                      </div>
                    </div>
                    <div className="bitfun-acp-agents__capabilities">
                      <CapabilityBadge
                        icon={<Terminal size={12} />}
                        item={requirementProbe?.tool}
                        label={t('requirements.tool')}
                        installedText={t('requirements.installed')}
                        missingText={t('requirements.missing')}
                        checking={probePending}
                        checkingText={t('requirements.checking')}
                      />
                    </div>
                    <div className="bitfun-acp-agents__status-cell">
                      <AgentStatusBadge status={status} label={getStatusLabel(status)} />
                    </div>
                    <div className="bitfun-acp-agents__confirmation-cell">
                      <Select
                        className="bitfun-acp-agents__confirmation-select"
                        options={permissionOptions}
                        value={clientConfig.permissionMode}
                        onChange={(value) => patchClientConfig(clientId, {
                          permissionMode: normalizePermissionMode(value),
                        })}
                        size="small"
                      />
                    </div>
                  </div>
                );
              })}
            </div>
          )}
          </ConfigPageSection>
        </div>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default AcpAgentsConfig;
