/**
 * SSH Connection Dialog Component
 * Professional SSH connection dialog following BitFun design patterns
 */

import React, { useState, useEffect, useCallback, useRef } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { useSSHRemoteContext } from './SSHRemoteContext';
import { SSHAuthPromptDialog, type SSHAuthPromptSubmitPayload } from './SSHAuthPromptDialog';
import { Modal } from '@/component-library';
import { Button } from '@/component-library';
import { Input } from '@/component-library';
import { Select } from '@/component-library';
import { Alert } from '@/component-library';
import { IconButton } from '@/component-library';
import {
  ArrowDownToLine,
  CheckCircle2,
  ChevronDown,
  Eye,
  EyeOff,
  FolderOpen,
  Key,
  Loader2,
  Lock,
  Pencil,
  Play,
  Plus,
  RefreshCw,
  Search,
  Server,
  Trash2,
  User,
  XCircle,
} from 'lucide-react';
import type {
  ConnectionTestReport,
  ConnectionTestStage,
  DockerContainerInfo,
  SSHConnectionConfig,
  SSHAuthMethod,
  SavedConnection,
  SSHConfigEntry,
} from './types';
import { sshApi } from './sshApi';
import {
  pickSshCertificatePath,
  pickSshPrivateKeyPath,
} from './pickSshPrivateKeyPath';
import './SSHConnectionDialog.scss';

interface SSHConnectionDialogProps {
  open: boolean;
  onClose: () => void;
}

