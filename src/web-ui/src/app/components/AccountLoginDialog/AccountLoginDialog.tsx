/**
 * Account Login + Device Control Dialog
 *
 * Views: login → overwrite (optional) → devices → sessions → chat
 */

import React, { useState, useEffect, useCallback, useRef } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { Modal, Button, Input, Alert } from '@/component-library';
import {
  User, Lock, Server, LogIn, Monitor, CloudDownload, Upload,
  ChevronRight, ArrowLeft, Send, Plus, MessageSquare, RefreshCw,
  Eye, EyeOff, X,
} from 'lucide-react';
import { remoteConnectAPI } from '@/infrastructure/api/service-api/RemoteConnectAPI';
import type { AccountHint, AccountDeviceInfo } from '@/infrastructure/api/service-api/RemoteConnectAPI';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import './AccountLoginDialog.scss';

const log = createLogger('AccountLoginDialog');

interface AccountLoginDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

interface RemoteWorkspaceInfo {
  has_workspace: boolean;
  path: string | null;
  project_name: string | null;
}

interface RemoteSessionInfo {
  session_id: string;
  name: string;
  agent_type: string;
  updated_at: string;
  message_count: number;
}

type View = 'login' | 'overwrite' | 'devices' | 'sessions' | 'chat';

/** Parse an RPC response JSON string and check for remote-side errors. */
function parseRpcResponse(json: string): any {
  const data = JSON.parse(json);
  if (data.resp === 'error') {
    throw new Error(data.message || 'Remote error');
  }
  return data;
}

