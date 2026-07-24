/**
 * Relay Deploy Wizard — one-click self-hosted relay server deployment.
 *
 * Steps: connect (pick an SSH server) → preflight (environment checks, optional
 * Docker install) → deploy (interactive remote PTY + background build) →
 * register (create the first account, provisioned locally so the plaintext
 * password never leaves this device) → done.
 *
 * Deploy/install run inside an embedded remote PTY so sudo passwords work.
 * Closing the wizard cancels any in-progress remote task.
 */

import React, { useState, useEffect, useCallback, useRef } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { Modal, Button, Input, Select, Alert, IconButton } from '@/component-library';
import {
  Server, User, Lock, Key, FolderOpen, Loader2, Play, ArrowDownToLine,
  CheckCircle2, XCircle, AlertTriangle, RefreshCw, Eye, EyeOff, Search,
  ChevronLeft, Rocket, PartyPopper,
} from 'lucide-react';
import { sshApi } from '../ssh-remote/sshApi';
import { pickSshPrivateKeyPath } from '../ssh-remote/pickSshPrivateKeyPath';
import { SSHAuthPromptDialog, type SSHAuthPromptSubmitPayload } from '../ssh-remote/SSHAuthPromptDialog';
import type {
  SavedConnection,
  SSHConfigEntry,
  SSHConnectionConfig,
  SSHAuthMethod,
} from '../ssh-remote/types';
import {
  relayDeployApi,
  type RelayPreflight,
  type RelayDeployTask,
  type RelayTaskStatus,
  type RelayVerifyResult,
  type DockerAccessMode,
} from './relayDeployApi';
import { ConnectedTerminal, getTerminalService } from '@/tools/terminal';
import { createLogger } from '@/shared/utils/logger';
import './RelayDeployWizard.scss';

const log = createLogger('RelayDeployWizard');

const DEFAULT_RELAY_PORT = 9700;
const POLL_INTERVAL_MS = 1500;
const MAX_POLL_FAILURES = 10;

function parseRelayPort(raw: string): number | null {
  const n = Number.parseInt(raw.trim(), 10);
  if (!Number.isFinite(n) || n < 1 || n > 65535) return null;
  return n;
}
/** Default terminal font is 14; embed two levels smaller to fit the dialog. */
const DEPLOY_TERMINAL_FONT_SIZE = 12;
const DEPLOY_TERMINAL_OPTIONS = { fontSize: DEPLOY_TERMINAL_FONT_SIZE };

type Step = 'connect' | 'preflight' | 'deploy' | 'register' | 'done';

export interface RelayDeployResult {
  relayUrl: string;
  username: string;
  password: string;
}

interface RelayDeployWizardProps {
  isOpen: boolean;
  onClose: () => void;
  /** Called with the registered credentials so the login dialog can sign in. */
  onRegistered: (result: RelayDeployResult) => void;
}

