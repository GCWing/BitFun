/**
 * Account Login + Device Control Dialog
 *
 * Three views:
 * 1. Login form (username/password/server)
 * 2. Cloud overwrite confirmation (non-first login)
 * 3. Device control panel (list devices → browse workspaces/sessions → send messages)
 */

import React, { useState, useEffect, useCallback } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { Modal, Button, Input, Alert } from '@/component-library';
import {
  User, Lock, Server, LogIn, Monitor, CloudDownload,
  ChevronRight, ArrowLeft, Send, Plus, MessageSquare,
} from 'lucide-react';
import { remoteConnectAPI } from '@/infrastructure/api/service-api/RemoteConnectAPI';
import type {
  AccountHint, AccountDeviceInfo,
} from '@/infrastructure/api/service-api/RemoteConnectAPI';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import './AccountLoginDialog.scss';

const log = createLogger('AccountLoginDialog');

interface AccountLoginDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

// Remote workspace info (from RemoteResponse::WorkspaceInfo)
interface RemoteWorkspaceInfo {
  has_workspace: boolean;
  path: string | null;
  project_name: string | null;
}

// Remote session info (from RemoteResponse::SessionList)
interface RemoteSessionInfo {
  session_id: string;
  name: string;
  agent_type: string;
  updated_at: string;
  message_count: number;
}

type View = 'login' | 'overwrite' | 'devices' | 'sessions' | 'chat';