export const AccountLoginDialog: React.FC<AccountLoginDialogProps> = ({
  isOpen,
  onClose,
}) => {
  const { t } = useI18n('common');
  const { success, info, warning } = useNotification();
  const { workspacePath } = useCurrentWorkspace();

  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [authServer, setAuthServer] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [syncStatus, setSyncStatus] = useState<'idle' | 'syncing' | 'done' | 'failed'>('idle');
  const [showPassword, setShowPassword] = useState(false);
  const [view, setView] = useState<View>('login');

  // Device control panel state
  const [devices, setDevices] = useState<AccountDeviceInfo[]>([]);
  const [selectedDevice, setSelectedDevice] = useState<AccountDeviceInfo | null>(null);
  const [remoteWorkspace, setRemoteWorkspace] = useState<RemoteWorkspaceInfo | null>(null);
  const [remoteWorkspaces, setRemoteWorkspaces] = useState<any[]>([]);
  const [remoteSessions, setRemoteSessions] = useState<RemoteSessionInfo[]>([]);
  const [remoteMessages, setRemoteMessages] = useState<any[]>([]);
  const [selectedSession, setSelectedSession] = useState<string | null>(null);
  const [messageInput, setMessageInput] = useState('');
  const [newWorkspacePath, setNewWorkspacePath] = useState('');
  const [localDeviceId, setLocalDeviceId] = useState<string | null>(null);
  const refreshTimer = useRef<ReturnType<typeof setInterval> | null>(null);

  const refreshDevices = useCallback(async () => {
    try {
      let list = await remoteConnectAPI.accountListDevices();
      // If the local device appears offline, retry once after a short
      // delay — the WS auth round-trip may not have completed yet.
      const localOffline = list.some(d => d.device_id === localDeviceId && !d.online);
      if (localOffline && localDeviceId) {
        await new Promise(r => setTimeout(r, 1500));
        list = await remoteConnectAPI.accountListDevices();
      }
      setDevices(list);
    } catch (e) { log.warn('refreshDevices failed', e); }
  }, [localDeviceId]);

  const resetState = useCallback(() => {
    setDevices([]); setSelectedDevice(null); setRemoteWorkspace(null);
    setRemoteWorkspaces([]); setRemoteSessions([]); setRemoteMessages([]);
    setSelectedSession(null); setMessageInput(''); setNewWorkspacePath('');
    setLocalDeviceId(null);
    if (refreshTimer.current) { clearInterval(refreshTimer.current); refreshTimer.current = null; }
  }, []);

  useEffect(() => {
    if (!isOpen) {
      setUsername(''); setPassword(''); setAuthServer('');
      setError(null); setLoading(false); setView('login');
      resetState();
    } else {
      remoteConnectAPI.getDeviceInfo().then((info) => {
        setLocalDeviceId(info.device_id);
      }).catch((e) => { log.warn('getDeviceInfo failed', e); });
      remoteConnectAPI.accountGetCredentialHint().then((hint: AccountHint | null) => {
        if (hint) { setUsername(hint.username); setAuthServer(hint.relay_url); }
      });
      remoteConnectAPI.accountStatus().then(async (status) => {
        if (status.logged_in && status.user_id) {
          // Ensure device WS connection is up before listing devices
          // so the local device appears as online.
          try {
            await remoteConnectAPI.accountConnectDevices();
          } catch (err) {
            log.warn('accountConnectDevices failed', err);
          }
          setView('devices');
          refreshDevices();
          refreshTimer.current = setInterval(refreshDevices, 10000);
        }
      });
    }
    return () => {
      if (refreshTimer.current) { clearInterval(refreshTimer.current); refreshTimer.current = null; }
    };
  }, [isOpen, refreshDevices, resetState]);

  const validate = useCallback(() => {
    if (!username.trim() || !password.trim() || !authServer.trim()) {
      setError(t('accountLogin.emptyFields'));
      return false;
    }
    setError(null);
    return true;
  }, [username, password, authServer, t]);

  const doAutoSync = useCallback((isFirstLogin: boolean) => {
    const wp = workspacePath || '/';
    setSyncStatus('syncing');
    info(t('accountLogin.syncStarted'));
    (async () => {
      let configJson = '{}';
      if (isFirstLogin) {
        try {
          const exported = await configAPI.exportConfig();
          configJson = JSON.stringify(exported);
        } catch (e) { log.warn('export config failed', e); }
      }
      const result = await remoteConnectAPI.accountAutoSync(isFirstLogin, wp, configJson);
      log.info(`Auto-sync done: settings=${result.settings_synced} exported=${result.sessions_exported} imported=${result.sessions_imported}`);
      setSyncStatus('done');
      success(t('accountLogin.syncDone', {
        exported: result.sessions_exported,
        imported: result.sessions_imported,
      }));
    })().catch((e) => {
      log.error('Auto-sync failed', e);
      setSyncStatus('failed');
      warning(t('accountLogin.syncFailed'));
    });
  }, [workspacePath, info, success, warning, t]);

  const handleLogin = useCallback(async () => {
    if (!validate()) return;
    setLoading(true); setError(null);
    try {
      const result = await remoteConnectAPI.accountLogin(authServer.trim(), username.trim(), password);
      if (result.has_cloud_settings) {
        setView('overwrite');
        setLoading(false);
        return;
      }
      doAutoSync(true);
      // Await device connection so the relay registers us as online
      // before we query the device list (otherwise we appear offline).
      try {
        await remoteConnectAPI.accountConnectDevices();
      } catch (err) {
        log.warn('accountConnectDevices failed', err);
      }
      success(t('accountLogin.loginSuccess', { user_id: result.user_id }));
      setView('devices');
      refreshDevices();
      refreshTimer.current = setInterval(refreshDevices, 10000);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally { setLoading(false); }
  }, [validate, authServer, username, password, doAutoSync, success, t, refreshDevices]);

  const handleConfirmOverwrite = useCallback(async () => {
    setLoading(true); setError(null);
    try {
      doAutoSync(false);
      try {
        await remoteConnectAPI.accountConnectDevices();
      } catch (err) {
        log.warn('accountConnectDevices failed', err);
      }
      success(t('accountLogin.loginSuccess', { user_id: username }));
      setView('devices');
      refreshDevices();
      refreshTimer.current = setInterval(refreshDevices, 10000);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally { setLoading(false); }
  }, [doAutoSync, success, t, username, refreshDevices]);

  // Use local config to overwrite cloud — same as first-login sync (upload).
  const handleUseLocalOverwrite = useCallback(async () => {
    setLoading(true); setError(null);
    try {
      doAutoSync(true);
      try {
        await remoteConnectAPI.accountConnectDevices();
      } catch (err) {
        log.warn('accountConnectDevices failed', err);
      }
      success(t('accountLogin.loginSuccess', { user_id: username }));
      setView('devices');
      refreshDevices();
      refreshTimer.current = setInterval(refreshDevices, 10000);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally { setLoading(false); }
  }, [doAutoSync, success, t, username, refreshDevices]);

  const handleCancelOverwrite = useCallback(async () => {
    try { await remoteConnectAPI.accountLogout(); } catch (e) { log.warn('logout failed', e); }
    resetState();
    setView('login');
    onClose();
  }, [onClose, resetState]);

  const handleLogout = useCallback(async () => {
    setLoading(true);
    try {
      await remoteConnectAPI.accountLogout();
      resetState();
      setView('login');
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally { setLoading(false); }
  }, [resetState]);

  const handleDeleteDevice = useCallback(async (deviceId: string, deviceName: string) => {
    if (!window.confirm(t('accountLogin.confirmRemoveDevice', { name: deviceName }))) return;
    try {
      await remoteConnectAPI.accountDeleteDevice(deviceId);
      success(t('accountLogin.deviceRemoved', { name: deviceName }));
      refreshDevices();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [t, success, refreshDevices]);

  // ── Device control: browse remote workspace + sessions ──────────────
  const selectDevice = useCallback(async (device: AccountDeviceInfo) => {
    if (!device.online) return;
    setSelectedDevice(device);
    setLoading(true); setError(null);
    try {
      const wsInfo = parseRpcResponse(await remoteConnectAPI.accountDeviceRpc(
        device.device_id, JSON.stringify({ cmd: 'get_workspace_info' }),
      ));
      setRemoteWorkspace(wsInfo);

      const wsListData = parseRpcResponse(await remoteConnectAPI.accountDeviceRpc(
        device.device_id, JSON.stringify({ cmd: 'list_recent_workspaces' }),
      ));
      setRemoteWorkspaces(wsListData.workspaces || []);

      const sessData = parseRpcResponse(await remoteConnectAPI.accountDeviceRpc(
        device.device_id,
        JSON.stringify({ cmd: 'list_sessions', workspace_path: wsInfo.path || '/', limit: 50 }),
      ));
      setRemoteSessions(sessData.sessions || []);
      setView('sessions');
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally { setLoading(false); }
  }, []);

  const selectSession = useCallback(async (sessionId: string) => {
    if (!selectedDevice) return;
    setSelectedSession(sessionId);
    setLoading(true); setError(null);
    try {
      const data = parseRpcResponse(await remoteConnectAPI.accountDeviceRpc(
        selectedDevice.device_id,
        JSON.stringify({ cmd: 'get_session_messages', session_id: sessionId, limit: 100 }),
      ));
      setRemoteMessages(data.messages || []);
      setView('chat');
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally { setLoading(false); }
  }, [selectedDevice]);

  const handleSendRemoteMessage = useCallback(async () => {
    if (!selectedDevice || !selectedSession || !messageInput.trim()) return;
    setLoading(true); setError(null);
    try {
      await remoteConnectAPI.accountDeviceRpc(
        selectedDevice.device_id,
        JSON.stringify({ cmd: 'send_message', session_id: selectedSession, content: messageInput, agent_type: null, images: null, image_contexts: null }),
      );
      setMessageInput('');
      await selectSession(selectedSession);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally { setLoading(false); }
  }, [selectedDevice, selectedSession, messageInput, selectSession]);

  const handleCreateRemoteSession = useCallback(async () => {
    if (!selectedDevice) return;
    setLoading(true); setError(null);
    try {
      const data = parseRpcResponse(await remoteConnectAPI.accountDeviceRpc(
        selectedDevice.device_id,
        JSON.stringify({ cmd: 'create_session', agent_type: null, session_name: null, workspace_path: remoteWorkspace?.path || '/' }),
      ));
      if (data.session_id) {
        await selectDevice(selectedDevice);
        await selectSession(data.session_id);
      }
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally { setLoading(false); }
  }, [selectedDevice, remoteWorkspace, selectDevice, selectSession]);

  const switchWorkspace = useCallback(async (wsPath: string) => {
    if (!selectedDevice) return;
    setLoading(true); setError(null);
    try {
      await remoteConnectAPI.accountDeviceRpc(
        selectedDevice.device_id, JSON.stringify({ cmd: 'set_workspace', path: wsPath }),
      );
      const sessData = parseRpcResponse(await remoteConnectAPI.accountDeviceRpc(
        selectedDevice.device_id, JSON.stringify({ cmd: 'list_sessions', workspace_path: wsPath, limit: 50 }),
      ));
      setRemoteSessions(sessData.sessions || []);
      setRemoteWorkspace({ has_workspace: true, path: wsPath, project_name: wsPath.split('/').pop() || wsPath });
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally { setLoading(false); }
  }, [selectedDevice]);

  const handleCreateRemoteWorkspace = useCallback(async () => {
    if (!selectedDevice || !newWorkspacePath.trim()) return;
    setLoading(true); setError(null);
    try {
      parseRpcResponse(await remoteConnectAPI.accountDeviceRpc(
        selectedDevice.device_id, JSON.stringify({ cmd: 'create_workspace', path: newWorkspacePath.trim() }),
      ));
      setNewWorkspacePath('');
      await selectDevice(selectedDevice);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally { setLoading(false); }
  }, [selectedDevice, newWorkspacePath, selectDevice]);

  // ── Render ───────────────────────────────────────────────────────────

  const title = view === 'login' || view === 'overwrite'
    ? t('shared:features.accountLogin')
    : view === 'devices'
    ? t('accountLogin.devices')
    : view === 'sessions'
    ? (selectedDevice?.device_name || t('accountLogin.devices'))
    : t('accountLogin.devices');

  return (
    <Modal isOpen={isOpen} onClose={onClose} title={title} size="medium"
      showCloseButton closeOnOverlayClick={false} contentClassName="modal__content--fill-flex">
      <div className="account-login-dialog">
        {error && (
          <div className="account-login-dialog__error-banner">
            <Alert type="error" message={error} closable onClose={() => setError(null)}
              className="account-login-dialog__error-alert" />
          </div>
        )}

        {/* Loading overlay */}
        {loading && (view === 'devices' || view === 'sessions' || view === 'chat') && (
          <div className="account-login-dialog__loading-overlay">
            <RefreshCw size={20} className="spinning" />
            <span>{t('accountLogin.processing')}</span>
          </div>
        )}

        {/* ── Login form ────────────────────────────────────────────── */}
        {view === 'login' && (
          <div className="account-login-dialog__scroll">
            <div className="account-login-dialog__form">
              <div className="account-login-dialog__field">
                <Input label={t('accountLogin.username')} type="text" value={username}
                  onChange={(e) => setUsername(e.target.value)} prefix={<User size={16} />}
                  size="medium" disabled={loading} />
              </div>
              <div className="account-login-dialog__field">
                <Input label={t('accountLogin.password')} type={showPassword ? 'text' : 'password'} value={password}
                  onChange={(e) => setPassword(e.target.value)} prefix={<Lock size={16} />}
                  size="medium" disabled={loading}
                  suffix={
                    <button type="button" className="bitfun-input-toggle" onClick={() => setShowPassword(s => !s)} tabIndex={-1}>
                      {showPassword ? <EyeOff size={16} /> : <Eye size={16} />}
                    </button>
                  } />
              </div>
              <div className="account-login-dialog__field">
                <Input label={t('accountLogin.authServer')} type="url" value={authServer}
                  onChange={(e) => setAuthServer(e.target.value)}
                  placeholder={t('accountLogin.authServerPlaceholder')}
                  prefix={<Server size={16} />} size="medium" disabled={loading} />
              </div>
            </div>
            <div className="account-login-dialog__actions">
              <Button variant="secondary" size="small" onClick={onClose} disabled={loading}>
                {t('accountLogin.cancel')}
              </Button>
              <Button variant="primary" size="small" onClick={handleLogin} disabled={loading}>
                <LogIn size={14} />
                {loading ? t('accountLogin.processing') : t('accountLogin.login')}
              </Button>
            </div>
          </div>
        )}

        {/* ── Cloud overwrite confirmation ─────────────────────────── */}
        {view === 'overwrite' && (
          <div className="account-login-dialog__scroll">
            <div className="account-login-dialog__overwrite-notice">
              <CloudDownload size={32} />
              <p>{t('accountLogin.cloudOverwriteWarning')}</p>
            </div>
            <div className="account-login-dialog__sync-options">
              <button
                className="account-login-dialog__sync-option"
                onClick={handleUseLocalOverwrite}
                disabled={loading}
              >
                <Upload size={20} />
                <div className="account-login-dialog__sync-option-text">
                  <span className="account-login-dialog__sync-option-title">{t('accountLogin.useLocalTitle')}</span>
                  <span className="account-login-dialog__sync-option-desc">{t('accountLogin.useLocalDesc')}</span>
                </div>
              </button>
              <button
                className="account-login-dialog__sync-option"
                onClick={handleConfirmOverwrite}
                disabled={loading}
              >
                <CloudDownload size={20} />
                <div className="account-login-dialog__sync-option-text">
                  <span className="account-login-dialog__sync-option-title">{t('accountLogin.useCloudTitle')}</span>
                  <span className="account-login-dialog__sync-option-desc">{t('accountLogin.useCloudDesc')}</span>
                </div>
              </button>
            </div>
            <div className="account-login-dialog__actions">
              <Button variant="secondary" size="small" onClick={handleCancelOverwrite} disabled={loading}>
                {t('accountLogin.disagree')}
              </Button>
            </div>
          </div>
        )}

        {/* ── Device list ───────────────────────────────────────────── */}
        {view === 'devices' && (
          <div className="account-login-dialog__scroll">
            {syncStatus !== 'idle' && (
              <div className={`account-login-dialog__sync-indicator ${syncStatus}`}>
                {syncStatus === 'syncing' && <RefreshCw size={14} className="spinning" />}
                {syncStatus === 'done' && <span>✓</span>}
                {syncStatus === 'failed' && <span>⚠</span>}
                <span>
                  {syncStatus === 'syncing' && t('accountLogin.syncing')}
                  {syncStatus === 'done' && t('accountLogin.syncDoneShort')}
                  {syncStatus === 'failed' && t('accountLogin.syncFailed')}
                </span>
              </div>
            )}
            <div className="account-login-dialog__device-list">
              {devices.length === 0 && (
                <div className="account-login-dialog__empty">{t('accountLogin.noDevices')}</div>
              )}
              {devices.map((d) => {
                const isLocal = localDeviceId === d.device_id;
                return (
                <div key={d.device_id}
                  className={`account-login-dialog__device-card ${d.online ? '' : 'offline'} ${isLocal ? 'current' : ''}`}
                  onClick={() => !isLocal && selectDevice(d)}>
                  <Monitor size={16} />
                  <div className="account-login-dialog__device-info">
                    <span className="account-login-dialog__device-name">
                      {d.device_name}
                      {isLocal && <span className="account-login-dialog__device-badge">{t('accountLogin.thisDevice')}</span>}
                    </span>
                    <span className="account-login-dialog__device-id">
                      {d.device_id.slice(0, 8)} · {d.online ? t('accountLogin.online') : t('accountLogin.offline')}
                    </span>
                  </div>
                  {isLocal
                    ? null
                    : <>
                        {d.online && <ChevronRight size={14} />}
                        <button className="account-login-dialog__device-remove"
                          onClick={(e) => { e.stopPropagation(); handleDeleteDevice(d.device_id, d.device_name); }}
                          title={t('accountLogin.removeDevice')}
                          tabIndex={-1}>
                          <X size={14} />
                        </button>
                      </>}
                </div>
                );
              })}
            </div>
            <div className="account-login-dialog__actions">
              <Button variant="secondary" size="small" onClick={handleLogout} disabled={loading}>
                {t('accountLogin.logout')}
              </Button>
            </div>
          </div>
        )}

        {/* ── Session list on selected device ───────────────────────── */}
        {view === 'sessions' && selectedDevice && (
          <div className="account-login-dialog__scroll">
            <div className="account-login-dialog__back-bar">
              <Button variant="ghost" size="small" onClick={() => setView('devices')}>
                <ArrowLeft size={14} /> {t('accountLogin.devices')}
              </Button>
              <div className="account-login-dialog__back-bar-right">
                <Button variant="ghost" size="small" onClick={() => selectDevice(selectedDevice)} disabled={loading}>
                  <RefreshCw size={14} />
                </Button>
                <Button variant="ghost" size="small" onClick={handleCreateRemoteSession} disabled={loading}>
                  <Plus size={14} /> {t('accountLogin.newSession')}
                </Button>
              </div>
            </div>

            {/* Workspace selector */}
            {remoteWorkspaces.length > 0 && (
              <div className="account-login-dialog__workspace-section">
                <div className="account-login-dialog__workspace-label">{t('accountLogin.devices')}</div>
                <div className="account-login-dialog__workspace-chips">
                  {remoteWorkspaces.map((ws: any, i: number) => (
                    <div key={i}
                      className={`account-login-dialog__workspace-chip ${remoteWorkspace?.path === ws.path ? 'active' : ''}`}
                      onClick={() => switchWorkspace(ws.path)}>
                      {ws.name || ws.path}
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* New workspace input */}
            <div className="account-login-dialog__new-workspace">
              <Input type="text" value={newWorkspacePath}
                onChange={(e) => setNewWorkspacePath(e.target.value)}
                placeholder={t('accountLogin.newWorkspacePlaceholder')}
                size="small" disabled={loading} />
              <Button variant="secondary" size="small" onClick={handleCreateRemoteWorkspace}
                disabled={loading || !newWorkspacePath.trim()}>
                <Plus size={14} /> {t('accountLogin.newWorkspace')}
              </Button>
            </div>

            {remoteWorkspace && (
              <div className="account-login-dialog__workspace-info">
                {remoteWorkspace.project_name || remoteWorkspace.path || '/'} ({remoteSessions.length})
              </div>
            )}
            <div className="account-login-dialog__session-list">
              {remoteSessions.length === 0 && (
                <div className="account-login-dialog__empty">{t('accountLogin.noDevices')}</div>
              )}
              {remoteSessions.map((s) => (
                <div key={s.session_id} className="account-login-dialog__session-item"
                  onClick={() => !loading && selectSession(s.session_id)}>
                  <MessageSquare size={14} />
                  <div className="account-login-dialog__session-info">
                    <span className="account-login-dialog__session-name">{s.name}</span>
                    <span className="account-login-dialog__session-meta">
                      {s.agent_type} · {s.message_count}
                    </span>
                  </div>
                  <ChevronRight size={14} />
                </div>
              ))}
            </div>
          </div>
        )}

        {/* ── Chat view (remote session messages) ──────────────────── */}
        {view === 'chat' && selectedDevice && selectedSession && (
          <div className="account-login-dialog__scroll">
            <div className="account-login-dialog__back-bar">
              <Button variant="ghost" size="small" onClick={() => setView('sessions')}>
                <ArrowLeft size={14} /> {t('accountLogin.devices')}
              </Button>
              <Button variant="ghost" size="small" onClick={() => selectSession(selectedSession)} disabled={loading}>
                <RefreshCw size={14} />
              </Button>
            </div>
            <div className="account-login-dialog__messages">
              {remoteMessages.length === 0 && (
                <div className="account-login-dialog__empty">{t('accountLogin.noDevices')}</div>
              )}
              {remoteMessages.map((msg, i) => (
                <div key={i} className={`account-login-dialog__message account-login-dialog__message--${msg.role}`}>
                  <div className="account-login-dialog__message-role">{msg.role}</div>
                  <div className="account-login-dialog__message-content">{msg.content}</div>
                </div>
              ))}
            </div>
            <div className="account-login-dialog__message-input">
              <Input type="text" value={messageInput}
                onChange={(e) => setMessageInput(e.target.value)}
                placeholder={t('accountLogin.sendMessagePlaceholder')}
                onKeyDown={(e) => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); handleSendRemoteMessage(); } }}
                size="medium" disabled={loading} />
              <Button variant="primary" size="small" onClick={handleSendRemoteMessage} disabled={loading || !messageInput.trim()}>
                <Send size={14} />
              </Button>
            </div>
          </div>
        )}
      </div>
    </Modal>
  );
};

export default AccountLoginDialog;