export const SSHConnectionDialog: React.FC<SSHConnectionDialogProps> = ({
  open,
  onClose,
}) => {
  const { t } = useI18n('common');
  const { connect, status, connectionError, clearError } = useSSHRemoteContext();
  const [savedConnections, setSavedConnections] = useState<SavedConnection[]>([]);
  const [sshConfigHosts, setSSHConfigHosts] = useState<SSHConfigEntry[]>([]);
  const [localError, setLocalError] = useState<string | null>(null);
  const [isConnecting, setIsConnecting] = useState(false);
  const [credentialsPrompt, setCredentialsPrompt] = useState<SavedConnection | null>(null);
  const [savedSearch, setSavedSearch] = useState('');
  const [configSearch, setConfigSearch] = useState('');
  const [isTesting, setIsTesting] = useState(false);
  const [isListingContainers, setIsListingContainers] = useState(false);
  const [connectionTest, setConnectionTest] = useState<ConnectionTestReport | null>(null);
  const [dockerContainers, setDockerContainers] = useState<DockerContainerInfo[]>([]);

  const error = localError || connectionError;

  // Form state
  const [formData, setFormData] = useState({
    targetType: 'ssh' as 'ssh' | 'remoteDocker' | 'localDocker' | 'containerSshd',
    name: '',
    host: '',
    port: '22',
    username: '',
    authType: 'password' as 'password' | 'privateKey' | 'agent' | 'keyboardInteractive',
    password: '',
    keyPath: '~/.ssh/id_rsa',
    passphrase: '',
    certificatePath: '',
    keyFingerprint: '',
    fallbackKeyPath: '~/.ssh/id_rsa',
    verificationCode: '',
    proxyJump: '',
    containerName: '',
    containerAccess: 'auto' as 'auto' | 'docker-exec',
    dockerPath: 'docker',
    containerShell: '/bin/sh',
    containerUser: '',
    connectTimeoutSecs: '30',
    authTimeoutSecs: '60',
    authAttempts: '3',
    connectAttempts: '1',
  });

  const [showPassword, setShowPassword] = useState(false);
  const [showPassphrase, setShowPassphrase] = useState(false);
  const [showAdvancedSettings, setShowAdvancedSettings] = useState(false);
  const formRef = useRef<HTMLDivElement>(null);
  const formHighlightTimerRef = useRef<number | null>(null);
  const [formHighlighted, setFormHighlighted] = useState(false);

  const revealConnectionForm = useCallback(() => {
    const el = formRef.current;
    if (!el) return;
    el.scrollIntoView({ behavior: 'smooth', block: 'start' });
    setFormHighlighted(true);
    if (formHighlightTimerRef.current != null) {
      window.clearTimeout(formHighlightTimerRef.current);
    }
    formHighlightTimerRef.current = window.setTimeout(() => {
      setFormHighlighted(false);
      formHighlightTimerRef.current = null;
    }, 1200);
  }, []);

  useEffect(() => {
    return () => {
      if (formHighlightTimerRef.current != null) {
        window.clearTimeout(formHighlightTimerRef.current);
      }
    };
  }, []);

  async function loadSavedConnections() {
    setLocalError(null);
    try {
      const connections = await sshApi.listSavedConnections();
      setSavedConnections(connections);
    } catch (_error) {
      setSavedConnections([]);
    }
  }

  async function loadSSHConfigHosts() {
    try {
      const hosts = await sshApi.listSSHConfigHosts();
      setSSHConfigHosts(hosts);
    } catch (error) {
      console.error('Failed to load SSH config hosts:', error);
      setSSHConfigHosts([]);
    }
  }

  // Clear errors when dialog opens
  useEffect(() => {
    if (open) {
      clearError();
      setLocalError(null);
      setShowAdvancedSettings(false);
      setSavedSearch('');
      setConfigSearch('');
      setConnectionTest(null);
      setDockerContainers([]);
      void loadSavedConnections();
      void loadSSHConfigHosts();
    }
  }, [open, clearError]);

  // Load SSH config from ~/.ssh/config when host changes
  useEffect(() => {
    if (!formData.host.trim()) return;

    const loadSSHConfig = async () => {
      try {
        const result = await sshApi.getSSHConfig(formData.host.trim());
        if (result.found && result.config) {
          const config = result.config;
          if (config.proxyJump) {
            setShowAdvancedSettings(true);
          }
          // Auto-fill fields from SSH config if they're not already set
          setFormData((prev) => ({
            ...prev,
            port: config.port ? String(config.port) : prev.port,
            username: config.user || prev.username,
            keyPath: config.identityFile || prev.keyPath,
            certificatePath: config.certificateFile || prev.certificatePath,
            proxyJump: config.proxyJump || prev.proxyJump,
            authType: config.identityFile
              ? 'privateKey'
              : config.agent
                ? 'agent'
                : prev.authType,
          }));
        }
      } catch (e) {
        // Silently ignore SSH config errors
        console.debug('Failed to load SSH config:', e);
      }
    };

    // Debounce the SSH config lookup
    const timeout = setTimeout(loadSSHConfig, 300);
    return () => clearTimeout(timeout);
  }, [formData.host]);

  const handleInputChange = (field: string, value: string) => {
    setFormData((prev) => ({ ...prev, [field]: value }));
    setConnectionTest(null);
    if (field === 'targetType' || field === 'host' || field === 'dockerPath') {
      setDockerContainers([]);
    }
  };

  const handleBrowsePrivateKey = useCallback(async () => {
    if (isConnecting || status === 'connecting') return;
    const path = await pickSshPrivateKeyPath({
      title: t('ssh.remote.pickPrivateKeyDialogTitle'),
    });
    if (path) setFormData((prev) => ({ ...prev, keyPath: path }));
  }, [isConnecting, status, t]);

  const handleBrowseCertificate = useCallback(async () => {
    if (isConnecting || status === 'connecting') return;
    const path = await pickSshCertificatePath({
      title: t('ssh.remote.pickCertificateDialogTitle'),
    });
    if (path) setFormData((prev) => ({ ...prev, certificatePath: path }));
  }, [isConnecting, status, t]);

  // Port is intentionally excluded so that the ID stays stable when the user
  // changes the SSH port.  Old-format IDs that include the port (e.g.
  // "ssh-root@host:22") are migrated on the Rust side when saved connections
  // are loaded from disk.
  const generateConnectionId = (host: string, _port: number, username: string) => {
    return `ssh-${username}@${host}`;
  };

  const buildAuthMethod = (): SSHAuthMethod => {
    switch (formData.authType) {
      case 'password':
        return { type: 'Password', password: formData.password };
      case 'privateKey':
        return {
          type: 'PrivateKey',
          keyPath: formData.keyPath,
          passphrase: formData.passphrase || undefined,
          certificatePath: formData.certificatePath.trim() || undefined,
        };
      case 'agent':
        return {
          type: 'Agent',
          keyFingerprint: formData.keyFingerprint.trim() || undefined,
          fallbackKeyPath: formData.fallbackKeyPath.trim() || undefined,
        };
      case 'keyboardInteractive':
        return {
          type: 'KeyboardInteractive',
          responses: [formData.password, formData.verificationCode].filter(Boolean),
        };
    }
  };

  const buildConnectionConfig = async (
    requireContainer: boolean,
  ): Promise<SSHConnectionConfig | null> => {
    const isLocalDocker = formData.targetType === 'localDocker';
    const usesContainer = formData.targetType !== 'ssh';
    if (!isLocalDocker && !formData.host.trim()) {
      setLocalError(t('ssh.remote.hostRequired'));
      return null;
    }
    if (!isLocalDocker && !formData.username.trim()) {
      setLocalError(t('ssh.remote.usernameRequired'));
      return null;
    }
    const port = parseInt(formData.port, 10);
    if (isNaN(port) || port < 1 || port > 65535) {
      setLocalError(t('ssh.remote.portInvalid'));
      return null;
    }
    if (!isLocalDocker && formData.authType === 'password' && !formData.password) {
      setLocalError(t('ssh.remote.passwordRequired'));
      return null;
    }
    if (!isLocalDocker && formData.authType === 'privateKey' && !formData.keyPath.trim()) {
      setLocalError(t('ssh.remote.keyPathRequired'));
      return null;
    }
    if (
      !isLocalDocker
      && formData.authType === 'keyboardInteractive'
      && !formData.password
      && !formData.verificationCode
    ) {
      setLocalError(t('ssh.remote.challengeResponseRequired'));
      return null;
    }
    if (usesContainer && requireContainer && !formData.containerName.trim()) {
      setLocalError(t('ssh.remote.containerRequired'));
      return null;
    }
    const connectTimeoutSecs = Number(formData.connectTimeoutSecs);
    const authTimeoutSecs = Number(formData.authTimeoutSecs);
    const authAttempts = Number(formData.authAttempts);
    const connectAttempts = Number(formData.connectAttempts);
    if (
      !Number.isInteger(connectTimeoutSecs)
      || connectTimeoutSecs < 1
      || !Number.isInteger(authTimeoutSecs)
      || authTimeoutSecs < 1
      || !Number.isInteger(authAttempts)
      || authAttempts < 1
      || authAttempts > 10
      || !Number.isInteger(connectAttempts)
      || connectAttempts < 1
      || connectAttempts > 5
    ) {
      setShowAdvancedSettings(true);
      setLocalError(t('ssh.remote.connectionOptionsInvalid'));
      return null;
    }

    const hostInput = isLocalDocker ? 'local-docker' : formData.host.trim();
    let connectHost = hostInput;
    let resolvedProxyJump = formData.proxyJump.trim();
    if (!isLocalDocker) {
      try {
        const lookup = await sshApi.getSSHConfig(hostInput);
        const resolved = lookup.found && lookup.config?.hostname?.trim();
        if (resolved) {
          connectHost = resolved;
        }
        if (lookup.found && lookup.config?.proxyJump && !resolvedProxyJump) {
          resolvedProxyJump = lookup.config.proxyJump;
        }
      } catch {
        // Use the manual values if ~/.ssh/config cannot be read.
      }
    }

    const username = isLocalDocker
      ? (formData.username.trim() || 'docker')
      : formData.username.trim();
    const containerName = formData.containerName.trim() || 'discovery';
    const id = isLocalDocker
      ? `docker-local-${containerName}`
      : `${generateConnectionId(connectHost, port, username)}${usesContainer ? `-container-${containerName}` : ''}`;
    return {
      id,
      name: formData.name || (isLocalDocker ? containerName : `${username}@${hostInput}`),
      host: connectHost,
      port,
      username,
      auth: isLocalDocker
        ? { type: 'PrivateKey', keyPath: '' }
        : buildAuthMethod(),
      proxyJump: !isLocalDocker ? resolvedProxyJump || undefined : undefined,
      container: usesContainer ? {
        name: containerName,
        access: formData.targetType === 'containerSshd'
          ? 'sshd'
          : formData.containerAccess,
        local: isLocalDocker,
        dockerPath: formData.dockerPath.trim() || 'docker',
        shell: formData.containerShell.trim() || '/bin/sh',
        user: formData.containerUser.trim() || undefined,
        interactive: true,
      } : undefined,
      options: {
        connectTimeoutSecs,
        authTimeoutSecs,
        authAttempts,
        connectAttempts,
      },
    };
  };

  const handleConnect = async () => {
    const config = await buildConnectionConfig(true);
    if (!config) return;

    setIsConnecting(true);
    setLocalError(null);
    try {
      await connect(config.id, config, { browseAfterConnect: true });
      // Don't call onClose() here - connect() handles closing the dialog via context
    } catch (e) {
      setLocalError(e instanceof Error ? e.message : 'Connection failed');
    } finally {
      setIsConnecting(false);
    }
  };

  const handleTestConnection = async () => {
    const config = await buildConnectionConfig(true);
    if (!config) return;

    setIsTesting(true);
    setConnectionTest(null);
    setLocalError(null);
    try {
      setConnectionTest(await sshApi.testConnection(config));
    } catch (error) {
      setLocalError(error instanceof Error ? error.message : t('ssh.remote.testFailed'));
    } finally {
      setIsTesting(false);
    }
  };

  const handleListContainers = async () => {
    const config = await buildConnectionConfig(false);
    if (!config) return;

    setIsListingContainers(true);
    setLocalError(null);
    try {
      const containers = await sshApi.listDockerContainers(config);
      setDockerContainers(containers);
      if (containers.length === 0) {
        setLocalError(t('ssh.remote.noContainersFound'));
      }
    } catch (error) {
      setLocalError(error instanceof Error ? error.message : t('ssh.remote.containerListFailed'));
    } finally {
      setIsListingContainers(false);
    }
  };

  const handleQuickConnect = async (conn: SavedConnection) => {
    setLocalError(null);

    if (conn.authType.type === 'Password') {
      setIsConnecting(true);
      setLocalError(null);
      try {
        await connect(
          conn.id,
          {
            id: conn.id,
            name: conn.name,
            host: conn.host,
            port: conn.port,
            username: conn.username,
            auth: { type: 'Password', password: '' },
            defaultWorkspace: conn.defaultWorkspace,
            proxyJump: conn.proxyJump,
            container: conn.container,
            options: conn.options,
          },
          { browseAfterConnect: true }
        );
      } catch {
        setCredentialsPrompt(conn);
      } finally {
        setIsConnecting(false);
      }
    } else if (conn.authType.type === 'PrivateKey' || conn.authType.type === 'Agent') {
      const auth: SSHAuthMethod = conn.authType.type === 'PrivateKey'
        ? {
            type: 'PrivateKey',
            keyPath: conn.authType.keyPath,
            certificatePath: conn.authType.certificatePath,
          }
        : {
            type: 'Agent',
            keyFingerprint: conn.authType.keyFingerprint,
            fallbackKeyPath: conn.authType.fallbackKeyPath,
          };
      setIsConnecting(true);
      try {
        await connect(
          conn.id,
          {
            id: conn.id,
            name: conn.name,
            host: conn.host,
            port: conn.port,
            username: conn.username,
            auth,
            defaultWorkspace: conn.defaultWorkspace,
            proxyJump: conn.proxyJump,
            container: conn.container,
            options: conn.options,
          },
          { browseAfterConnect: true }
        );
      } catch {
        setCredentialsPrompt(conn);
      } finally {
        setIsConnecting(false);
      }
    } else {
      setCredentialsPrompt(conn);
    }
  };

  // Fill the manual connection form from an ~/.ssh/config host entry
  const handleFillFromConfig = (configHost: SSHConfigEntry) => {
    const port = configHost.port ? String(configHost.port) : '22';
    const username = configHost.user || '';
    const keyPath = configHost.identityFile?.trim() || '~/.ssh/id_rsa';
    const hasKey = !!configHost.identityFile?.trim();

    setFormData({
      name: configHost.host,
      host: configHost.host,
      port,
      username,
      authType: hasKey ? 'privateKey' : configHost.agent ? 'agent' : 'password',
      password: '',
      keyPath,
      passphrase: '',
      certificatePath: configHost.certificateFile || '',
      keyFingerprint: '',
      fallbackKeyPath: '~/.ssh/id_rsa',
      verificationCode: '',
      proxyJump: configHost.proxyJump || '',
      targetType: 'ssh',
      containerName: '',
      containerAccess: 'auto',
      dockerPath: 'docker',
      containerShell: '/bin/sh',
      containerUser: '',
      connectTimeoutSecs: '30',
      authTimeoutSecs: '60',
      authAttempts: '3',
      connectAttempts: '1',
    });
    setShowAdvancedSettings(Boolean(configHost.proxyJump));
    // Config list sits above the form; scroll so the filled fields are visible.
    requestAnimationFrame(() => revealConnectionForm());
  };

  const handleCredentialsPromptSubmit = async (payload: SSHAuthPromptSubmitPayload) => {
    if (!credentialsPrompt) return;

    const { auth, username: resolvedUsername } = payload;
    const conn = credentialsPrompt;
    setIsConnecting(true);
    setLocalError(null);
    try {
      const full: SSHConnectionConfig = {
        id: conn.id,
        name: conn.name,
        host: conn.host,
        port: conn.port,
        username: resolvedUsername,
        auth,
        defaultWorkspace: conn.defaultWorkspace,
        proxyJump: conn.proxyJump,
        container: conn.container,
        options: conn.options,
      };
      await connect(conn.id, full, { browseAfterConnect: true });
      setCredentialsPrompt(null);
    } catch (e) {
      setLocalError(e instanceof Error ? e.message : 'Connection failed');
    } finally {
      setIsConnecting(false);
    }
  };

  const handleCredentialsPromptCancel = () => {
    setCredentialsPrompt(null);
    setLocalError(null);
  };

  const handleEditConnection = (e: React.MouseEvent, conn: SavedConnection) => {
    e.stopPropagation();
    const keyPath = conn.authType.type === 'PrivateKey' ? conn.authType.keyPath : '~/.ssh/id_rsa';
    const defaultConnectionName = conn.container?.local
      ? conn.container.name
      : `${conn.username}@${conn.host}`;
    const authType = conn.authType.type === 'Password'
      ? 'password'
      : conn.authType.type === 'PrivateKey'
        ? 'privateKey'
        : conn.authType.type === 'Agent'
          ? 'agent'
          : 'keyboardInteractive';
    const targetType = conn.container?.local
      ? 'localDocker'
      : conn.container?.access === 'docker-exec' || conn.container?.access === 'auto'
        ? 'remoteDocker'
        : conn.container?.access === 'sshd'
          ? 'containerSshd'
          : 'ssh';
    setFormData({
      targetType,
      name: conn.name,
      host: conn.host,
      port: String(conn.port),
      username: conn.username,
      authType,
      password: '',
      keyPath,
      passphrase: '',
      certificatePath: conn.authType.type === 'PrivateKey'
        ? conn.authType.certificatePath || ''
        : '',
      keyFingerprint: conn.authType.type === 'Agent'
        ? conn.authType.keyFingerprint || ''
        : '',
      fallbackKeyPath: conn.authType.type === 'Agent'
        ? conn.authType.fallbackKeyPath || ''
        : '~/.ssh/id_rsa',
      verificationCode: '',
      proxyJump: conn.proxyJump || '',
      containerName: conn.container?.name || '',
      containerAccess: conn.container?.access === 'docker-exec' ? 'docker-exec' : 'auto',
      dockerPath: conn.container?.dockerPath || 'docker',
      containerShell: conn.container?.shell || '/bin/sh',
      containerUser: conn.container?.user || '',
      connectTimeoutSecs: String(conn.options?.connectTimeoutSecs ?? 30),
      authTimeoutSecs: String(conn.options?.authTimeoutSecs ?? 60),
      authAttempts: String(conn.options?.authAttempts ?? 3),
      connectAttempts: String(conn.options?.connectAttempts ?? 1),
    });
    setShowAdvancedSettings(
      Boolean(conn.proxyJump)
      || conn.name !== defaultConnectionName
      || (conn.options?.connectTimeoutSecs ?? 30) !== 30
      || (conn.options?.authTimeoutSecs ?? 60) !== 60
      || (conn.options?.authAttempts ?? 3) !== 3
      || (conn.options?.connectAttempts ?? 1) !== 1
    );
    requestAnimationFrame(() => revealConnectionForm());
  };

  const handleDeleteConnection = async (e: React.MouseEvent, connectionId: string) => {
    e.stopPropagation();
    try {
      await sshApi.deleteConnection(connectionId);
      await loadSavedConnections();
    } catch (err) {
      setLocalError(err instanceof Error ? err.message : 'Failed to delete');
    }
  };

  const authOptions = [
    { label: t('ssh.remote.password') || 'Password', value: 'password', icon: <Lock size={14} /> },
    { label: t('ssh.remote.privateKey') || 'Private Key', value: 'privateKey', icon: <Key size={14} /> },
    { label: t('ssh.remote.sshAgent'), value: 'agent', icon: <Key size={14} /> },
    {
      label: t('ssh.remote.keyboardInteractive'),
      value: 'keyboardInteractive',
      icon: <Lock size={14} />,
    },
  ];
  const containerAccessOptions = [
    { label: t('ssh.remote.containerAccessAuto'), value: 'auto' },
    { label: t('ssh.remote.containerAccessDockerExec'), value: 'docker-exec' },
  ];
  const formatTestStageLabel = (stage: ConnectionTestStage): string => {
    if (stage.id.startsWith('jump-')) {
      return `${t('ssh.remote.testStageJump')} ${stage.id.slice('jump-'.length)} · ${stage.label}`;
    }
    if (stage.id === 'target') {
      return `${t('ssh.remote.testStageTarget')} · ${stage.label}`;
    }
    if (stage.id === 'container') {
      return `${t('ssh.remote.testStageContainer')} · ${stage.label}`;
    }
    if (stage.id === 'docker-host') {
      return t('ssh.remote.testStageLocalDocker');
    }
    return `${t('ssh.remote.testStageConfiguration')} · ${stage.label}`;
  };
  const targetOptions = [
    { label: t('ssh.remote.targetSsh'), value: 'ssh' },
    { label: t('ssh.remote.targetRemoteDocker'), value: 'remoteDocker' },
    { label: t('ssh.remote.targetLocalDocker'), value: 'localDocker' },
    { label: t('ssh.remote.targetContainerSshd'), value: 'containerSshd' },
  ];
  const isLocalDockerTarget = formData.targetType === 'localDocker';
  const usesContainerTarget = formData.targetType !== 'ssh';

  const filteredSavedConnections = savedConnections.filter((conn) => {
    if (!savedSearch.trim()) return true;
    const q = savedSearch.toLowerCase();
    return (
      conn.name.toLowerCase().includes(q) ||
      conn.host.toLowerCase().includes(q) ||
      conn.username.toLowerCase().includes(q)
    );
  });

  const filteredSSHConfigHosts = sshConfigHosts.filter((configHost) => {
    // Hide SSH config hosts that already have a saved connection
    const hostname = configHost.hostname || configHost.host;
    const port = configHost.port || 22;
    const user = configHost.user || '';
    if (savedConnections.some((c) => c.host === hostname && c.port === port && c.username === user)) {
      return false;
    }
    if (!configSearch.trim()) return true;
    const q = configSearch.toLowerCase();
    return (
      configHost.host.toLowerCase().includes(q) ||
      hostname.toLowerCase().includes(q) ||
      (configHost.user || '').toLowerCase().includes(q)
    );
  });

  const dismissError = () => {
    setLocalError(null);
    clearError();
  };

  if (!open) return null;

  return (
    <>
      <Modal
        isOpen={open}
        onClose={onClose}
        title={t('ssh.remote.title') || 'SSH Remote'}
        size="medium"
        showCloseButton
        closeOnOverlayClick={false}
        overlayClassName="ssh-connection-dialog__modal-overlay"
        contentClassName="modal__content--fill-flex"
      >
        <div className="ssh-connection-dialog">
          {error && (
            <div className="ssh-connection-dialog__error-banner">
              <Alert
                type="error"
                message={error}
                closable
                onClose={dismissError}
                className="ssh-connection-dialog__error-alert"
              />
            </div>
          )}

          <div className="ssh-connection-dialog__scroll">
          {/* Saved connections section */}
          {savedConnections.length > 0 && (
            <div className="ssh-connection-dialog__section">
              <div className="ssh-connection-dialog__section-header">
                <h3 className="ssh-connection-dialog__section-title">
                  {t('ssh.remote.savedConnections')}
                </h3>
                <Input
                  className="ssh-connection-dialog__search"
                  value={savedSearch}
                  onChange={(e) => setSavedSearch(e.target.value)}
                  placeholder={t('actions.search')}
                  prefix={<Search size={14} />}
                  size="small"
                />
              </div>
              <div className="ssh-connection-dialog__saved-list">
                {filteredSavedConnections.map((conn) => (
                  <div
                    key={conn.id}
                    className="ssh-connection-dialog__saved-item"
                    onClick={() => !isConnecting && handleQuickConnect(conn)}
                    role="button"
                    tabIndex={0}
                    onKeyDown={(e) => e.key === 'Enter' && !isConnecting && handleQuickConnect(conn)}
                  >
                    <div className="ssh-connection-dialog__saved-icon">
                      <Server size={16} />
                    </div>
                    <div className="ssh-connection-dialog__saved-info">
                      <span className="ssh-connection-dialog__saved-name">{conn.name}</span>
                      <span className="ssh-connection-dialog__saved-detail">
                        {conn.container?.local
                          ? `Docker · ${conn.container.name}`
                          : `${conn.username}@${conn.host}:${conn.port}${conn.container ? ` · ${conn.container.name}` : ''}${conn.proxyJump ? ` · ${t('ssh.remote.via')} ${conn.proxyJump}` : ''}`}
                      </span>
                    </div>
                    <div className="ssh-connection-dialog__saved-actions">
                      <Button
                        size="small"
                        variant="ghost"
                        onClick={(e) => handleEditConnection(e, conn)}
                        disabled={isConnecting}
                        title={t('actions.edit') || 'Edit'}
                      >
                        <Pencil size={13} />
                      </Button>
                      <Button
                        size="small"
                        variant="ghost"
                        onClick={(e) => handleDeleteConnection(e, conn.id)}
                        disabled={isConnecting}
                        className="ssh-connection-dialog__delete-btn"
                        title={t('actions.delete') || 'Delete'}
                      >
                        <Trash2 size={13} />
                      </Button>
                      <Button
                        size="small"
                        variant="primary"
                        onClick={(e) => {
                          e.stopPropagation();
                          handleQuickConnect(conn);
                        }}
                        disabled={isConnecting || status === 'connecting'}
                      >
                        <Play size={12} />
                      </Button>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* SSH Config hosts section */}
          {sshConfigHosts.length > 0 && (
            <div className="ssh-connection-dialog__section">
              <div className="ssh-connection-dialog__section-header">
                <h3 className="ssh-connection-dialog__section-title">
                  {t('ssh.remote.sshConfigHosts') || 'SSH Config'}
                </h3>
                <Input
                  className="ssh-connection-dialog__search"
                  value={configSearch}
                  onChange={(e) => setConfigSearch(e.target.value)}
                  placeholder={t('actions.search')}
                  prefix={<Search size={14} />}
                  size="small"
                />
              </div>
              <div className="ssh-connection-dialog__saved-list">
                {filteredSSHConfigHosts.map((configHost) => (
                  <div
                    key={configHost.host}
                    className="ssh-connection-dialog__saved-item ssh-connection-dialog__saved-item--config"
                    onClick={() => !isConnecting && handleFillFromConfig(configHost)}
                    role="button"
                    tabIndex={0}
                    onKeyDown={(e) => e.key === 'Enter' && !isConnecting && handleFillFromConfig(configHost)}
                  >
                    <div className="ssh-connection-dialog__saved-icon">
                      <Server size={16} />
                    </div>
                    <div className="ssh-connection-dialog__saved-info">
                      <span className="ssh-connection-dialog__saved-name">{configHost.host}</span>
                      <span className="ssh-connection-dialog__saved-detail">
                        {configHost.user || ''}@{configHost.hostname || configHost.host}:{configHost.port || 22}
                      </span>
                    </div>
                    <div className="ssh-connection-dialog__saved-actions">
                      <Button
                        size="small"
                        variant="ghost"
                        onClick={(e) => {
                          e.stopPropagation();
                          handleFillFromConfig(configHost);
                        }}
                        disabled={isConnecting || status === 'connecting'}
                        title={t('ssh.remote.fillForm')}
                      >
                        <ArrowDownToLine size={12} />
                        {t('ssh.remote.fillForm')}
                      </Button>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Divider */}
          {(savedConnections.length > 0 || sshConfigHosts.length > 0) && (
            <div className="ssh-connection-dialog__divider">
              <span>{t('ssh.remote.newConnection')}</span>
            </div>
          )}

          {/* New connection form */}
          <div
            ref={formRef}
            className={[
              'ssh-connection-dialog__form',
              formHighlighted ? 'ssh-connection-dialog__form--highlighted' : '',
            ].filter(Boolean).join(' ')}
          >
            <div className="ssh-connection-dialog__field">
              <label className="ssh-connection-dialog__label">
                {t('ssh.remote.targetType')}
              </label>
              <Select
                options={targetOptions}
                value={formData.targetType}
                onChange={(value) => handleInputChange('targetType', String(value))}
                size="medium"
              />
            </div>

            {!isLocalDockerTarget && (
              <>
            {/* Host and Port */}
            <div className="ssh-connection-dialog__row ssh-connection-dialog__row--host">
              <div className="ssh-connection-dialog__field ssh-connection-dialog__field--flex">
                <Input
                  label={t('ssh.remote.host')}
                  value={formData.host}
                  onChange={(e) => handleInputChange('host', e.target.value)}
                  placeholder=""
                  prefix={<Server size={16} />}
                  size="medium"
                />
              </div>
              <div className="ssh-connection-dialog__field ssh-connection-dialog__field--port">
                <Input
                  label={t('ssh.remote.port')}
                  value={formData.port}
                  onChange={(e) => handleInputChange('port', e.target.value)}
                  placeholder="22"
                  size="medium"
                />
              </div>
            </div>

            {/* Username */}
            <div className="ssh-connection-dialog__field">
              <Input
                label={t('ssh.remote.username')}
                value={formData.username}
                onChange={(e) => handleInputChange('username', e.target.value)}
                placeholder=""
                prefix={<User size={16} />}
                size="medium"
              />
            </div>
              </>
            )}

            {usesContainerTarget && (
              <div className="ssh-connection-dialog__container-fields">
                <div className="ssh-connection-dialog__field">
                  <div className="ssh-connection-dialog__field-header">
                    <label className="ssh-connection-dialog__label">
                      {t('ssh.remote.containerName')}
                    </label>
                    {formData.targetType !== 'containerSshd' && (
                      <Button
                        variant="ghost"
                        size="small"
                        onClick={() => void handleListContainers()}
                        disabled={isListingContainers || isConnecting || status === 'connecting'}
                      >
                        {isListingContainers
                          ? <Loader2 size={13} className="ssh-connection-dialog__spinner" />
                          : <RefreshCw size={13} />}
                        {t('ssh.remote.discoverContainers')}
                      </Button>
                    )}
                  </div>
                  {dockerContainers.length > 0 ? (
                    <Select
                      options={dockerContainers.map((container) => ({
                        label: `${container.name} · ${container.image} · ${container.state}`,
                        value: container.name,
                      }))}
                      value={formData.containerName}
                      onChange={(value) => handleInputChange('containerName', String(value))}
                      size="medium"
                    />
                  ) : (
                    <Input
                      value={formData.containerName}
                      onChange={(e) => handleInputChange('containerName', e.target.value)}
                      placeholder={t('ssh.remote.containerNamePlaceholder')}
                      size="medium"
                    />
                  )}
                </div>
                {formData.targetType !== 'containerSshd' && (
                  <>
                    <div className="ssh-connection-dialog__field">
                      <label className="ssh-connection-dialog__label">
                        {t('ssh.remote.containerAccess')}
                      </label>
                      <Select
                        options={containerAccessOptions}
                        value={formData.containerAccess}
                        onChange={(value) => handleInputChange('containerAccess', String(value))}
                        size="medium"
                      />
                      <div className="ssh-connection-dialog__hint">
                        {t('ssh.remote.containerAccessHint')}
                      </div>
                    </div>
                    <div className="ssh-connection-dialog__field">
                      <Input
                        label={t('ssh.remote.dockerPath')}
                        value={formData.dockerPath}
                        onChange={(e) => handleInputChange('dockerPath', e.target.value)}
                        placeholder="docker"
                        size="medium"
                      />
                    </div>
                    <div className="ssh-connection-dialog__row">
                      <div className="ssh-connection-dialog__field ssh-connection-dialog__field--flex">
                        <Input
                          label={t('ssh.remote.containerShell')}
                          value={formData.containerShell}
                          onChange={(e) => handleInputChange('containerShell', e.target.value)}
                          placeholder="/bin/sh"
                          size="medium"
                        />
                      </div>
                      <div className="ssh-connection-dialog__field ssh-connection-dialog__field--flex">
                        <Input
                          label={t('ssh.remote.containerUser')}
                          value={formData.containerUser}
                          onChange={(e) => handleInputChange('containerUser', e.target.value)}
                          placeholder={t('ssh.remote.optional')}
                          size="medium"
                        />
                      </div>
                    </div>
                  </>
                )}
                <div className="ssh-connection-dialog__hint">
                  {formData.targetType === 'remoteDocker'
                    ? t('ssh.remote.remoteDockerHint')
                    : formData.targetType === 'localDocker'
                      ? t('ssh.remote.localDockerHint')
                      : t('ssh.remote.containerSshdHint')}
                </div>
              </div>
            )}

            {!isLocalDockerTarget && (
              <>
            {/* Authentication Method */}
            <div className="ssh-connection-dialog__field">
              <label className="ssh-connection-dialog__label">
                {t('ssh.remote.authMethod')}
              </label>
              <Select
                options={authOptions}
                value={formData.authType}
                onChange={(value) => handleInputChange('authType', String(value))}
                size="medium"
              />
            </div>

            {/* Password */}
            {formData.authType === 'password' && (
              <div className="ssh-connection-dialog__field">
                <Input
                  label={t('ssh.remote.password')}
                  type={showPassword ? 'text' : 'password'}
                  value={formData.password}
                  onChange={(e) => handleInputChange('password', e.target.value)}
                  placeholder=""
                  prefix={<Lock size={16} />}
                  size="medium"
                  suffix={
                    <button type="button" className="bitfun-input-toggle" onClick={() => setShowPassword(s => !s)} tabIndex={-1}>
                      {showPassword ? <EyeOff size={16} /> : <Eye size={16} />}
                    </button>
                  }
                />
              </div>
            )}

            {/* Private Key */}
            {formData.authType === 'privateKey' && (
              <>
                <div className="ssh-connection-dialog__field">
                  <Input
                    label={t('ssh.remote.privateKeyPath')}
                    value={formData.keyPath}
                    onChange={(e) => handleInputChange('keyPath', e.target.value)}
                    placeholder="~/.ssh/id_rsa"
                    prefix={<Key size={16} />}
                    suffix={
                      <IconButton
                        type="button"
                        variant="ghost"
                        size="small"
                        className="ssh-connection-dialog__browse-key"
                        tooltip={t('ssh.remote.browsePrivateKey')}
                        aria-label={t('ssh.remote.browsePrivateKey')}
                        disabled={isConnecting || status === 'connecting'}
                        onClick={() => void handleBrowsePrivateKey()}
                      >
                        <FolderOpen size={16} />
                      </IconButton>
                    }
                    size="medium"
                  />
                </div>
                <div className="ssh-connection-dialog__field">
                  <Input
                    label={t('ssh.remote.passphrase')}
                    type={showPassphrase ? 'text' : 'password'}
                    value={formData.passphrase}
                    onChange={(e) => handleInputChange('passphrase', e.target.value)}
                    placeholder={t('ssh.remote.passphraseOptional')}
                    size="medium"
                    suffix={
                      <button type="button" className="bitfun-input-toggle" onClick={() => setShowPassphrase(s => !s)} tabIndex={-1}>
                        {showPassphrase ? <EyeOff size={16} /> : <Eye size={16} />}
                      </button>
                    }
                  />
                </div>
                <div className="ssh-connection-dialog__field">
                  <Input
                    label={t('ssh.remote.certificatePath')}
                    value={formData.certificatePath}
                    onChange={(e) => handleInputChange('certificatePath', e.target.value)}
                    placeholder={t('ssh.remote.certificatePathOptional')}
                    suffix={
                      <IconButton
                        type="button"
                        variant="ghost"
                        size="small"
                        className="ssh-connection-dialog__browse-key"
                        tooltip={t('ssh.remote.browseCertificate')}
                        aria-label={t('ssh.remote.browseCertificate')}
                        disabled={isConnecting || status === 'connecting'}
                        onClick={() => void handleBrowseCertificate()}
                      >
                        <FolderOpen size={16} />
                      </IconButton>
                    }
                    size="medium"
                  />
                </div>
              </>
            )}

            {formData.authType === 'agent' && (
              <>
                <div className="ssh-connection-dialog__field">
                  <Input
                    label={t('ssh.remote.agentFingerprint')}
                    value={formData.keyFingerprint}
                    onChange={(e) => handleInputChange('keyFingerprint', e.target.value)}
                    placeholder={t('ssh.remote.optional')}
                    size="medium"
                  />
                </div>
                <div className="ssh-connection-dialog__field">
                  <Input
                    label={t('ssh.remote.agentFallbackKey')}
                    value={formData.fallbackKeyPath}
                    onChange={(e) => handleInputChange('fallbackKeyPath', e.target.value)}
                    placeholder={t('ssh.remote.optional')}
                    size="medium"
                  />
                </div>
              </>
            )}

            {formData.authType === 'keyboardInteractive' && (
              <div className="ssh-connection-dialog__row">
                <div className="ssh-connection-dialog__field ssh-connection-dialog__field--flex">
                  <Input
                    label={t('ssh.remote.challengePassword')}
                    type="password"
                    value={formData.password}
                    onChange={(e) => handleInputChange('password', e.target.value)}
                    size="medium"
                  />
                </div>
                <div className="ssh-connection-dialog__field ssh-connection-dialog__field--flex">
                  <Input
                    label={t('ssh.remote.verificationCode')}
                    type="password"
                    value={formData.verificationCode}
                    onChange={(e) => handleInputChange('verificationCode', e.target.value)}
                    placeholder={t('ssh.remote.optional')}
                    size="medium"
                  />
                </div>
              </div>
            )}

              </>
            )}

            <div className="ssh-connection-dialog__advanced">
              <button
                type="button"
                className="ssh-connection-dialog__advanced-toggle"
                aria-expanded={showAdvancedSettings}
                onClick={() => setShowAdvancedSettings((visible) => !visible)}
              >
                <span>{t('ssh.remote.advancedSettings')}</span>
                <ChevronDown
                  size={16}
                  aria-hidden="true"
                  className={[
                    'ssh-connection-dialog__advanced-chevron',
                    showAdvancedSettings
                      ? 'ssh-connection-dialog__advanced-chevron--expanded'
                      : '',
                  ].filter(Boolean).join(' ')}
                />
              </button>

              {showAdvancedSettings && (
                <div className="ssh-connection-dialog__advanced-panel">
                  <div className="ssh-connection-dialog__field">
                    <Input
                      label={t('ssh.remote.connectionName')}
                      value={formData.name}
                      onChange={(e) => handleInputChange('name', e.target.value)}
                      placeholder={t('ssh.remote.connectionNamePlaceholder')}
                      size="medium"
                    />
                  </div>

                  {!isLocalDockerTarget && (
                    <>
                      <div className="ssh-connection-dialog__field">
                        <Input
                          label={t('ssh.remote.proxyJump')}
                          value={formData.proxyJump}
                          onChange={(e) => handleInputChange('proxyJump', e.target.value)}
                          placeholder={t('ssh.remote.proxyJumpPlaceholder')}
                          size="medium"
                        />
                        <div className="ssh-connection-dialog__hint">
                          {t('ssh.remote.proxyJumpHint')}
                        </div>
                      </div>

                      <div className="ssh-connection-dialog__connection-options">
                        <div className="ssh-connection-dialog__label">
                          {t('ssh.remote.connectionOptions')}
                        </div>
                        <div className="ssh-connection-dialog__options-grid">
                          <div className="ssh-connection-dialog__field">
                            <Input
                              label={t('ssh.remote.connectTimeout')}
                              value={formData.connectTimeoutSecs}
                              onChange={(e) => handleInputChange('connectTimeoutSecs', e.target.value)}
                              size="medium"
                            />
                          </div>
                          <div className="ssh-connection-dialog__field">
                            <Input
                              label={t('ssh.remote.authTimeout')}
                              value={formData.authTimeoutSecs}
                              onChange={(e) => handleInputChange('authTimeoutSecs', e.target.value)}
                              size="medium"
                            />
                          </div>
                          <div className="ssh-connection-dialog__field">
                            <Input
                              label={t('ssh.remote.authAttempts')}
                              value={formData.authAttempts}
                              onChange={(e) => handleInputChange('authAttempts', e.target.value)}
                              size="medium"
                            />
                          </div>
                          <div className="ssh-connection-dialog__field">
                            <Input
                              label={t('ssh.remote.connectAttempts')}
                              value={formData.connectAttempts}
                              onChange={(e) => handleInputChange('connectAttempts', e.target.value)}
                              size="medium"
                            />
                          </div>
                        </div>
                      </div>
                    </>
                  )}
                </div>
              )}
            </div>

            {connectionTest && (
              <div
                className={[
                  'ssh-connection-dialog__test-report',
                  connectionTest.success
                    ? 'ssh-connection-dialog__test-report--success'
                    : 'ssh-connection-dialog__test-report--error',
                ].join(' ')}
              >
                {connectionTest.stages.map((stage) => (
                  <div key={stage.id} className="ssh-connection-dialog__test-stage">
                    {stage.success
                      ? <CheckCircle2 size={14} />
                      : <XCircle size={14} />}
                    <span>{formatTestStageLabel(stage)}</span>
                    {stage.error && <span title={stage.error}>{stage.error}</span>}
                  </div>
                ))}
                {connectionTest.resolvedContainerAccess && (
                  <div className="ssh-connection-dialog__hint">
                    {t('ssh.remote.resolvedContainerAccess')}:{' '}
                    {connectionTest.resolvedContainerAccess === 'sshd'
                      ? t('ssh.remote.containerAccessSshd')
                      : connectionTest.resolvedContainerAccess === 'docker-exec'
                        ? t('ssh.remote.containerAccessDockerExec')
                        : t('ssh.remote.containerAccessAuto')}
                  </div>
                )}
              </div>
            )}
          </div>
          </div>

          {/* Actions */}
          <div className="ssh-connection-dialog__actions">
            <Button
              variant="secondary"
              size="small"
              onClick={() => void handleTestConnection()}
              disabled={isTesting || isConnecting || status === 'connecting'}
            >
              {isTesting && <Loader2 size={14} className="ssh-connection-dialog__spinner" />}
              {t('ssh.remote.testConnection')}
            </Button>
            <Button
              variant="secondary"
              size="small"
              onClick={onClose}
              disabled={isConnecting || status === 'connecting'}
            >
              {t('actions.cancel')}
            </Button>
            <Button
              variant="primary"
              size="small"
              onClick={handleConnect}
              disabled={
                isConnecting
                || status === 'connecting'
                || (isLocalDockerTarget
                  ? !formData.containerName.trim()
                  : !formData.host.trim() || !formData.username.trim())
              }
            >
              {(isConnecting || status === 'connecting') ? (
                <>
                  <Loader2 size={14} className="ssh-connection-dialog__spinner" />
                  {t('ssh.remote.connecting')}
                </>
              ) : (
                <>
                  <Plus size={14} />
                  {t('ssh.remote.connect')}
                </>
              )}
            </Button>
          </div>
        </div>
      </Modal>

      {credentialsPrompt && (
        <SSHAuthPromptDialog
          open
          targetDescription={`${credentialsPrompt.username}@${credentialsPrompt.host}:${credentialsPrompt.port}`}
          defaultAuthMethod={
            credentialsPrompt.authType.type === 'PrivateKey'
              ? 'privateKey'
              : credentialsPrompt.authType.type === 'Agent'
                ? 'agent'
                : credentialsPrompt.authType.type === 'KeyboardInteractive'
                  ? 'keyboardInteractive'
                  : 'password'
          }
          defaultKeyPath={
            credentialsPrompt.authType.type === 'PrivateKey'
              ? credentialsPrompt.authType.keyPath
              : '~/.ssh/id_rsa'
          }
          defaultCertificatePath={
            credentialsPrompt.authType.type === 'PrivateKey'
              ? credentialsPrompt.authType.certificatePath
              : undefined
          }
          initialUsername={credentialsPrompt.username}
          lockUsername
          onSubmit={handleCredentialsPromptSubmit}
          onCancel={handleCredentialsPromptCancel}
          isConnecting={isConnecting}
        />
      )}
    </>
  );
};

export default SSHConnectionDialog;