function errMsg(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

/** Keeps the same id scheme as the SSH connection dialog. */
function generateConnectionId(host: string, username: string): string {
  return `ssh-${username}@${host}`;
}

export const RelayDeployWizard: React.FC<RelayDeployWizardProps> = ({
  isOpen,
  onClose,
  onRegistered,
}) => {
  const { t } = useI18n('common');

  const [step, setStep] = useState<Step>('connect');
  const [error, setError] = useState<string | null>(null);

  // ── connect ─────────────────────────────────────────────────────────────
  const [savedConnections, setSavedConnections] = useState<SavedConnection[]>([]);
  const [sshConfigHosts, setSSHConfigHosts] = useState<SSHConfigEntry[]>([]);
  const [savedSearch, setSavedSearch] = useState('');
  const [configSearch, setConfigSearch] = useState('');
  const [formData, setFormData] = useState({
    name: '',
    host: '',
    port: '22',
    username: '',
    authType: 'password' as 'password' | 'privateKey',
    password: '',
    keyPath: '~/.ssh/id_rsa',
    passphrase: '',
  });
  const [connecting, setConnecting] = useState(false);
  const [credentialsPrompt, setCredentialsPrompt] = useState<SavedConnection | null>(null);
  const [showPassword, setShowPassword] = useState(false);
  const [showPassphrase, setShowPassphrase] = useState(false);
  const connectFormRef = useRef<HTMLDivElement>(null);
  const connectFormHighlightTimerRef = useRef<number | null>(null);
  const [connectFormHighlighted, setConnectFormHighlighted] = useState(false);

  const revealConnectForm = useCallback(() => {
    const el = connectFormRef.current;
    if (!el) return;
    el.scrollIntoView({ behavior: 'smooth', block: 'start' });
    setConnectFormHighlighted(true);
    if (connectFormHighlightTimerRef.current != null) {
      window.clearTimeout(connectFormHighlightTimerRef.current);
    }
    connectFormHighlightTimerRef.current = window.setTimeout(() => {
      setConnectFormHighlighted(false);
      connectFormHighlightTimerRef.current = null;
    }, 1200);
  }, []);

  useEffect(() => {
    return () => {
      if (connectFormHighlightTimerRef.current != null) {
        window.clearTimeout(connectFormHighlightTimerRef.current);
      }
    };
  }, []);

  // ── connected session ────────────────────────────────────────────────────
  const [connectionId, setConnectionId] = useState<string | null>(null);
  const [serverHost, setServerHost] = useState('');
  const [serverLabel, setServerLabel] = useState('');

  // ── preflight ────────────────────────────────────────────────────────────
  const [preflight, setPreflight] = useState<RelayPreflight | null>(null);
  const [preflightLoading, setPreflightLoading] = useState(false);
  const [relayPortInput, setRelayPortInput] = useState(String(DEFAULT_RELAY_PORT));

  // ── interactive PTY task (install docker / deploy) ───────────────────────
  const [activeTask, setActiveTask] = useState<RelayDeployTask | null>(null);
  const [taskStatus, setTaskStatus] = useState<RelayTaskStatus | null>(null);
  const [terminalSessionId, setTerminalSessionId] = useState<string | null>(null);
  const pollRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pollActiveRef = useRef(false);
  const cursorRef = useRef(0);
  const pollFailuresRef = useRef(0);
  const terminalSessionIdRef = useRef<string | null>(null);
  const connectionIdRef = useRef<string | null>(null);
  const activeTaskRef = useRef<RelayDeployTask | null>(null);
  const taskStatusRef = useRef<RelayTaskStatus | null>(null);

  // ── register ─────────────────────────────────────────────────────────────
  const [regUsername, setRegUsername] = useState('');
  const [regPassword, setRegPassword] = useState('');
  const [regConfirm, setRegConfirm] = useState('');
  const [regLoading, setRegLoading] = useState(false);
  const [showRegPassword, setShowRegPassword] = useState(false);
  /**
   * Redeploys keep the relay database, so account creation can fail with
   * "already exists". `existing` skips provisioning and hands the typed
   * credentials straight to the caller's real login.
   */
  const [regMode, setRegMode] = useState<'create' | 'existing'>('create');

  // ── done ─────────────────────────────────────────────────────────────────
  const [verify, setVerify] = useState<RelayVerifyResult | null>(null);

  const relayPort = parseRelayPort(relayPortInput) ?? DEFAULT_RELAY_PORT;
  const existingRelayPort =
    preflight && preflight.existingRelayPort > 0 ? preflight.existingRelayPort : null;
  const relayUrl = `http://${serverHost}:${relayPort}`;
  // Unrelated process on the selected port — block deploy. Busy-because-our
  // relay is handled by alreadyDeployed / portOwnedByRelay instead.
  const portConflict = !!preflight && preflight.portBusy && !preflight.portOwnedByRelay;
  // Container-aware: health on the typed port alone misses a running
  // bitfun-relay when the user changes RELAY_PORT. See feature README.
  const alreadyDeployed = !!preflight && (
    preflight.relayHealthy || preflight.containerRunning
  );

  const stopPolling = useCallback(() => {
    pollActiveRef.current = false;
    if (pollRef.current) {
      clearTimeout(pollRef.current);
      pollRef.current = null;
    }
  }, []);

  const closeDeployTerminal = useCallback(async () => {
    const sid = terminalSessionIdRef.current;
    terminalSessionIdRef.current = null;
    setTerminalSessionId(null);
    if (!sid) return;
    try {
      await getTerminalService().closeSession(sid, true);
    } catch (e) {
      log.warn('failed to close deploy terminal', e);
    }
  }, []);

  /** Snapshot refs first — close/reset must not clear them before cancel runs. */
  const cancelRemoteTaskIfRunning = useCallback(async (
    snapshot?: {
      connectionId: string | null;
      task: RelayDeployTask | null;
      status: RelayTaskStatus | null;
    },
  ) => {
    const connId = snapshot?.connectionId ?? connectionIdRef.current;
    const task = snapshot?.task ?? activeTaskRef.current;
    const status = snapshot?.status ?? taskStatusRef.current;
    if (!connId || !task || status !== 'running') return;
    try {
      await relayDeployApi.cancel(connId, task);
    } catch (e) {
      log.warn('failed to cancel remote deploy task', e);
    }
  }, []);

  useEffect(() => {
    connectionIdRef.current = connectionId;
  }, [connectionId]);

  useEffect(() => {
    activeTaskRef.current = activeTask;
  }, [activeTask]);

  useEffect(() => {
    taskStatusRef.current = taskStatus;
  }, [taskStatus]);

  // ── lifecycle ────────────────────────────────────────────────────────────
  // Closing the wizard MUST cancel the remote task (kill pid tree / best-effort
  // compose stop). Never leave a nohup Docker build running after dismiss.
  useEffect(() => {
    if (!isOpen) {
      stopPolling();
      // Capture before any state reset so cancel still has connection/task ids.
      const cancelSnapshot = {
        connectionId: connectionIdRef.current,
        task: activeTaskRef.current,
        status: taskStatusRef.current,
      };
      void (async () => {
        await cancelRemoteTaskIfRunning(cancelSnapshot);
        await closeDeployTerminal();
      })();
      setStep('connect');
      setError(null);
      setSavedSearch('');
      setConfigSearch('');
      setFormData({
        name: '', host: '', port: '22', username: '',
        authType: 'password', password: '', keyPath: '~/.ssh/id_rsa', passphrase: '',
      });
      setConnecting(false);
      setCredentialsPrompt(null);
      setConnectionId(null);
      setServerHost('');
      setServerLabel('');
      setPreflight(null);
      setPreflightLoading(false);
      setRelayPortInput(String(DEFAULT_RELAY_PORT));
      setActiveTask(null);
      setTaskStatus(null);
      setRegUsername('');
      setRegPassword('');
      setRegConfirm('');
      setRegLoading(false);
      setRegMode('create');
      setVerify(null);
      return;
    }
    sshApi.listSavedConnections().then(setSavedConnections).catch(() => setSavedConnections([]));
    sshApi.listSSHConfigHosts().then(setSSHConfigHosts).catch(() => setSSHConfigHosts([]));
  }, [isOpen, stopPolling, closeDeployTerminal, cancelRemoteTaskIfRunning]);

  // Stop polling / cancel remote task / close PTY on unmount.
  useEffect(() => {
    return () => {
      stopPolling();
      const cancelSnapshot = {
        connectionId: connectionIdRef.current,
        task: activeTaskRef.current,
        status: taskStatusRef.current,
      };
      void (async () => {
        await cancelRemoteTaskIfRunning(cancelSnapshot);
        await closeDeployTerminal();
      })();
    };
  }, [stopPolling, closeDeployTerminal, cancelRemoteTaskIfRunning]);

  // Auto-fill the form from ~/.ssh/config when the host changes (same behavior
  // as the SSH connection dialog).
  useEffect(() => {
    if (!formData.host.trim()) return;
    const timeout = setTimeout(async () => {
      try {
        const result = await sshApi.getSSHConfig(formData.host.trim());
        if (result.found && result.config) {
          const config = result.config;
          setFormData((prev) => ({
            ...prev,
            port: config.port ? String(config.port) : prev.port,
            username: config.user || prev.username,
            keyPath: config.identityFile || prev.keyPath,
            authType: config.identityFile ? 'privateKey' : prev.authType,
          }));
        }
      } catch {
        // ~/.ssh/config lookup is best-effort
      }
    }, 300);
    return () => clearTimeout(timeout);
  }, [formData.host]);

  // ── preflight ────────────────────────────────────────────────────────────
  const runPreflight = useCallback(async (connId: string, portOverride?: number) => {
    const port = portOverride ?? parseRelayPort(relayPortInput) ?? DEFAULT_RELAY_PORT;
    setPreflightLoading(true);
    setError(null);
    try {
      const pf = await relayDeployApi.preflight(connId, port);
      setPreflight(pf);
      setStep('preflight');
    } catch (e) {
      log.warn('preflight failed', e);
      setError(`${t('relayDeploy.checkFailed')}: ${errMsg(e)}`);
    } finally {
      setPreflightLoading(false);
    }
  }, [t, relayPortInput]);

  const onConnected = useCallback((connId: string, host: string, label: string) => {
    setConnectionId(connId);
    setServerHost(host);
    setServerLabel(label);
    void runPreflight(connId);
  }, [runPreflight]);

  // Re-probe when the user changes the relay listen port on the check step.
  useEffect(() => {
    if (step !== 'preflight' || !connectionId || activeTask) return;
    const port = parseRelayPort(relayPortInput);
    if (port == null) return;
    if (preflight?.probedPort === port) return;
    const timer = window.setTimeout(() => {
      void runPreflight(connectionId, port);
    }, 450);
    return () => window.clearTimeout(timer);
  }, [step, connectionId, relayPortInput, preflight?.probedPort, activeTask, runPreflight]);

  // ── connect handlers ─────────────────────────────────────────────────────
  const buildAuthMethod = (): SSHAuthMethod => {
    if (formData.authType === 'password') {
      return { type: 'Password', password: formData.password };
    }
    return {
      type: 'PrivateKey',
      keyPath: formData.keyPath,
      passphrase: formData.passphrase || undefined,
    };
  };

  const handleFormConnect = async () => {
    if (!formData.host.trim()) { setError(t('ssh.remote.hostRequired')); return; }
    if (!formData.username.trim()) { setError(t('ssh.remote.usernameRequired')); return; }
    const port = parseInt(formData.port, 10);
    if (isNaN(port) || port < 1 || port > 65535) { setError(t('ssh.remote.portInvalid')); return; }
    if (formData.authType === 'password' && !formData.password) { setError(t('ssh.remote.passwordRequired')); return; }
    if (formData.authType === 'privateKey' && !formData.keyPath.trim()) { setError(t('ssh.remote.keyPathRequired')); return; }

    const hostInput = formData.host.trim();
    let connectHost = hostInput;
    try {
      const lookup = await sshApi.getSSHConfig(hostInput);
      const resolved = lookup.found && lookup.config?.hostname?.trim();
      if (resolved) connectHost = resolved;
    } catch {
      // proceed with the raw host
    }

    const config: SSHConnectionConfig = {
      id: generateConnectionId(connectHost, formData.username.trim()),
      name: formData.name || `${formData.username.trim()}@${hostInput}`,
      host: connectHost,
      port,
      username: formData.username.trim(),
      auth: buildAuthMethod(),
    };

    setConnecting(true);
    setError(null);
    try {
      const result = await sshApi.connect(config);
      onConnected(result.connectionId || config.id, connectHost, config.name);
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setConnecting(false);
    }
  };

  const handleQuickConnect = async (conn: SavedConnection) => {
    setConnecting(true);
    setError(null);
    const auth: SSHAuthMethod = conn.authType.type === 'Password'
      ? { type: 'Password', password: '' }
      : { type: 'PrivateKey', keyPath: conn.authType.keyPath };
    try {
      const result = await sshApi.connect({
        id: conn.id,
        name: conn.name,
        host: conn.host,
        port: conn.port,
        username: conn.username,
        auth,
      });
      onConnected(result.connectionId || conn.id, conn.host, conn.name);
    } catch (e) {
      if (conn.authType.type === 'Password') {
        // No stored password in the vault — prompt for credentials.
        setCredentialsPrompt(conn);
      } else {
        setError(errMsg(e));
      }
    } finally {
      setConnecting(false);
    }
  };

  const handleCredentialsSubmit = async (payload: SSHAuthPromptSubmitPayload) => {
    const conn = credentialsPrompt;
    if (!conn) return;
    setConnecting(true);
    setError(null);
    try {
      const result = await sshApi.connect({
        id: conn.id,
        name: conn.name,
        host: conn.host,
        port: conn.port,
        username: payload.username,
        auth: payload.auth,
      });
      setCredentialsPrompt(null);
      onConnected(result.connectionId || conn.id, conn.host, conn.name);
    } catch (e) {
      setError(errMsg(e));
      setCredentialsPrompt(null);
    } finally {
      setConnecting(false);
    }
  };

  const handleFillFromConfig = (entry: SSHConfigEntry) => {
    const hasKey = !!entry.identityFile?.trim();
    setFormData({
      name: entry.host,
      host: entry.host,
      port: entry.port ? String(entry.port) : '22',
      username: entry.user || '',
      authType: hasKey ? 'privateKey' : 'password',
      password: '',
      keyPath: entry.identityFile?.trim() || '~/.ssh/id_rsa',
      passphrase: '',
    });
    // Config list sits above the form; scroll so the filled fields are visible.
    requestAnimationFrame(() => revealConnectForm());
  };

  const handleBrowsePrivateKey = useCallback(async () => {
    const path = await pickSshPrivateKeyPath({ title: t('ssh.remote.pickPrivateKeyDialogTitle') });
    if (path) setFormData((prev) => ({ ...prev, keyPath: path }));
  }, [t]);

  // ── status polling (PTY shows live output; poll only drives wizard state) ─
  const startTaskPolling = useCallback((task: RelayDeployTask, connId: string) => {
    stopPolling();
    cursorRef.current = 0;
    pollFailuresRef.current = 0;
    pollActiveRef.current = true;

    const pollOnce = async (): Promise<boolean> => {
      if (!pollActiveRef.current) return false;
      try {
        const res = await relayDeployApi.poll(connId, task, cursorRef.current);
        if (!pollActiveRef.current) return false;
        cursorRef.current = res.cursor;
        pollFailuresRef.current = 0;
        if (res.status !== 'running') {
          stopPolling();
          setTaskStatus(res.status);
          if (res.status === 'succeeded') {
            if (task === 'install_docker') {
              setActiveTask(null);
              void closeDeployTerminal();
              void runPreflight(connId);
            } else {
              window.setTimeout(() => setStep('register'), 800);
            }
          }
          return false;
        }
      } catch (e) {
        if (!pollActiveRef.current) return false;
        pollFailuresRef.current += 1;
        log.warn('task poll failed', e);
        if (pollFailuresRef.current >= MAX_POLL_FAILURES) {
          stopPolling();
          setTaskStatus('failed');
          setError(`[poll] ${errMsg(e)}`);
          return false;
        }
      }
      return pollActiveRef.current;
    };

    const scheduleNext = () => {
      if (!pollActiveRef.current) return;
      pollRef.current = setTimeout(() => {
        void pollOnce().then((shouldContinue) => {
          if (shouldContinue) scheduleNext();
        });
      }, POLL_INTERVAL_MS);
    };

    void pollOnce().then((shouldContinue) => {
      if (shouldContinue) scheduleNext();
    });
  }, [runPreflight, stopPolling, closeDeployTerminal]);

  const launchInteractiveTask = useCallback(async (
    task: RelayDeployTask,
    connId: string,
    scriptPath: string,
  ) => {
    await closeDeployTerminal();
    const session = await getTerminalService().createSession({
      connectionId: connId,
      name: task === 'deploy' ? 'Relay Deploy' : 'Relay Docker Install',
      cols: 100,
      rows: 28,
      source: 'manual',
    });
    terminalSessionIdRef.current = session.id;
    setTerminalSessionId(session.id);
    // Give the shell a moment to print its prompt before sending the command.
    await new Promise((r) => window.setTimeout(r, 400));
    const quoted = `'${scriptPath.replace(/'/g, `'\\''`)}'`;
    await getTerminalService().sendCommand(session.id, `bash ${quoted}`);
    startTaskPolling(task, connId);
  }, [closeDeployTerminal, startTaskPolling]);

  const handleInstallDocker = async () => {
    if (!connectionId) return;
    setError(null);
    setTaskStatus('running');
    setActiveTask('install_docker');
    try {
      const started = await relayDeployApi.installDocker(connectionId);
      await launchInteractiveTask('install_docker', connectionId, started.scriptPath);
    } catch (e) {
      setTaskStatus('failed');
      setError(`[start] ${errMsg(e)}`);
    }
  };

  const handleStartDeploy = async () => {
    if (!connectionId) return;
    const port = parseRelayPort(relayPortInput);
    if (port == null) {
      setError(t('relayDeploy.portInvalid'));
      return;
    }
    setError(null);
    setStep('deploy');
    setTaskStatus('running');
    setActiveTask('deploy');
    try {
      const started = await relayDeployApi.startDeploy(connectionId, port);
      await launchInteractiveTask('deploy', connectionId, started.scriptPath);
    } catch (e) {
      setTaskStatus('failed');
      setError(`[start] ${errMsg(e)}`);
    }
  };

  // ── register / finish ────────────────────────────────────────────────────
  const handleRegister = async () => {
    if (!connectionId) return;
    if (!regUsername.trim()) { setError(t('relayDeploy.usernameRequired')); return; }
    if (regPassword.length < 8) { setError(t('relayDeploy.passwordTooShort')); return; }
    if (regPassword !== regConfirm) { setError(t('relayDeploy.passwordMismatch')); return; }
    setRegLoading(true);
    setError(null);
    try {
      await relayDeployApi.register(connectionId, regUsername.trim(), regPassword);
      const v = await relayDeployApi
        .verify(relayUrl)
        .catch(() => ({ reachable: false, version: null }) as RelayVerifyResult);
      setVerify(v);
      setStep('done');
    } catch (e) {
      const msg = errMsg(e);
      // Redeploy keeps the database — point the user at the existing-account
      // mode instead of leaving a raw relay error.
      setError(/already exists/i.test(msg)
        ? `${msg} — ${t('relayDeploy.accountExistsHint')}`
        : msg);
    } finally {
      setRegLoading(false);
    }
  };

  /** Existing-account mode: no provisioning; the caller performs the login. */
  const handleUseExisting = () => {
    if (!regUsername.trim()) { setError(t('relayDeploy.usernameRequired')); return; }
    if (!regPassword) { setError(t('accountLogin.emptyFields')); return; }
    setError(null);
    onRegistered({ relayUrl, username: regUsername.trim(), password: regPassword });
  };

  const handleFinish = () => {
    onRegistered({ relayUrl, username: regUsername.trim(), password: regPassword });
  };

  const handleBackToPreflight = () => {
    stopPolling();
    const cancelSnapshot = {
      connectionId: connectionIdRef.current,
      task: activeTaskRef.current,
      status: taskStatusRef.current,
    };
    void (async () => {
      await cancelRemoteTaskIfRunning(cancelSnapshot);
      await closeDeployTerminal();
    })();
    setActiveTask(null);
    setTaskStatus(null);
    if (connectionId) {
      void runPreflight(connectionId);
    } else {
      setStep('connect');
    }
  };

  const dockerAccessHint = (mode: DockerAccessMode | undefined): string => {
    switch (mode) {
      case 'ok':
        return t('relayDeploy.checkDockerOk');
      case 'group_inactive':
        return t('relayDeploy.checkDockerGroupInactive');
      case 'sudo_nopass':
        return t('relayDeploy.checkDockerSudoNopass');
      case 'sudo_needs_password':
        return t('relayDeploy.checkDockerSudoPassword');
      case 'broken_docker_home':
        return t('relayDeploy.checkDockerHomeBroken');
      case 'daemon_down':
        return t('relayDeploy.checkDockerDaemonDown');
      case 'missing':
        return t('relayDeploy.checkDockerMissing');
      default:
        return t('relayDeploy.checkDockerMissing');
    }
  };

  // ── derived view data ────────────────────────────────────────────────────
  const filteredSavedConnections = savedConnections.filter((conn) => {
    if (!savedSearch.trim()) return true;
    const q = savedSearch.toLowerCase();
    return (
      conn.name.toLowerCase().includes(q) ||
      conn.host.toLowerCase().includes(q) ||
      conn.username.toLowerCase().includes(q)
    );
  });

  const filteredSSHConfigHosts = sshConfigHosts.filter((entry) => {
    const hostname = entry.hostname || entry.host;
    const port = entry.port || 22;
    const user = entry.user || '';
    if (savedConnections.some((c) => c.host === hostname && c.port === port && c.username === user)) {
      return false;
    }
    if (!configSearch.trim()) return true;
    const q = configSearch.toLowerCase();
    return (
      entry.host.toLowerCase().includes(q) ||
      hostname.toLowerCase().includes(q) ||
      user.toLowerCase().includes(q)
    );
  });

  const accessMode = preflight?.dockerAccessMode;
  const dockerRecoverable = !!preflight && preflight.dockerInstalled
    && accessMode !== 'missing'
    && (preflight.composeAvailable
      || accessMode === 'sudo_nopass'
      || accessMode === 'sudo_needs_password'
      || accessMode === 'group_inactive'
      || accessMode === 'broken_docker_home'
      || accessMode === 'daemon_down');
  const canInstallDocker = !!preflight && !preflight.dockerInstalled
    && (preflight.sudoAvailable || preflight.sudoNeedsPassword);
  const portValid = parseRelayPort(relayPortInput) != null;
  const canDeploy = !!preflight && preflight.archSupported
    && preflight.curlAvailable && preflight.tarAvailable
    && dockerRecoverable
    && portValid
    && (!preflight.portBusy || preflight.portOwnedByRelay);

  const steps: Array<{ key: Step; label: string }> = [
    { key: 'connect', label: t('relayDeploy.stepServer') },
    { key: 'preflight', label: t('relayDeploy.stepCheck') },
    { key: 'deploy', label: t('relayDeploy.stepDeploy') },
    { key: 'register', label: t('relayDeploy.stepAccount') },
    { key: 'done', label: t('shared:statuses.done') },
  ];
  const stepIndex = steps.findIndex((s) => s.key === step);

  const authOptions = [
    { label: t('ssh.remote.password'), value: 'password', icon: <Lock size={14} /> },
    { label: t('ssh.remote.privateKey'), value: 'privateKey', icon: <Key size={14} /> },
  ];

  // ── step renderers ───────────────────────────────────────────────────────
  const renderConnect = () => (
    <div className="relay-deploy-wizard__scroll">
      <p className="relay-deploy-wizard__desc">{t('relayDeploy.selectServerDesc')}</p>

      {savedConnections.length > 0 && (
        <div className="relay-deploy-wizard__section">
          <div className="relay-deploy-wizard__section-header">
            <h3 className="relay-deploy-wizard__section-title">{t('ssh.remote.savedConnections')}</h3>
            <Input
              className="relay-deploy-wizard__search"
              value={savedSearch}
              onChange={(e) => setSavedSearch(e.target.value)}
              placeholder={t('actions.search')}
              prefix={<Search size={14} />}
              size="small"
            />
          </div>
          <div className="relay-deploy-wizard__server-list">
            {filteredSavedConnections.map((conn) => (
              <div
                key={conn.id}
                className="relay-deploy-wizard__server-item"
                onClick={() => !connecting && handleQuickConnect(conn)}
                role="button"
                tabIndex={0}
                onKeyDown={(e) => e.key === 'Enter' && !connecting && handleQuickConnect(conn)}
              >
                <div className="relay-deploy-wizard__server-icon"><Server size={16} /></div>
                <div className="relay-deploy-wizard__server-info">
                  <span className="relay-deploy-wizard__server-name">{conn.name}</span>
                  <span className="relay-deploy-wizard__server-detail">
                    {conn.username}@{conn.host}:{conn.port}
                  </span>
                </div>
                <Button size="small" variant="primary" disabled={connecting}
                  onClick={(e) => { e.stopPropagation(); handleQuickConnect(conn); }}>
                  <Play size={12} />
                </Button>
              </div>
            ))}
          </div>
        </div>
      )}

      {filteredSSHConfigHosts.length > 0 && (
        <div className="relay-deploy-wizard__section">
          <div className="relay-deploy-wizard__section-header">
            <h3 className="relay-deploy-wizard__section-title">{t('ssh.remote.sshConfigHosts')}</h3>
            <Input
              className="relay-deploy-wizard__search"
              value={configSearch}
              onChange={(e) => setConfigSearch(e.target.value)}
              placeholder={t('actions.search')}
              prefix={<Search size={14} />}
              size="small"
            />
          </div>
          <div className="relay-deploy-wizard__server-list">
            {filteredSSHConfigHosts.map((entry) => (
              <div
                key={entry.host}
                className="relay-deploy-wizard__server-item relay-deploy-wizard__server-item--config"
                onClick={() => handleFillFromConfig(entry)}
                role="button"
                tabIndex={0}
                onKeyDown={(e) => e.key === 'Enter' && handleFillFromConfig(entry)}
              >
                <div className="relay-deploy-wizard__server-icon"><Server size={16} /></div>
                <div className="relay-deploy-wizard__server-info">
                  <span className="relay-deploy-wizard__server-name">{entry.host}</span>
                  <span className="relay-deploy-wizard__server-detail">
                    {entry.user || ''}@{entry.hostname || entry.host}:{entry.port || 22}
                  </span>
                </div>
                <Button size="small" variant="ghost" disabled={connecting}
                  onClick={(e) => { e.stopPropagation(); handleFillFromConfig(entry); }}>
                  <ArrowDownToLine size={12} />
                  {t('ssh.remote.fillForm')}
                </Button>
              </div>
            ))}
          </div>
        </div>
      )}

      {(savedConnections.length > 0 || filteredSSHConfigHosts.length > 0) && (
        <div className="relay-deploy-wizard__divider"><span>{t('ssh.remote.newConnection')}</span></div>
      )}

      <div
        ref={connectFormRef}
        className={[
          'relay-deploy-wizard__form',
          connectFormHighlighted ? 'relay-deploy-wizard__form--highlighted' : '',
        ].filter(Boolean).join(' ')}
      >
        <div className="relay-deploy-wizard__row">
          <div className="relay-deploy-wizard__field relay-deploy-wizard__field--flex">
            <Input label={t('ssh.remote.host')} value={formData.host}
              onChange={(e) => setFormData((p) => ({ ...p, host: e.target.value }))}
              prefix={<Server size={16} />} size="medium" disabled={connecting} />
          </div>
          <div className="relay-deploy-wizard__field relay-deploy-wizard__field--port">
            <Input label={t('ssh.remote.port')} value={formData.port}
              onChange={(e) => setFormData((p) => ({ ...p, port: e.target.value }))}
              placeholder="22" size="medium" disabled={connecting} />
          </div>
        </div>
        <div className="relay-deploy-wizard__field">
          <Input label={t('ssh.remote.username')} value={formData.username}
            onChange={(e) => setFormData((p) => ({ ...p, username: e.target.value }))}
            prefix={<User size={16} />} size="medium" disabled={connecting} />
        </div>
        <div className="relay-deploy-wizard__field">
          <label className="relay-deploy-wizard__label">{t('ssh.remote.authMethod')}</label>
          <Select options={authOptions} value={formData.authType}
            onChange={(v) => setFormData((p) => ({ ...p, authType: String(v) as 'password' | 'privateKey' }))}
            size="medium" disabled={connecting} />
        </div>
        {formData.authType === 'password' && (
          <div className="relay-deploy-wizard__field">
            <Input label={t('ssh.remote.password')} type={showPassword ? 'text' : 'password'}
              value={formData.password}
              onChange={(e) => setFormData((p) => ({ ...p, password: e.target.value }))}
              prefix={<Lock size={16} />} size="medium" disabled={connecting}
              suffix={
                <button type="button" className="bitfun-input-toggle" onClick={() => setShowPassword((s) => !s)} tabIndex={-1}>
                  {showPassword ? <EyeOff size={16} /> : <Eye size={16} />}
                </button>
              } />
          </div>
        )}
        {formData.authType === 'privateKey' && (
          <>
            <div className="relay-deploy-wizard__field">
              <Input label={t('ssh.remote.privateKeyPath')} value={formData.keyPath}
                onChange={(e) => setFormData((p) => ({ ...p, keyPath: e.target.value }))}
                placeholder="~/.ssh/id_rsa" prefix={<Key size={16} />} size="medium"
                disabled={connecting}
                suffix={
                  <IconButton type="button" variant="ghost" size="small"
                    tooltip={t('ssh.remote.browsePrivateKey')}
                    aria-label={t('ssh.remote.browsePrivateKey')}
                    disabled={connecting}
                    onClick={() => void handleBrowsePrivateKey()}>
                    <FolderOpen size={16} />
                  </IconButton>
                } />
            </div>
            <div className="relay-deploy-wizard__field">
              <Input label={t('ssh.remote.passphrase')} type={showPassphrase ? 'text' : 'password'}
                value={formData.passphrase}
                onChange={(e) => setFormData((p) => ({ ...p, passphrase: e.target.value }))}
                placeholder={t('ssh.remote.passphraseOptional')} size="medium" disabled={connecting}
                suffix={
                  <button type="button" className="bitfun-input-toggle" onClick={() => setShowPassphrase((s) => !s)} tabIndex={-1}>
                    {showPassphrase ? <EyeOff size={16} /> : <Eye size={16} />}
                  </button>
                } />
            </div>
          </>
        )}
      </div>

      <div className="relay-deploy-wizard__actions">
        <Button variant="secondary" size="small" onClick={onClose} disabled={connecting}>
          {t('actions.cancel')}
        </Button>
        <Button variant="primary" size="small" onClick={handleFormConnect}
          disabled={connecting || !formData.host.trim() || !formData.username.trim()}>
          {connecting ? (
            <><Loader2 size={14} className="spinning" />{t('ssh.remote.connecting')}</>
          ) : (
            <><Play size={14} />{t('relayDeploy.connectAndContinue')}</>
          )}
        </Button>
      </div>
    </div>
  );

  const renderCheckRow = (
    ok: boolean | 'warn',
    label: string,
    detail: string,
  ) => (
    <div className="relay-deploy-wizard__check-row">
      {ok === true && <CheckCircle2 size={15} className="relay-deploy-wizard__check-icon relay-deploy-wizard__check-icon--ok" />}
      {ok === 'warn' && <AlertTriangle size={15} className="relay-deploy-wizard__check-icon relay-deploy-wizard__check-icon--warn" />}
      {ok === false && <XCircle size={15} className="relay-deploy-wizard__check-icon relay-deploy-wizard__check-icon--fail" />}
      <span className="relay-deploy-wizard__check-label">{label}</span>
      <span className="relay-deploy-wizard__check-detail">{detail}</span>
    </div>
  );

  const renderPreflight = () => {
    const pf = preflight;
    const taskRunning = activeTask === 'install_docker' && taskStatus === 'running';
    const taskFailed = activeTask === 'install_docker' && taskStatus === 'failed';
    const dockerOk = pf?.dockerAccessMode === 'ok';
    const dockerWarn = !!pf?.dockerInstalled && !dockerOk && pf.dockerAccessMode !== 'missing';
    return (
      <div className="relay-deploy-wizard__scroll">
        <div className="relay-deploy-wizard__server-banner">
          <Server size={14} />
          <span>{serverLabel}</span>
        </div>

        {preflightLoading || !pf ? (
          <div className="relay-deploy-wizard__checking">
            <Loader2 size={18} className="spinning" />
            <span>{t('relayDeploy.checking')}</span>
          </div>
        ) : (
          <>
            {alreadyDeployed && (
              <div className="relay-deploy-wizard__notice relay-deploy-wizard__notice--info">
                <CheckCircle2 size={18} />
                <div className="relay-deploy-wizard__notice-text">
                  <span className="relay-deploy-wizard__notice-title">{t('relayDeploy.alreadyDeployedTitle')}</span>
                  <span className="relay-deploy-wizard__notice-desc">
                    {existingRelayPort && existingRelayPort !== relayPort
                      ? t('relayDeploy.alreadyDeployedDescOtherPort', { port: existingRelayPort })
                      : t('relayDeploy.alreadyDeployedDesc')}
                  </span>
                </div>
              </div>
            )}

            <div className="relay-deploy-wizard__port-row">
              <div className="relay-deploy-wizard__field relay-deploy-wizard__field--port">
                <Input
                  label={t('relayDeploy.relayPort')}
                  type="number"
                  value={relayPortInput}
                  onChange={(e) => setRelayPortInput(e.target.value)}
                  size="medium"
                  disabled={taskRunning || preflightLoading}
                  min={1}
                  max={65535}
                />
              </div>
              <p className="relay-deploy-wizard__port-hint">{t('relayDeploy.relayPortHint')}</p>
            </div>

            {portConflict && (
              <div className="relay-deploy-wizard__notice relay-deploy-wizard__notice--warn">
                <AlertTriangle size={18} />
                <div className="relay-deploy-wizard__notice-text">
                  <span className="relay-deploy-wizard__notice-title">
                    {t('relayDeploy.portConflictTitle', { port: relayPort })}
                  </span>
                  <span className="relay-deploy-wizard__notice-desc">
                    {t('relayDeploy.portConflictDesc')}
                  </span>
                </div>
              </div>
            )}
            {!portValid && (
              <div className="relay-deploy-wizard__notice relay-deploy-wizard__notice--warn">
                <AlertTriangle size={18} />
                <div className="relay-deploy-wizard__notice-text">
                  <span className="relay-deploy-wizard__notice-desc">{t('relayDeploy.portInvalid')}</span>
                </div>
              </div>
            )}

            <div className="relay-deploy-wizard__checks">
              {renderCheckRow(
                pf.archSupported,
                t('relayDeploy.checkOs'),
                pf.archSupported
                  ? `${pf.os} / ${pf.arch}`
                  : `${pf.os} / ${pf.arch} — ${t('relayDeploy.checkOsUnsupported')}`,
              )}
              {renderCheckRow(
                dockerOk ? true : dockerWarn ? 'warn' : false,
                t('relayDeploy.checkDocker'),
                dockerAccessHint(pf.dockerAccessMode),
              )}
              {renderCheckRow(
                !pf.dockerInstalled ? 'warn' : pf.composeAvailable,
                t('relayDeploy.checkCompose'),
                pf.composeAvailable ? t('relayDeploy.checkDockerOk') : t('relayDeploy.checkComposeMissing'),
              )}
              {renderCheckRow(
                pf.curlAvailable,
                'curl',
                pf.curlAvailable ? t('relayDeploy.checkDockerOk') : t('relayDeploy.checkMissing'),
              )}
              {renderCheckRow(
                pf.tarAvailable,
                'tar',
                pf.tarAvailable ? t('relayDeploy.checkDockerOk') : t('relayDeploy.checkMissing'),
              )}
              {renderCheckRow(
                pf.memTotalMb === 0 ? 'warn' : pf.memTotalMb >= 2048 ? true : 'warn',
                t('relayDeploy.checkMemory'),
                pf.memTotalMb >= 2048
                  ? t('relayDeploy.checkMemoryValue', { mb: pf.memTotalMb })
                  : `${t('relayDeploy.checkMemoryValue', { mb: pf.memTotalMb })} — ${t('relayDeploy.checkMemoryLow')}`,
              )}
              {renderCheckRow(
                !pf.portBusy || pf.portOwnedByRelay,
                t('relayDeploy.checkPort', { port: pf.probedPort || relayPort }),
                !pf.portBusy
                  ? t('relayDeploy.checkPortFree')
                  : pf.portOwnedByRelay
                    ? t('relayDeploy.checkPortOwned')
                    : t('relayDeploy.checkPortBusy'),
              )}
              {!pf.dockerInstalled && renderCheckRow(
                pf.sudoAvailable || pf.sudoNeedsPassword ? (pf.sudoAvailable ? true : 'warn') : false,
                t('relayDeploy.checkSudo'),
                pf.sudoAvailable
                  ? t('relayDeploy.checkSudoOk')
                  : pf.sudoNeedsPassword
                    ? t('relayDeploy.checkSudoPasswordOk')
                    : t('relayDeploy.checkSudoMissing'),
              )}
            </div>

            {dockerWarn && !taskRunning && (
              <p className="relay-deploy-wizard__hint">{t('relayDeploy.interactiveTerminalHint')}</p>
            )}

            {(taskRunning || taskFailed) && terminalSessionId && (
              <div className="relay-deploy-wizard__terminal">
                <ConnectedTerminal
                  sessionId={terminalSessionId}
                  showToolbar={false}
                  showStatusBar={false}
                  autoFocus
                  options={DEPLOY_TERMINAL_OPTIONS}
                />
              </div>
            )}
            {taskRunning && (
              <div className="relay-deploy-wizard__task-status">
                <Loader2 size={14} className="spinning" />
                <span>{t('relayDeploy.installingDocker')}</span>
              </div>
            )}
            {taskFailed && (
              <div className="relay-deploy-wizard__task-status relay-deploy-wizard__task-status--failed">
                <XCircle size={14} />
                <span>{t('relayDeploy.dockerInstallFailed')}</span>
              </div>
            )}

            <div className="relay-deploy-wizard__actions">
              {alreadyDeployed ? (
                <>
                  <Button variant="secondary" size="small" onClick={handleStartDeploy} disabled={taskRunning}>
                    <Rocket size={14} />
                    {t('relayDeploy.redeploy')}
                  </Button>
                  <Button
                    variant="primary"
                    size="small"
                    onClick={() => {
                      // Account creation must hit the running relay, not the
                      // (possibly different) redeploy listen port.
                      if (existingRelayPort) {
                        setRelayPortInput(String(existingRelayPort));
                      }
                      setStep('register');
                    }}
                    disabled={taskRunning || !pf.relayHealthy}
                  >
                    <User size={14} />
                    {t('relayDeploy.skipToRegister')}
                  </Button>
                </>
              ) : (
                <>
                  <Button variant="secondary" size="small" onClick={handleBackToPreflight} disabled={taskRunning || preflightLoading}>
                    <ChevronLeft size={14} />
                    {t('relayDeploy.back')}
                  </Button>
                  {canInstallDocker && (
                    <Button variant="secondary" size="small" onClick={handleInstallDocker} disabled={taskRunning}>
                      {taskRunning ? <Loader2 size={14} className="spinning" /> : <RefreshCw size={14} />}
                      {t('relayDeploy.installDocker')}
                    </Button>
                  )}
                  {!pf.dockerInstalled && !canInstallDocker && !taskRunning && (
                    <span className="relay-deploy-wizard__hint">{t('relayDeploy.dockerManualHint')}</span>
                  )}
                  <Button variant="primary" size="small" onClick={handleStartDeploy}
                    disabled={!canDeploy || taskRunning}>
                    <Rocket size={14} />
                    {t('relayDeploy.startDeploy')}
                  </Button>
                </>
              )}
            </div>
          </>
        )}
      </div>
    );
  };

  const renderDeploy = () => (
    <div className="relay-deploy-wizard__scroll">
      <div className="relay-deploy-wizard__server-banner">
        <Server size={14} />
        <span>{serverLabel}</span>
      </div>
      <div className="relay-deploy-wizard__task-header">
        {taskStatus === 'running' && (
          <>
            <Loader2 size={16} className="spinning" />
            <div className="relay-deploy-wizard__task-header-text">
              <span className="relay-deploy-wizard__task-title">{t('relayDeploy.deployingTitle')}</span>
              <span className="relay-deploy-wizard__task-desc">{t('relayDeploy.deployingHint')}</span>
            </div>
          </>
        )}
        {taskStatus === 'succeeded' && (
          <>
            <CheckCircle2 size={16} className="relay-deploy-wizard__check-icon--ok" />
            <div className="relay-deploy-wizard__task-header-text">
              <span className="relay-deploy-wizard__task-title">{t('relayDeploy.deploySucceeded')}</span>
            </div>
          </>
        )}
        {taskStatus === 'failed' && (
          <>
            <XCircle size={16} className="relay-deploy-wizard__check-icon--fail" />
            <div className="relay-deploy-wizard__task-header-text">
              <span className="relay-deploy-wizard__task-title">{t('relayDeploy.deployFailed')}</span>
            </div>
          </>
        )}
      </div>
      {terminalSessionId ? (
        <div className="relay-deploy-wizard__terminal relay-deploy-wizard__terminal--large">
          <ConnectedTerminal
            sessionId={terminalSessionId}
            showToolbar={false}
            showStatusBar={false}
            autoFocus
            options={DEPLOY_TERMINAL_OPTIONS}
          />
        </div>
      ) : (
        <div className="relay-deploy-wizard__checking relay-deploy-wizard__checking--terminal">
          <Loader2 size={18} className="spinning" />
          <span>{t('relayDeploy.openingTerminal')}</span>
        </div>
      )}
      <div className="relay-deploy-wizard__actions">
        <Button variant="secondary" size="small" onClick={handleBackToPreflight}
          disabled={taskStatus === 'running'}>
          <ChevronLeft size={14} />
          {t('relayDeploy.back')}
        </Button>
        {taskStatus === 'failed' && (
          <Button variant="primary" size="small" onClick={handleStartDeploy}>
            <RefreshCw size={14} />
            {t('relayDeploy.retry')}
          </Button>
        )}
      </div>
    </div>
  );

  const renderRegister = () => (
    <div className="relay-deploy-wizard__scroll">
      <div className="relay-deploy-wizard__server-banner">
        <Server size={14} />
        <span>{relayUrl}</span>
      </div>
      <div className="relay-deploy-wizard__reg-mode">
        <button
          type="button"
          className={`relay-deploy-wizard__reg-mode-btn${regMode === 'create' ? ' is-active' : ''}`}
          onClick={() => { setRegMode('create'); setError(null); }}
          disabled={regLoading}
        >
          {t('relayDeploy.regModeCreate')}
        </button>
        <button
          type="button"
          className={`relay-deploy-wizard__reg-mode-btn${regMode === 'existing' ? ' is-active' : ''}`}
          onClick={() => { setRegMode('existing'); setError(null); }}
          disabled={regLoading}
        >
          {t('relayDeploy.regModeExisting')}
        </button>
      </div>
      <div className="relay-deploy-wizard__notice relay-deploy-wizard__notice--info">
        <User size={18} />
        <div className="relay-deploy-wizard__notice-text">
          <span className="relay-deploy-wizard__notice-title">
            {regMode === 'create' ? t('relayDeploy.registerTitle') : t('relayDeploy.registerExistingTitle')}
          </span>
          <span className="relay-deploy-wizard__notice-desc">
            {regMode === 'create' ? t('relayDeploy.registerDesc') : t('relayDeploy.registerExistingDesc')}
          </span>
        </div>
      </div>
      <div className="relay-deploy-wizard__form">
        <div className="relay-deploy-wizard__field">
          <Input label={t('accountLogin.username')} type="text" value={regUsername}
            onChange={(e) => setRegUsername(e.target.value)}
            prefix={<User size={16} />} size="medium" disabled={regLoading} />
        </div>
        <div className="relay-deploy-wizard__field">
          <Input label={t('accountLogin.password')} type={showRegPassword ? 'text' : 'password'}
            value={regPassword} onChange={(e) => setRegPassword(e.target.value)}
            prefix={<Lock size={16} />} size="medium" disabled={regLoading}
            suffix={
              <button type="button" className="bitfun-input-toggle" onClick={() => setShowRegPassword((s) => !s)} tabIndex={-1}>
                {showRegPassword ? <EyeOff size={16} /> : <Eye size={16} />}
              </button>
            } />
        </div>
        {regMode === 'create' && (
          <div className="relay-deploy-wizard__field">
            <Input label={t('relayDeploy.confirmPassword')} type={showRegPassword ? 'text' : 'password'}
              value={regConfirm} onChange={(e) => setRegConfirm(e.target.value)}
              prefix={<Lock size={16} />} size="medium" disabled={regLoading} />
          </div>
        )}
      </div>
      <div className="relay-deploy-wizard__actions">
        <Button variant="secondary" size="small" onClick={handleBackToPreflight} disabled={regLoading}>
          <ChevronLeft size={14} />
          {t('relayDeploy.back')}
        </Button>
        {regMode === 'create' ? (
          <Button variant="primary" size="small" onClick={handleRegister}
            disabled={regLoading || !regUsername.trim() || !regPassword || !regConfirm}>
            {regLoading ? (
              <><Loader2 size={14} className="spinning" />{t('relayDeploy.creatingAccount')}</>
            ) : (
              <><User size={14} />{t('relayDeploy.createAccount')}</>
            )}
          </Button>
        ) : (
          <Button variant="primary" size="small" onClick={handleUseExisting}
            disabled={regLoading || !regUsername.trim() || !regPassword}>
            <CheckCircle2 size={14} />
            {t('relayDeploy.finishAndLogin')}
          </Button>
        )}
      </div>
    </div>
  );

  const renderDone = () => (
    <div className="relay-deploy-wizard__scroll">
      <div className="relay-deploy-wizard__done">
        <PartyPopper size={32} className="relay-deploy-wizard__done-icon" />
        <span className="relay-deploy-wizard__done-title">{t('relayDeploy.doneTitle')}</span>
        <code className="relay-deploy-wizard__done-url">{relayUrl}</code>
        {verify && (
          <div className={`relay-deploy-wizard__verify ${verify.reachable ? 'ok' : 'failed'}`}>
            {verify.reachable ? <CheckCircle2 size={14} /> : <AlertTriangle size={14} />}
            <span>{verify.reachable ? t('relayDeploy.verifyOk') : t('relayDeploy.verifyFailed')}</span>
          </div>
        )}
      </div>
      <div className="relay-deploy-wizard__actions">
        <Button variant="primary" size="small" onClick={handleFinish}>
          <CheckCircle2 size={14} />
          {t('relayDeploy.finishAndLogin')}
        </Button>
      </div>
    </div>
  );

  return (
    <>
      <Modal
        isOpen={isOpen}
        onClose={onClose}
        title={t('relayDeploy.title')}
        size="large"
        showCloseButton
        closeOnOverlayClick={false}
        contentClassName="modal__content--fill-flex"
      >
        <div className="relay-deploy-wizard">
          <div className="relay-deploy-wizard__steps">
            {steps.map((s, i) => (
              <React.Fragment key={s.key}>
                <div className={`relay-deploy-wizard__step ${i === stepIndex ? 'active' : ''} ${i < stepIndex ? 'completed' : ''}`}>
                  <span className="relay-deploy-wizard__step-dot">
                    {i < stepIndex ? <CheckCircle2 size={12} /> : i + 1}
                  </span>
                  <span className="relay-deploy-wizard__step-label">{s.label}</span>
                </div>
                {i < steps.length - 1 && <div className="relay-deploy-wizard__step-connector" />}
              </React.Fragment>
            ))}
          </div>

          {error && (
            <div className="relay-deploy-wizard__error-banner">
              <Alert type="error" message={error} closable onClose={() => setError(null)}
                className="relay-deploy-wizard__error-alert" />
            </div>
          )}

          {step === 'connect' && renderConnect()}
          {step === 'preflight' && renderPreflight()}
          {step === 'deploy' && renderDeploy()}
          {step === 'register' && renderRegister()}
          {step === 'done' && renderDone()}
        </div>
      </Modal>

      {credentialsPrompt && (
        <SSHAuthPromptDialog
          open
          targetDescription={`${credentialsPrompt.username}@${credentialsPrompt.host}:${credentialsPrompt.port}`}
          defaultAuthMethod="password"
          initialUsername={credentialsPrompt.username}
          lockUsername
          onSubmit={handleCredentialsSubmit}
          onCancel={() => setCredentialsPrompt(null)}
          isConnecting={connecting}
        />
      )}
    </>
  );
};

export default RelayDeployWizard;