export const AccountLoginDialog: React.FC<AccountLoginDialogProps> = ({
  isOpen,
  onClose,
}) => {
  const { t } = useI18n('common');
  const { success } = useNotification();
  const { workspacePath } = useCurrentWorkspace();

  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [authServer, setAuthServer] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [view, setView] = useState<View>('login');

  // Device control panel state
  const [devices, setDevices] = useState<AccountDeviceInfo[]>([]);
  const [selectedDevice, setSelectedDevice] = useState<AccountDeviceInfo | null>(null);
  const [remoteWorkspace, setRemoteWorkspace] = useState<RemoteWorkspaceInfo | null>(null);
  const [remoteSessions, setRemoteSessions] = useState<RemoteSessionInfo[]>([]);
  const [remoteMessages, setRemoteMessages] = useState<any[]>([]);
  const [selectedSession, setSelectedSession] = useState<string | null>(null);
  const [messageInput, setMessageInput] = useState('');

  const refreshDevices = useCallback(async () => {
    try {
      const list = await remoteConnectAPI.accountListDevices();
      setDevices(list);
    } catch (e) { log.warn('refreshDevices failed', e); }
  }, []);

  useEffect(() => {
    if (!isOpen) {
      setUsername(''); setPassword(''); setAuthServer('');
      setError(null); setLoading(false); setView('login');
      setDevices([]); setSelectedDevice(null); setRemoteSessions([]);
      setRemoteMessages([]); setSelectedSession(null); setMessageInput('');
    } else {
      remoteConnectAPI.accountGetCredentialHint().then((hint: AccountHint | null) => {
        if (hint) { setUsername(hint.username); setAuthServer(hint.relay_url); }
      });
      remoteConnectAPI.accountStatus().then((status) => {
        if (status.logged_in) { setView('devices'); refreshDevices(); }
      });
    }
  }, [isOpen, refreshDevices]);

  const validate = useCallback(() => {
    if (!username.trim() || !password.trim() || !authServer.trim()) {
      setError(t('accountLogin.emptyFields'));
      return false;
    }
    setError(null);
    return true;
  }, [username, password, authServer, t]);

  const doAutoSync = useCallback(async (isFirstLogin: boolean) => {
    let configJson = '{}';
    if (isFirstLogin) {
      try {
        const exported = await configAPI.exportConfig();
        configJson = JSON.stringify(exported);
      } catch (e) { log.warn('export config failed', e); }
    }
    const wp = workspacePath || '/';
    const result = await remoteConnectAPI.accountAutoSync(isFirstLogin, wp, configJson);
    log.info(`Auto-sync done: settings=${result.settings_synced} exported=${result.sessions_exported} imported=${result.sessions_imported}`);
  }, [workspacePath]);

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
      await doAutoSync(true);
      remoteConnectAPI.accountConnectDevices().catch((err) => {
        log.warn('accountConnectDevices failed', err);
      });
      success(t('accountLogin.loginSuccess', { user_id: result.user_id }));
      setView('devices');
      refreshDevices();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally { setLoading(false); }
  }, [validate, authServer, username, password, doAutoSync, success, t, refreshDevices]);

  const handleConfirmOverwrite = useCallback(async () => {
    setLoading(true); setError(null);
    try {
      await doAutoSync(false);
      remoteConnectAPI.accountConnectDevices().catch((err) => {
        log.warn('accountConnectDevices failed', err);
      });
      success(t('accountLogin.loginSuccess', { user_id: username }));
      setView('devices');
      refreshDevices();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally { setLoading(false); }
  }, [doAutoSync, success, t, username, refreshDevices]);

  const handleCancelOverwrite = useCallback(async () => {
    try { await remoteConnectAPI.accountLogout(); } catch (e) { log.warn('logout failed', e); }
    setView('login');
    onClose();
  }, [onClose]);

  // ── Device control: browse remote workspace + sessions ──────────────
  const selectDevice = useCallback(async (device: AccountDeviceInfo) => {
    setSelectedDevice(device);
    setLoading(true); setError(null);
    try {
      // Get workspace info
      const wsResp = await remoteConnectAPI.accountDeviceRpc(
        device.device_id,
        JSON.stringify({ cmd: 'get_workspace_info' }),
      );
      const wsInfo = JSON.parse(wsResp);
      setRemoteWorkspace(wsInfo);

      // List sessions
      const sessResp = await remoteConnectAPI.accountDeviceRpc(
        device.device_id,
        JSON.stringify({ cmd: 'list_sessions', workspace_path: wsInfo.path || '/', limit: 50 }),
      );
      const sessData = JSON.parse(sessResp);
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
      const resp = await remoteConnectAPI.accountDeviceRpc(
        selectedDevice.device_id,
        JSON.stringify({ cmd: 'get_session_messages', session_id: sessionId, limit: 100 }),
      );
      const data = JSON.parse(resp);
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
        JSON.stringify({
          cmd: 'send_message',
          session_id: selectedSession,
          content: messageInput,
          agent_type: null,
          images: null,
          image_contexts: null,
        }),
      );
      setMessageInput('');
      // Refresh messages
      await selectSession(selectedSession);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally { setLoading(false); }
  }, [selectedDevice, selectedSession, messageInput, selectSession]);

  const handleCreateRemoteSession = useCallback(async () => {
    if (!selectedDevice) return;
    setLoading(true); setError(null);
    try {
      const resp = await remoteConnectAPI.accountDeviceRpc(
        selectedDevice.device_id,
        JSON.stringify({
          cmd: 'create_session',
          agent_type: null,
          session_name: null,
          workspace_path: remoteWorkspace?.path || '/',
        }),
      );
      const data = JSON.parse(resp);
      if (data.session_id) {
        await selectDevice(selectedDevice);
        await selectSession(data.session_id);
      }
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally { setLoading(false); }
  }, [selectedDevice, remoteWorkspace, selectDevice, selectSession]);

  // ── Render ───────────────────────────────────────────────────────────

  const title = view === 'login' || view === 'overwrite'
    ? t('shared:features.accountLogin')
    : view === 'devices'
    ? t('accountLogin.devices')
    : view === 'sessions'
    ? selectedDevice?.device_name || 'Sessions'
    : 'Chat';

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={title}
      size="medium"
      showCloseButton
      closeOnOverlayClick={false}
      contentClassName="modal__content--fill-flex"
    >
      <div className="account-login-dialog">
        {error && (
          <div className="account-login-dialog__error-banner">
            <Alert type="error" message={error} closable onClose={() => setError(null)}
              className="account-login-dialog__error-alert" />
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
                <Input label={t('accountLogin.password')} type="password" value={password}
                  onChange={(e) => setPassword(e.target.value)} prefix={<Lock size={16} />}
                  size="medium" disabled={loading} />
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
              <p className="account-login-dialog__overwrite-detail">
                {t('accountLogin.cloudOverwriteDetail')}
              </p>
            </div>
            <div className="account-login-dialog__actions">
              <Button variant="secondary" size="small" onClick={handleCancelOverwrite} disabled={loading}>
                {t('accountLogin.disagree')}
              </Button>
              <Button variant="primary" size="small" onClick={handleConfirmOverwrite} disabled={loading}>
                {loading ? t('accountLogin.processing') : t('accountLogin.agree')}
              </Button>
            </div>
          </div>
        )}

        {/* ── Device list ───────────────────────────────────────────── */}
        {view === 'devices' && (
          <div className="account-login-dialog__scroll">
            <div className="account-login-dialog__device-list">
              {devices.length === 0 && (
                <div className="account-login-dialog__empty">{t('accountLogin.noDevices')}</div>
              )}
              {devices.map((d) => (
                <div key={d.device_id} className="account-login-dialog__device-card"
                  onClick={() => selectDevice(d)}>
                  <Monitor size={16} />
                  <div className="account-login-dialog__device-info">
                    <span className="account-login-dialog__device-name">{d.device_name}</span>
                    <span className="account-login-dialog__device-id">{d.device_id.slice(0, 8)}</span>
                  </div>
                  <ChevronRight size={14} />
                </div>
              ))}
            </div>
            <div className="account-login-dialog__actions">
              <Button variant="secondary" size="small" onClick={() => { remoteConnectAPI.accountLogout(); setView('login'); }}>
                {t('accountLogin.logout') || 'Logout'}
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
              <Button variant="ghost" size="small" onClick={handleCreateRemoteSession} disabled={loading}>
                <Plus size={14} /> {t('accountLogin.newSession') || 'New Session'}
              </Button>
            </div>
            {remoteWorkspace && (
              <div className="account-login-dialog__workspace-info">
                {remoteWorkspace.project_name || remoteWorkspace.path || '/'} ({remoteSessions.length} sessions)
              </div>
            )}
            <div className="account-login-dialog__session-list">
              {remoteSessions.map((s) => (
                <div key={s.session_id} className="account-login-dialog__session-item"
                  onClick={() => selectSession(s.session_id)}>
                  <MessageSquare size={14} />
                  <div className="account-login-dialog__session-info">
                    <span className="account-login-dialog__session-name">{s.name}</span>
                    <span className="account-login-dialog__session-meta">
                      {s.agent_type} · {s.message_count} msgs
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
                <ArrowLeft size={14} /> Sessions
              </Button>
            </div>
            <div className="account-login-dialog__messages">
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
                placeholder={t('accountLogin.sendMessagePlaceholder') || 'Send message...'}
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
