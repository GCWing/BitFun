/**
 * SSH Connection Dialog Component
 * Professional SSH connection dialog following BitFun design patterns
 */

import React, { useState, useEffect, useCallback } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { useSSHRemoteContext } from './SSHRemoteContext';
import { SSHAuthPromptDialog, type SSHAuthPromptSubmitPayload } from './SSHAuthPromptDialog';
import { Modal } from '@/component-library';
import { Button } from '@/component-library';
import { Input } from '@/component-library';
import { Select } from '@/component-library';
import { Alert } from '@/component-library';
import { IconButton } from '@/component-library';
import { FolderOpen, Loader2, Server, User, Key, Lock, Trash2, Plus, Pencil, Play } from 'lucide-react';
import type {
  SSHConnectionConfig,
  SSHAuthMethod,
  SavedConnection,
  SSHConfigEntry,
} from './types';
import { sshApi } from './sshApi';
import { pickSshPrivateKeyPath } from './pickSshPrivateKeyPath';
import './SSHConnectionDialog.scss';

type CredentialsPromptState =
  | { kind: 'saved'; connection: SavedConnection }
  | {
      kind: 'sshConfig';
      entry: SSHConfigEntry;
      connectHost: string;
      port: number;
    };

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
  const [credentialsPrompt, setCredentialsPrompt] = useState<CredentialsPromptState | null>(null);

  const error = localError || connectionError;

  // Form state
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
    } catch (_error) {
      setSSHConfigHosts([]);
    }
  }

  // Clear errors when dialog opens
  useEffect(() => {
    if (open) {
      clearError();
      setLocalError(null);
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
          // Auto-fill fields from SSH config if they're not already set
          setFormData((prev) => ({
            ...prev,
            port: config.port ? String(config.port) : prev.port,
            username: config.user || prev.username,
            keyPath: config.identityFile || prev.keyPath,
            // If identity file is set, default to privateKey auth
            authType: config.identityFile ? 'privateKey' : prev.authType,
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
  };

  const handleBrowsePrivateKey = useCallback(async () => {
    if (isConnecting || status === 'connecting') return;
    const path = await pickSshPrivateKeyPath({
      title: t('ssh.remote.pickPrivateKeyDialogTitle'),
    });
    if (path) setFormData((prev) => ({ ...prev, keyPath: path }));
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
        };
    }
  };

  const handleConnect = async () => {
    // Validation
    if (!formData.host.trim()) {
      setLocalError(t('ssh.remote.hostRequired'));
      return;
    }
    if (!formData.username.trim()) {
      setLocalError(t('ssh.remote.usernameRequired'));
      return;
    }
    const port = parseInt(formData.port, 10);
    if (isNaN(port) || port < 1 || port > 65535) {
      setLocalError(t('ssh.remote.portInvalid'));
      return;
    }
    if (formData.authType === 'password' && !formData.password) {
      setLocalError(t('ssh.remote.passwordRequired'));
      return;
    }
    if (formData.authType === 'privateKey' && !formData.keyPath.trim()) {
      setLocalError(t('ssh.remote.keyPathRequired'));
      return;
    }

    const hostInput = formData.host.trim();
    let connectHost = hostInput;
    try {
      const lookup = await sshApi.getSSHConfig(hostInput);
      const resolved = lookup.found && lookup.config?.hostname?.trim();
      if (resolved) {
        connectHost = resolved;
      }
    } catch {
      // Use hostInput if ~/.ssh/config cannot be read
    }

    const config: SSHConnectionConfig = {
      id: generateConnectionId(connectHost, port, formData.username.trim()),
      name: formData.name || `${formData.username}@${hostInput}`,
      host: connectHost,
      port,
      username: formData.username.trim(),
      auth: buildAuthMethod(),
    };

    setIsConnecting(true);
    setLocalError(null);
    try {
      await connect(config.id, config);
      // Don't call onClose() here - connect() handles closing the dialog via context
    } catch (e) {
      setLocalError(e instanceof Error ? e.message : 'Connection failed');
    } finally {
      setIsConnecting(false);
    }
  };

  const handleQuickConnect = async (conn: SavedConnection) => {
    setLocalError(null);

    if (conn.authType.type === 'Password') {
      setIsConnecting(true);
      setLocalError(null);
      try {
        await connect(conn.id, {
          id: conn.id,
          name: conn.name,
          host: conn.host,
          port: conn.port,
          username: conn.username,
          auth: { type: 'Password', password: '' },
        });
      } catch {
        setCredentialsPrompt({ kind: 'saved', connection: conn });
      } finally {
        setIsConnecting(false);
      }
    } else if (conn.authType.type === 'PrivateKey') {
      const auth: SSHAuthMethod = {
        type: 'PrivateKey',
        keyPath: conn.authType.keyPath,
      };

      setIsConnecting(true);
      try {
        await connect(conn.id, {
          id: conn.id,
          name: conn.name,
          host: conn.host,
          port: conn.port,
          username: conn.username,
          auth,
        });
      } catch (e) {
        setLocalError(e instanceof Error ? e.message : 'Connection failed');
      } finally {
        setIsConnecting(false);
      }
    }
  };

  // Handle connecting to a host from ~/.ssh/config
  const handleSSHConfigConnect = async (configHost: SSHConfigEntry) => {
    setLocalError(null);

    // Build hostname - if hostname is set in config, use it; otherwise use the host alias
    const hostname = configHost.hostname || configHost.host;
    const username = configHost.user || '';
    const port = configHost.port || 22;

    const hasValidIdentityFile = configHost.identityFile && configHost.identityFile.trim() !== '';

    if (!hasValidIdentityFile) {
      setCredentialsPrompt({
        kind: 'sshConfig',
        entry: configHost,
        connectHost: hostname,
        port,
      });
      return;
    }

    const connectionId = generateConnectionId(hostname, port, username);
    const name = configHost.host;

    const authConfig: SSHConnectionConfig = {
      id: connectionId,
      name,
      host: hostname,
      port,
      username,
      auth: { type: 'PrivateKey', keyPath: configHost.identityFile! },
    };

    setIsConnecting(true);
    try {
      await connect(connectionId, authConfig);
    } catch (e) {
      setLocalError(e instanceof Error ? e.message : 'Authentication failed');
    } finally {
      setIsConnecting(false);
    }
  };

  const credentialsTargetDescription = (state: CredentialsPromptState): string => {
    if (state.kind === 'saved') {
      const c = state.connection;
      return `${c.username}@${c.host}:${c.port}`;
    }
    const { entry, connectHost, port } = state;
    const u = entry.user?.trim();
    const base = u ? `${u}@${connectHost}:${port}` : `${connectHost}:${port}`;
    if (entry.host && entry.host !== connectHost) {
      return `${base} (${entry.host})`;
    }
    return base;
  };

  const handleCredentialsPromptSubmit = async (payload: SSHAuthPromptSubmitPayload) => {
    if (!credentialsPrompt) return;

    const { auth, username: resolvedUsername } = payload;
    setIsConnecting(true);
    setLocalError(null);
    try {
      if (credentialsPrompt.kind === 'saved') {
        const conn = credentialsPrompt.connection;
        const full: SSHConnectionConfig = {
          id: conn.id,
          name: conn.name,
          host: conn.host,
          port: conn.port,
          username: resolvedUsername,
          auth,
        };
        await connect(conn.id, full);
      } else {
        const { entry, connectHost, port } = credentialsPrompt;
        const connectionId = generateConnectionId(connectHost, port, resolvedUsername);
        const full: SSHConnectionConfig = {
          id: connectionId,
          name: entry.host,
          host: connectHost,
          port,
          username: resolvedUsername,
          auth,
        };
        await connect(connectionId, full);
        await loadSavedConnections();
      }
      setCredentialsPrompt(null);
      onClose();
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
    setFormData({
      name: conn.name,
      host: conn.host,
      port: String(conn.port),
      username: conn.username,
      authType: conn.authType.type === 'Password' ? 'password' : 'privateKey',
      password: '',
      keyPath,
      passphrase: '',
    });
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
  ];

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
              <h3 className="ssh-connection-dialog__section-title">
                {t('ssh.remote.savedConnections')}
              </h3>
              <div className="ssh-connection-dialog__saved-list">
                {savedConnections.map((conn) => (
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
                        {conn.username}@{conn.host}:{conn.port}
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
              <h3 className="ssh-connection-dialog__section-title">
                {t('ssh.remote.sshConfigHosts') || 'SSH Config'}
              </h3>
              <div className="ssh-connection-dialog__saved-list">
                {sshConfigHosts
                  .filter((configHost) => {
                    // Hide SSH config hosts that already have a saved connection
                    const hostname = configHost.hostname || configHost.host;
                    const port = configHost.port || 22;
                    const user = configHost.user || '';
                    return !savedConnections.some(
                      (c) => c.host === hostname && c.port === port && c.username === user
                    );
                  })
                  .map((configHost) => (
                  <div
                    key={configHost.host}
                    className="ssh-connection-dialog__saved-item ssh-connection-dialog__saved-item--config"
                    onClick={() => !isConnecting && handleSSHConfigConnect(configHost)}
                    role="button"
                    tabIndex={0}
                    onKeyDown={(e) => e.key === 'Enter' && !isConnecting && handleSSHConfigConnect(configHost)}
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
                        variant="primary"
                        onClick={(e) => {
                          e.stopPropagation();
                          void handleSSHConfigConnect(configHost);
                        }}
                        disabled={isConnecting || status === 'connecting'}
                        title={t('ssh.remote.connect')}
                      >
                        <Play size={12} />
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
          <div className="ssh-connection-dialog__form">
            {/* Host and Port */}
            <div className="ssh-connection-dialog__row">
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

            {/* Connection Name */}
            <div className="ssh-connection-dialog__field">
              <Input
                label={t('ssh.remote.connectionName')}
                value={formData.name}
                onChange={(e) => handleInputChange('name', e.target.value)}
                placeholder={t('ssh.remote.connectionNamePlaceholder')}
                size="medium"
              />
            </div>

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
                  type="password"
                  value={formData.password}
                  onChange={(e) => handleInputChange('password', e.target.value)}
                  placeholder=""
                  prefix={<Lock size={16} />}
                  size="medium"
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
                    type="password"
                    value={formData.passphrase}
                    onChange={(e) => handleInputChange('passphrase', e.target.value)}
                    placeholder={t('ssh.remote.passphraseOptional')}
                    size="medium"
                  />
                </div>
              </>
            )}
          </div>
          </div>

          {/* Actions */}
          <div className="ssh-connection-dialog__actions">
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
              disabled={isConnecting || status === 'connecting' || !formData.host.trim() || !formData.username.trim()}
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
          targetDescription={credentialsTargetDescription(credentialsPrompt)}
          defaultAuthMethod="password"
          defaultKeyPath={
            credentialsPrompt.kind === 'sshConfig' && credentialsPrompt.entry.identityFile?.trim()
              ? credentialsPrompt.entry.identityFile.trim()
              : '~/.ssh/id_rsa'
          }
          initialUsername={
            credentialsPrompt.kind === 'saved'
              ? credentialsPrompt.connection.username
              : credentialsPrompt.entry.user || ''
          }
          lockUsername={
            credentialsPrompt.kind === 'saved'
              ? true
              : !!(credentialsPrompt.entry.user && credentialsPrompt.entry.user.trim())
          }
          onSubmit={handleCredentialsPromptSubmit}
          onCancel={handleCredentialsPromptCancel}
          isConnecting={isConnecting}
        />
      )}
    </>
  );
};

export default SSHConnectionDialog;
