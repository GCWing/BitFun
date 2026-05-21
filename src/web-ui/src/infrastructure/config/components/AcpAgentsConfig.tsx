import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Bot,
  CircleAlert,
  Download,
  ExternalLink,
  FileJson,
  LoaderCircle,
  Plus,
  RefreshCw,
  Save,
  Search,
  Server,
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
import { sshApi } from '@/features/ssh-remote/sshApi';
import type { SavedConnection } from '@/features/ssh-remote/types';
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
    description: 'Native ACP coding agent.',
    command: 'opencode',
    args: ['acp'],
  },
  {
    id: 'claude-code',
    name: 'Claude Code',
    description: 'Claude Code via the Zed ACP adapter.',
    command: 'npx',
    args: ['--yes', '@zed-industries/claude-code-acp@latest'],
  },
  {
    id: 'codex',
    name: 'Codex',
    description: 'OpenAI Codex via the Zed ACP adapter.',
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
type AgentRowStatus = 'enabled' | 'ready' | 'partial' | 'not_installed' | 'invalid' | 'checking';

type RequirementIssueKind =
  | 'none'
  | 'cli_missing'
  | 'adapter_missing'
  | 'connection_failed'
  | 'permission_denied'
  | 'path_invalid'
  | 'version_mismatch'
  | 'config_invalid';

function classifyRequirementError(error?: string): Exclude<RequirementIssueKind, 'none' | 'adapter_missing'> {
  const lower = error?.toLowerCase() ?? '';
  if (!lower) {
    return 'config_invalid';
  }
  if (
    lower.includes('permission denied') ||
    lower.includes('operation not permitted') ||
    lower.includes('access denied')
  ) {
    return 'permission_denied';
  }
  if (
    lower.includes('ssh') ||
    lower.includes('connection refused') ||
    lower.includes('timed out') ||
    lower.includes('timeout') ||
    lower.includes('network') ||
    lower.includes('host key')
  ) {
    return 'connection_failed';
  }
  if (
    lower.includes('version') ||
    lower.includes('mismatch') ||
    lower.includes('incompatible')
  ) {
    return 'version_mismatch';
  }
  if (
    lower.includes('not found') ||
    lower.includes('no such file or directory') ||
    lower.includes('command -v') ||
    lower.includes('path')
  ) {
    return 'path_invalid';
  }
  return 'config_invalid';
}

function getAgentRowStatus({
  configured,
  enabled,
  toolInstalled,
  adapterInstalled,
  requiresAdapter,
  probePending,
}: {
  configured: boolean;
  enabled: boolean;
  toolInstalled?: boolean;
  adapterInstalled?: boolean;
  requiresAdapter: boolean;
  probePending: boolean;
}): AgentRowStatus {
  if (probePending) return 'checking';
  if (toolInstalled === false) return 'not_installed';
  if (requiresAdapter && adapterInstalled === false) return 'partial';
  if (!configured) return 'ready';
  if (!enabled) return 'invalid';
  return 'enabled';
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
  title,
}: {
  status: AgentRowStatus;
  label: string;
  title?: string;
}) {
  return (
    <span className={`bitfun-acp-agents__status is-${status}`} title={title}>
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
  const [savedConnections, setSavedConnections] = useState<SavedConnection[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [showJsonEditor, setShowJsonEditor] = useState(false);
  const [jsonConfig, setJsonConfig] = useState('');
  const [envDrafts, setEnvDrafts] = useState<Record<string, string>>({});
  const [requirementProbes, setRequirementProbes] = useState<AcpClientRequirementProbe[]>(
    requirementProbeCache ?? []
  );
  const [remoteRequirementProbes, setRemoteRequirementProbes] = useState<Record<string, AcpClientRequirementProbe[]>>({});
  const [probingRemoteRequirements, setProbingRemoteRequirements] = useState<Set<string>>(() => new Set());
  const [probingRequirements, setProbingRequirements] = useState(false);
  const [registrySearch, setRegistrySearch] = useState('');
  const [registryFilter, setRegistryFilter] = useState<RegistryFilter>('all');
  const [installingClientIds, setInstallingClientIds] = useState<Set<string>>(() => new Set());
  const [installingRemoteClientIds, setInstallingRemoteClientIds] = useState<Set<string>>(() => new Set());
  const requirementProbeRequestIdRef = useRef(0);
  const loadedRemoteProbeIdsRef = useRef<Set<string>>(new Set());
  const [remoteProbeRefreshNonce, setRemoteProbeRefreshNonce] = useState(0);

  const clientsById = useMemo(() => new Map(clients.map(client => [client.id, client])), [clients]);
  const remoteConnectionRows = useMemo(() => {
    return [...savedConnections].sort((left, right) => {
      const leftTime = left.lastConnected ?? 0;
      const rightTime = right.lastConnected ?? 0;
      if (leftTime !== rightTime) return rightTime - leftTime;
      return (left.name || left.id).localeCompare(right.name || right.id);
    });
  }, [savedConnections]);
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
        toolInstalled: probe?.tool.installed,
        adapterInstalled: probe?.adapter?.installed,
        requiresAdapter: Boolean(probe?.adapter || preset.id !== 'opencode'),
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
        toolInstalled: requirementProbe?.tool.installed,
        adapterInstalled: requirementProbe?.adapter?.installed,
        requiresAdapter: Boolean(requirementProbe?.adapter),
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

  const refreshRemoteRequirementProbes = useCallback(async (
    connectionId: string,
    options: { force?: boolean; notifyOnError?: boolean } = {}
  ) => {
    const normalizedConnectionId = connectionId.trim();
    if (!normalizedConnectionId) return;
    if (!options.force && loadedRemoteProbeIdsRef.current.has(normalizedConnectionId)) return;

    setProbingRemoteRequirements(prev => {
      const next = new Set(prev);
      next.add(normalizedConnectionId);
      return next;
    });
    try {
      const nextRequirementProbes = await ACPClientAPI.probeClientRequirements({
        remoteConnectionId: normalizedConnectionId,
        force: options.force,
      });
      loadedRemoteProbeIdsRef.current.add(normalizedConnectionId);
      setRemoteRequirementProbes(prev => ({
        ...prev,
        [normalizedConnectionId]: nextRequirementProbes,
      }));
    } catch (error) {
      log.error('Failed to probe remote ACP agent requirements', error);
      if (options.notifyOnError ?? true) {
        notifyError(error instanceof Error ? error.message : String(error), {
          title: t('notifications.probeFailed'),
        });
      }
    } finally {
      setProbingRemoteRequirements(prev => {
        const next = new Set(prev);
        next.delete(normalizedConnectionId);
        return next;
      });
    }
  }, [notifyError, t]);

  const loadConfig = useCallback(async (
    options: { showLoading?: boolean; refreshRequirements?: boolean } = {}
  ) => {
    const showLoading = options.showLoading ?? true;
    const refreshRequirements = options.refreshRequirements ?? true;
    try {
      if (showLoading) {
        setLoading(true);
      }
      const [rawConfig, nextClients] = await Promise.all([
        ACPClientAPI.loadJsonConfig(),
        ACPClientAPI.getClients(),
      ]);
      const nextSavedConnections = await sshApi.listSavedConnections().catch((error) => {
        log.warn('Failed to load saved SSH connections for ACP remote overrides', error);
        return [] as SavedConnection[];
      });
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
      setSavedConnections(nextSavedConnections);
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

  useEffect(() => {
    if (loading) return;
    for (const connection of remoteConnectionRows) {
      void refreshRemoteRequirementProbes(connection.id, { notifyOnError: false });
    }
  }, [loading, refreshRemoteRequirementProbes, remoteConnectionRows, remoteProbeRefreshNonce]);

  const patchClientConfig = (clientId: string, patch: Partial<AcpClientConfig>) => {
    setConfig(prev => {
      const preset = PRESET_BY_ID.get(clientId);
      const current = prev.acpClients[clientId] ??
        (preset ? defaultConfigForPreset(preset) : undefined);
      if (!current) return prev;

      const next = {
        ...prev,
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

  const installPresetClient = async (
    preset: AcpClientPreset,
    options: { remoteConnectionId?: string } = {}
  ) => {
    const remoteConnectionId = options.remoteConnectionId?.trim();
    const installKey = remoteConnectionId ? `${remoteConnectionId}:${preset.id}` : preset.id;
    const setInstalling = remoteConnectionId ? setInstallingRemoteClientIds : setInstallingClientIds;
    setInstalling(prev => new Set(prev).add(installKey));
    try {
      await ACPClientAPI.installClientCli({
        clientId: preset.id,
        remoteConnectionId,
      });
      if (remoteConnectionId) {
        loadedRemoteProbeIdsRef.current.delete(remoteConnectionId);
        await refreshRemoteRequirementProbes(remoteConnectionId, { force: true, notifyOnError: false });
      } else {
        requirementProbeCache = null;
        await refreshRequirementProbes({ force: true, notifyOnError: false });
      }
      notifySuccess(t('notifications.installSuccess'));
    } catch (error) {
      log.error('Failed to install ACP agent CLI', error);
      notifyError(error instanceof Error ? error.message : String(error), {
        title: t('notifications.installFailed'),
      });
    } finally {
      setInstalling(prev => {
        const next = new Set(prev);
        next.delete(installKey);
        return next;
      });
    }
  };

  const configurePresetClient = async (preset: AcpClientPreset) => {
    const installKey = preset.id;
    setInstallingClientIds(prev => new Set(prev).add(installKey));
    try {
      await ACPClientAPI.predownloadClientAdapter({
        clientId: preset.id,
      });
      requirementProbeCache = null;
      await refreshRequirementProbes({ force: true, notifyOnError: false });
      notifySuccess(t('notifications.configureSuccess'));
    } catch (error) {
      log.error('Failed to predownload ACP adapter', error);
      notifyError(error instanceof Error ? error.message : String(error), {
        title: t('notifications.configureFailed'),
      });
    } finally {
      setInstallingClientIds(prev => {
        const next = new Set(prev);
        next.delete(installKey);
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
      loadedRemoteProbeIdsRef.current.clear();
      setRemoteRequirementProbes({});
      setRemoteProbeRefreshNonce(prev => prev + 1);
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
      ...config,
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

  const getIssueKind = useCallback((args: {
    probe?: AcpClientRequirementProbe;
    requiresAdapter: boolean;
  }): RequirementIssueKind => {
    const { probe, requiresAdapter } = args;
    if (!probe) return 'config_invalid';

    const toolIssue = classifyRequirementError(probe.tool.error);
    if (toolIssue !== 'config_invalid') {
      return toolIssue;
    }

    if (!probe.tool.installed) {
      return 'cli_missing';
    }

    if (requiresAdapter) {
      if (probe.adapter?.error) {
        const adapterIssue = classifyRequirementError(probe.adapter.error);
        if (adapterIssue !== 'config_invalid') {
          return adapterIssue;
        }
      }
      if (probe.adapter && !probe.adapter.installed) {
        return 'adapter_missing';
      }
    }

    return probe.runnable ? 'none' : 'config_invalid';
  }, []);

  const getStatusLabel = useCallback((args: {
    status: AgentRowStatus;
    issueKind: RequirementIssueKind;
    probe?: AcpClientRequirementProbe;
    requiresAdapter: boolean;
  }) => {
    const { status, issueKind, probe, requiresAdapter } = args;
    if (status === 'enabled') return t('registry.enabled');
    if (status === 'ready') return t('registry.ready');
    if (status === 'partial') return t('registry.partial');
    if (status === 'checking') return t('registry.checking');

    if (issueKind === 'connection_failed') return t('registry.connectionFailed');
    if (issueKind === 'permission_denied') return t('registry.permissionDenied');
    if (issueKind === 'path_invalid') return t('registry.pathInvalid');
    if (issueKind === 'version_mismatch') return t('registry.versionMismatch');
    if (issueKind === 'adapter_missing' || (requiresAdapter && probe?.adapter && !probe.adapter.installed)) {
      return t('registry.acpMissing');
    }
    if (issueKind === 'cli_missing' || probe?.tool.installed === false) {
      return t('registry.cliMissing');
    }
    return t('registry.configInvalid');
  }, [t]);

  const getStatusTitle = useCallback((args: {
    status: AgentRowStatus;
    issueKind: RequirementIssueKind;
    probe?: AcpClientRequirementProbe;
    requiresAdapter: boolean;
  }) => {
    const { status, issueKind, probe, requiresAdapter } = args;
    const lines: string[] = [];
    if (status === 'enabled') {
      lines.push(t('registry.enabled'));
    } else if (status === 'ready') {
      lines.push(t('registry.ready'));
    } else if (status === 'partial') {
      lines.push(t('registry.partialDetail'));
    } else if (status === 'checking') {
      lines.push(t('registry.checking'));
    }

    if (issueKind === 'connection_failed') {
      lines.push(t('registry.connectionFailedDetail'));
    } else if (issueKind === 'permission_denied') {
      lines.push(t('registry.permissionDeniedDetail'));
    } else if (issueKind === 'path_invalid') {
      lines.push(t('registry.pathInvalidDetail'));
    } else if (issueKind === 'version_mismatch') {
      lines.push(t('registry.versionMismatchDetail'));
    } else if (issueKind === 'adapter_missing' || (requiresAdapter && probe?.adapter && !probe.adapter.installed)) {
      lines.push(t('registry.acpMissingDetail'));
    } else if (issueKind === 'cli_missing' || probe?.tool.installed === false) {
      lines.push(t('registry.cliMissingDetail'));
    } else if (status === 'invalid') {
      lines.push(t('registry.configInvalidDetail'));
    }

    if (probe?.tool.path) {
      lines.push(`${t('registry.toolPath')}: ${probe.tool.path}`);
    }
    if (probe?.tool.version) {
      lines.push(`${t('registry.toolVersion')}: ${probe.tool.version}`);
    }
    if (probe?.tool.error) {
      lines.push(probe.tool.error);
    }
    if (probe?.adapter?.error) {
      lines.push(probe.adapter.error);
    }
    if (probe?.notes.length) {
      lines.push(...probe.notes);
    }
    return lines.filter(Boolean).join('\n');
  }, [t]);

  const getRemoteSummary = useCallback((available: number, total: number) => {
    return t('remote.summary', { available, total });
  }, [t]);

  const openLearnMore = useCallback(() => {
    void systemAPI.openExternal('https://agentclientprotocol.com/get-started/introduction').catch((error) => {
      log.error('Failed to open ACP documentation', error);
      notifyError(error instanceof Error ? error.message : String(error), {
        title: t('notifications.openLinkFailed'),
      });
    });
  }, [notifyError, t]);

  const remoteAgentIds = useMemo(() => {
    const ids = new Set<string>([
      ...PRESETS.map(preset => preset.id),
      ...Object.keys(config.acpClients),
    ]);
    return Array.from(ids).sort((left, right) => {
      const leftPresetIndex = PRESETS.findIndex(preset => preset.id === left);
      const rightPresetIndex = PRESETS.findIndex(preset => preset.id === right);
      if (leftPresetIndex !== -1 || rightPresetIndex !== -1) {
        if (leftPresetIndex === -1) return 1;
        if (rightPresetIndex === -1) return -1;
        return leftPresetIndex - rightPresetIndex;
      }
      return left.localeCompare(right);
    });
  }, [config.acpClients]);

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
                const requiresAdapter = preset.id !== 'opencode' || Boolean(requirementProbe?.adapter);
                const issueKind = getIssueKind({ probe: requirementProbe, requiresAdapter });
                const status = getAgentRowStatus({
                  configured,
                  enabled,
                  toolInstalled: requirementProbe?.tool.installed,
                  adapterInstalled: requirementProbe?.adapter?.installed,
                  requiresAdapter,
                  probePending,
                });
                const statusLabel = getStatusLabel({
                  status,
                  issueKind,
                  probe: requirementProbe,
                  requiresAdapter,
                });
                const statusTitle = getStatusTitle({
                  status,
                  issueKind,
                  probe: requirementProbe,
                  requiresAdapter,
                });
                const installing = installingClientIds.has(preset.id);
                const configuring = installingClientIds.has(preset.id);
                const showSelect = hasConfigEntry && (status === 'enabled' || status === 'ready');
                const canInstallCli = status === 'not_installed' && issueKind !== 'connection_failed';
                const canConfigureAcp = !requiresAdapter
                  ? false
                  : issueKind === 'adapter_missing' || (status === 'partial' && issueKind === 'config_invalid');
                const canViewError = status === 'invalid'
                  || issueKind === 'connection_failed'
                  || issueKind === 'permission_denied'
                  || issueKind === 'path_invalid'
                  || issueKind === 'version_mismatch';

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
                      <AgentStatusBadge status={status} label={statusLabel} title={statusTitle} />
                    </div>
                    <div className="bitfun-acp-agents__confirmation-cell">
                      {showSelect ? (
                        <Select
                          className="bitfun-acp-agents__confirmation-select"
                          options={permissionOptions}
                          value={clientConfig.permissionMode}
                          onChange={(value) => patchClientConfig(preset.id, {
                            permissionMode: normalizePermissionMode(value),
                          })}
                          size="small"
                        />
                      ) : canInstallCli ? (
                        <Button
                          className="bitfun-acp-agents__add-button"
                          variant="secondary"
                          size="small"
                          onClick={() => { void installPresetClient(preset); }}
                          isLoading={installing}
                        >
                          <Download size={14} />
                          {t('actions.installCli')}
                        </Button>
                      ) : canConfigureAcp ? (
                        <Button
                          className="bitfun-acp-agents__add-button"
                          variant="secondary"
                          size="small"
                          onClick={() => { void configurePresetClient(preset); }}
                          isLoading={configuring}
                        >
                          <FileJson size={14} />
                          {t('actions.configureAcp')}
                        </Button>
                      ) : canViewError ? (
                        <Button
                          className="bitfun-acp-agents__add-button"
                          variant="secondary"
                          size="small"
                          onClick={() => {
                            notifyError(
                              statusTitle || t('registry.configInvalidDetail'),
                              { title: statusLabel }
                            );
                          }}
                        >
                          <CircleAlert size={14} />
                          {t('actions.viewError')}
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
                const requiresAdapter = Boolean(requirementProbe?.adapter);
                const issueKind = getIssueKind({ probe: requirementProbe, requiresAdapter });
                const status = getAgentRowStatus({
                  configured: true,
                  enabled: clientConfig.enabled !== false,
                  toolInstalled: requirementProbe?.tool.installed,
                  adapterInstalled: requirementProbe?.adapter?.installed,
                  requiresAdapter,
                  probePending,
                });
                const statusLabel = getStatusLabel({
                  status,
                  issueKind,
                  probe: requirementProbe,
                  requiresAdapter,
                });
                const statusTitle = getStatusTitle({
                  status,
                  issueKind,
                  probe: requirementProbe,
                  requiresAdapter,
                });
                const displayName = clientConfig.name || clientInfo?.name || clientId;
                const canViewError = status === 'invalid'
                  || issueKind === 'connection_failed'
                  || issueKind === 'permission_denied'
                  || issueKind === 'path_invalid'
                  || issueKind === 'version_mismatch';

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
                      <AgentStatusBadge status={status} label={statusLabel} title={statusTitle} />
                    </div>
                    <div className="bitfun-acp-agents__confirmation-cell">
                      {status === 'enabled' || status === 'ready' ? (
                        <Select
                          className="bitfun-acp-agents__confirmation-select"
                          options={permissionOptions}
                          value={clientConfig.permissionMode}
                          onChange={(value) => patchClientConfig(clientId, {
                            permissionMode: normalizePermissionMode(value),
                          })}
                          size="small"
                        />
                      ) : canViewError ? (
                        <Button
                          className="bitfun-acp-agents__add-button"
                          variant="secondary"
                          size="small"
                          onClick={() => {
                            notifyError(
                              statusTitle || t('registry.configInvalidDetail'),
                              { title: statusLabel }
                            );
                          }}
                        >
                          <CircleAlert size={14} />
                          {t('actions.viewError')}
                        </Button>
                      ) : null}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
          </ConfigPageSection>

          <ConfigPageSection title={t('remote.title')} description={t('remote.description')}>
            {remoteConnectionRows.length === 0 ? (
              <div className="bitfun-acp-agents__empty">{t('remote.empty')}</div>
            ) : (
              <div className="bitfun-acp-agents__remote-list">
                {remoteConnectionRows.map(connection => {
                  const hostLabel = [connection.username, connection.host]
                    .filter(Boolean)
                    .join('@');
                  const remoteProbes = remoteRequirementProbes[connection.id] ?? [];
                  const remoteProbesById = new Map(remoteProbes.map(probe => [probe.id, probe]));
                  const remoteProbeLoaded = Object.prototype.hasOwnProperty.call(
                    remoteRequirementProbes,
                    connection.id
                  );
                  const probingRemote = probingRemoteRequirements.has(connection.id);
                  const remoteRows = remoteAgentIds.map(clientId => {
                    const preset = PRESET_BY_ID.get(clientId);
                    const clientConfig = config.acpClients[clientId];
                    const requirementProbe = remoteProbesById.get(clientId);
                    const probePending = probingRemote || !remoteProbeLoaded || !requirementProbe;
                    const hasConfigEntry = Boolean(clientConfig);
                    const effectiveConfig = clientConfig ?? (preset ? defaultConfigForPreset(preset) : undefined);
                    const enabled = effectiveConfig?.enabled ?? true;
                    const requiresAdapter = Boolean(requirementProbe?.adapter || preset?.id !== 'opencode');
                    const issueKind = getIssueKind({ probe: requirementProbe, requiresAdapter });
                    const status = getAgentRowStatus({
                      configured: hasConfigEntry,
                      enabled,
                      toolInstalled: requirementProbe?.tool.installed,
                      adapterInstalled: requirementProbe?.adapter?.installed,
                      requiresAdapter,
                      probePending,
                    });
                    const displayName = effectiveConfig?.name || preset?.name || clientId;
                    const description = preset?.description ??
                      (effectiveConfig ? [effectiveConfig.command, ...effectiveConfig.args].join(' ') : clientId);
                    const installingRemote = installingRemoteClientIds.has(`${connection.id}:${clientId}`);

                    return {
                      clientId,
                      preset,
                      clientConfig,
                      requirementProbe,
                      probePending,
                      hasConfigEntry,
                      enabled,
                      requiresAdapter,
                      issueKind,
                      status,
                      displayName,
                      description,
                      installingRemote,
                    };
                  });
                  const availableCount = remoteRows.filter(row => row.status === 'enabled' || row.status === 'ready').length;
                  const issueCount = remoteRows.filter(row => (
                    row.status === 'partial' ||
                    row.status === 'not_installed' ||
                    row.status === 'invalid'
                  )).length;

                  return (
                    <div key={connection.id} className="bitfun-acp-agents__remote-server">
                      <div className="bitfun-acp-agents__remote-head">
                        <div className="bitfun-acp-agents__registry-main">
                          <span className="bitfun-acp-agents__registry-icon">
                            <Server size={16} />
                          </span>
                          <div className="bitfun-acp-agents__registry-copy">
                            <span className="bitfun-acp-agents__registry-name">
                              {connection.name || connection.id}
                            </span>
                            <p className="bitfun-acp-agents__registry-description">
                              {hostLabel || connection.id}
                            </p>
                            <div className="bitfun-acp-agents__remote-summary">
                              <span className="bitfun-acp-agents__summary-pill is-success">
                                {getRemoteSummary(availableCount, remoteRows.length)}
                              </span>
                              {issueCount > 0 && (
                                <span className="bitfun-acp-agents__summary-pill is-warning">
                                  {t('remote.issueSummary', { count: issueCount })}
                                </span>
                              )}
                            </div>
                          </div>
                        </div>
                        <div className="bitfun-acp-agents__remote-actions">
                          <Button
                            variant="secondary"
                            size="small"
                            onClick={() => {
                              loadedRemoteProbeIdsRef.current.delete(connection.id);
                              void refreshRemoteRequirementProbes(connection.id, {
                                force: true,
                              });
                            }}
                            isLoading={probingRemote}
                          >
                            <RefreshCw size={14} />
                            {t('remote.refreshDetection')}
                          </Button>
                        </div>
                      </div>
                      <div className="bitfun-acp-agents__remote-agent-list">
                        {remoteRows.map(row => {
                          const statusLabel = getStatusLabel({
                            status: row.status,
                            issueKind: row.issueKind,
                            probe: row.requirementProbe,
                            requiresAdapter: row.requiresAdapter,
                          });
                          const statusTitle = getStatusTitle({
                            status: row.status,
                            issueKind: row.issueKind,
                            probe: row.requirementProbe,
                            requiresAdapter: row.requiresAdapter,
                          });
                          const canInstallCli = row.preset && row.status === 'not_installed' && row.issueKind === 'cli_missing';
                          const canViewError = row.status === 'invalid' || row.status === 'partial'
                            || row.issueKind === 'connection_failed'
                            || row.issueKind === 'permission_denied'
                            || row.issueKind === 'path_invalid'
                            || row.issueKind === 'version_mismatch'
                            || row.issueKind === 'adapter_missing';

                          return (
                            <div key={row.clientId} className="bitfun-acp-agents__registry-row bitfun-acp-agents__registry-row--remote">
                              <div className="bitfun-acp-agents__registry-main">
                                <span className="bitfun-acp-agents__registry-icon">
                                  <Bot size={16} />
                                </span>
                                <div className="bitfun-acp-agents__registry-copy">
                                  <span className="bitfun-acp-agents__registry-name">{row.displayName}</span>
                                  <p className="bitfun-acp-agents__registry-description">{row.description}</p>
                                </div>
                              </div>
                              <div className="bitfun-acp-agents__capabilities">
                                <CapabilityBadge
                                  icon={<Terminal size={12} />}
                                  item={row.requirementProbe?.tool}
                                  label={t('requirements.tool')}
                                  installedText={t('requirements.installed')}
                                  missingText={t('requirements.missing')}
                                  checking={row.probePending}
                                  checkingText={t('requirements.checking')}
                                />
                                {row.requirementProbe?.adapter && (
                                  <CapabilityBadge
                                    icon={<FileJson size={12} />}
                                    item={row.requirementProbe.adapter}
                                    label={t('requirements.adapter')}
                                    installedText={t('requirements.installed')}
                                    missingText={t('requirements.missing')}
                                    checking={row.probePending}
                                    checkingText={t('requirements.checking')}
                                  />
                                )}
                              </div>
                              <div className="bitfun-acp-agents__status-cell">
                                <AgentStatusBadge status={row.status} label={statusLabel} title={statusTitle} />
                              </div>
                              <div className="bitfun-acp-agents__confirmation-cell">
                                {canInstallCli ? (
                                  <Button
                                    className="bitfun-acp-agents__add-button"
                                    variant="secondary"
                                    size="small"
                                    onClick={() => {
                                      void installPresetClient(row.preset!, {
                                        remoteConnectionId: connection.id,
                                      });
                                    }}
                                    isLoading={row.installingRemote}
                                  >
                                    <Download size={14} />
                                    {t('actions.installCli')}
                                  </Button>
                                ) : row.status === 'enabled' || row.status === 'ready' ? (
                                  row.clientConfig ? (
                                    <Select
                                      className="bitfun-acp-agents__confirmation-select"
                                      options={permissionOptions}
                                      value={row.clientConfig.permissionMode}
                                      onChange={(value) => patchClientConfig(row.clientId, {
                                        permissionMode: normalizePermissionMode(value),
                                      })}
                                      size="small"
                                    />
                                  ) : row.preset ? (
                                  <Button
                                    className="bitfun-acp-agents__add-button"
                                    variant="secondary"
                                    size="small"
                                    onClick={() => addPresetClient(row.preset!)}
                                  >
                                    <Plus size={14} />
                                    {t('actions.add')}
                                  </Button>
                                  ) : null
                                ) : canViewError ? (
                                  <Button
                                    className="bitfun-acp-agents__add-button"
                                    variant="secondary"
                                    size="small"
                                    onClick={() => {
                                      notifyError(
                                        statusTitle || t('registry.configInvalidDetail'),
                                        { title: statusLabel }
                                      );
                                    }}
                                  >
                                    <CircleAlert size={14} />
                                    {t('actions.viewError')}
                                  </Button>
                                ) : (
                                  null
                                )}
                              </div>
                            </div>
                          );
                        })}
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
