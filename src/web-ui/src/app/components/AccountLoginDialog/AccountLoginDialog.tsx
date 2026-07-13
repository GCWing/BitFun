/**
 * Account Login + Online Devices
 *
 * Views: login → overwrite (optional) → devices
 * Clicking an online peer device enters Peer Device Mode and closes the dialog.
 */

import React, { useState, useEffect, useCallback, useRef } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { Modal, Button, Input, Alert } from '@/component-library';
import {
  User, Lock, Server, LogIn, Monitor, CloudDownload, Upload,
  ChevronRight, RefreshCw, Eye, EyeOff, X,
} from 'lucide-react';
import { remoteConnectAPI } from '@/infrastructure/api/service-api/RemoteConnectAPI';
import type { AccountHint, AccountDeviceInfo } from '@/infrastructure/api/service-api/RemoteConnectAPI';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import { usePeerDeviceMode } from '@/infrastructure/peer-device/PeerDeviceContext';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import './AccountLoginDialog.scss';

const log = createLogger('AccountLoginDialog');

const DEVICE_POLL_FALLBACK_MS = 30_000;

function isAccountAuthFailure(error: unknown): boolean {
  const msg = (error instanceof Error ? error.message : String(error)).toLowerCase();
  return (
    msg.includes('401')
    || msg.includes('unauthorized')
    || msg.includes('invalid or expired token')
    || msg.includes('relay auth error')
  );
}

interface AccountLoginDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

type View = 'login' | 'overwrite' | 'devices';

export const AccountLoginDialog: React.FC<AccountLoginDialogProps> = ({
  isOpen,
  onClose,
}) => {
  const { t } = useI18n('common');
  const { success, info, warning } = useNotification();
  const { workspacePath } = useCurrentWorkspace();
  const { enterPeerMode } = usePeerDeviceMode();

  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [authServer, setAuthServer] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [syncStatus, setSyncStatus] = useState<'idle' | 'syncing' | 'done' | 'failed'>('idle');
  const [showPassword, setShowPassword] = useState(false);
  const [view, setView] = useState<View>('login');

  const [devices, setDevices] = useState<AccountDeviceInfo[]>([]);
  const [localDeviceId, setLocalDeviceId] = useState<string | null>(null);
  const refreshTimer = useRef<ReturnType<typeof setInterval> | null>(null);

  const resetState = useCallback(() => {
    setDevices([]);
    setLocalDeviceId(null);
    if (refreshTimer.current) { clearInterval(refreshTimer.current); refreshTimer.current = null; }
  }, []);

  const handleSessionExpired = useCallback(async (_error: unknown) => {
    try {
      await remoteConnectAPI.accountLogout();
    } catch (e) {
      log.warn('logout after session expiry failed', e);
    }
    resetState();
    setView('login');
    setError(t('accountLogin.sessionExpired'));
  }, [resetState, t]);

  const refreshDevices = useCallback(async () => {
    try {
      let list = await remoteConnectAPI.accountListDevices();
      const localOffline = list.some(d => d.device_id === localDeviceId && !d.online);
      if (localOffline && localDeviceId) {
        await new Promise(r => setTimeout(r, 1500));
        list = await remoteConnectAPI.accountListDevices();
      }
      setDevices(list);
    } catch (e) {
      log.warn('refreshDevices failed', e);
      if (isAccountAuthFailure(e)) {
        await handleSessionExpired(e);
      }
    }
  }, [localDeviceId, handleSessionExpired]);

  const applyPresenceOnline = useCallback((onlineDevices: Array<{ device_id: string; device_name: string }>) => {
    const onlineIds = new Set(onlineDevices.map(d => d.device_id));
    setDevices(prev => {
      const byId = new Map(prev.map(d => [d.device_id, d]));
      for (const d of onlineDevices) {
        const existing = byId.get(d.device_id);
        if (existing) {
          byId.set(d.device_id, { ...existing, online: true, device_name: d.device_name || existing.device_name });
        } else {
          byId.set(d.device_id, {
            device_id: d.device_id,
            device_name: d.device_name,
            online: true,
            last_seen_at: Date.now(),
          });
        }
      }
      for (const [id, device] of byId) {
        if (!onlineIds.has(id) && device.online) {
          byId.set(id, { ...device, online: false });
        }
      }
      return Array.from(byId.values());
    });
  }, []);

  const startDevicePolling = useCallback(() => {
    if (refreshTimer.current) {
      clearInterval(refreshTimer.current);
    }
    refreshTimer.current = setInterval(refreshDevices, DEVICE_POLL_FALLBACK_MS);
  }, [refreshDevices]);

  useEffect(() => {
    if (!isOpen) {
      setUsername(''); setPassword(''); setAuthServer('');
      setError(null); setLoading(false); setView('login');
      resetState();
      return;
    }

    remoteConnectAPI.getDeviceInfo().then((info) => {
      setLocalDeviceId(info.device_id);
    }).catch((e) => { log.warn('getDeviceInfo failed', e); });
    remoteConnectAPI.accountGetCredentialHint().then((hint: AccountHint | null) => {
      if (hint) { setUsername(hint.username); setAuthServer(hint.relay_url); }
    });
    remoteConnectAPI.accountStatus().then(async (status) => {
      if (status.logged_in && status.user_id) {
        try {
          await remoteConnectAPI.accountConnectDevices();
        } catch (err) {
          log.warn('accountConnectDevices failed', err);
          if (isAccountAuthFailure(err)) {
            await handleSessionExpired(err);
            return;
          }
        }
        setView('devices');
        refreshDevices();
        startDevicePolling();
      }
    });

    const unlistenPresence = api.listen<{ devices: Array<{ device_id: string; device_name: string }> }>(
      'account://device-presence',
      (payload) => {
        if (payload?.devices) {
          applyPresenceOnline(payload.devices);
        }
      },
    );
    const unlistenSettings = api.listen('account://settings-applied', async () => {
      try {
        await configAPI.reloadConfig();
        configManager.clearCache();
      } catch (e) {
        log.warn('Failed to apply settings-applied event', e);
      }
    });

    return () => {
      if (refreshTimer.current) { clearInterval(refreshTimer.current); refreshTimer.current = null; }
      unlistenPresence();
      unlistenSettings();
    };
  }, [isOpen, refreshDevices, resetState, startDevicePolling, applyPresenceOnline, handleSessionExpired]);

  const validate = useCallback(() => {
    if (!username.trim() || !password.trim() || !authServer.trim()) {
      setError(t('accountLogin.emptyFields'));
      return false;
    }
    setError(null);
    return true;
  }, [username, password, authServer, t]);

  const doAutoSync = useCallback(async (isFirstLogin: boolean) => {
    const wp = workspacePath || '/';
    setSyncStatus('syncing');
    info(t('accountLogin.syncStarted'));
    try {
      let configJson = '{}';
      if (isFirstLogin) {
        try {
          const exported = await configAPI.exportConfig();
          configJson = JSON.stringify(exported);
        } catch (e) { log.warn('export config failed', e); }
      }
      const result = await remoteConnectAPI.accountAutoSync(isFirstLogin, wp, configJson);
      log.info(`Auto-sync done: settings=${result.settings_synced} exported=${result.sessions_exported} imported=${result.sessions_imported}`);
      if (result.settings_synced && !isFirstLogin) {
        try {
          await configAPI.reloadConfig();
          configManager.clearCache();
          success(t('accountLogin.settingsApplied'));
        } catch (e) {
          log.warn('reloadConfig after sync failed', e);
        }
      }
      setSyncStatus('done');
      success(t('accountLogin.syncDone', {
        exported: result.sessions_exported,
        imported: result.sessions_imported,
      }));
    } catch (e) {
      log.error('Auto-sync failed', e);
      setSyncStatus('failed');
      warning(t('accountLogin.syncFailed'));
      throw e;
    }
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
      await doAutoSync(true);
      try {
        await remoteConnectAPI.accountConnectDevices();
      } catch (err) {
        log.warn('accountConnectDevices failed', err);
      }
      success(t('accountLogin.loginSuccess', { user_id: result.user_id }));
      setView('devices');
      refreshDevices();
      startDevicePolling();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally { setLoading(false); }
  }, [validate, authServer, username, password, doAutoSync, success, t, refreshDevices, startDevicePolling]);

  const handleConfirmOverwrite = useCallback(async () => {
    setLoading(true); setError(null);
    try {
      await doAutoSync(false);
      try {
        await remoteConnectAPI.accountConnectDevices();
      } catch (err) {
        log.warn('accountConnectDevices failed', err);
      }
      success(t('accountLogin.loginSuccess', { user_id: username }));
      setView('devices');
      refreshDevices();
      startDevicePolling();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally { setLoading(false); }
  }, [doAutoSync, success, t, username, refreshDevices, startDevicePolling]);

  const handleUseLocalOverwrite = useCallback(async () => {
    setLoading(true); setError(null);
    try {
      await doAutoSync(true);
      try {
        await remoteConnectAPI.accountConnectDevices();
      } catch (err) {
        log.warn('accountConnectDevices failed', err);
      }
      success(t('accountLogin.loginSuccess', { user_id: username }));
      setView('devices');
      refreshDevices();
      startDevicePolling();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally { setLoading(false); }
  }, [doAutoSync, success, t, username, refreshDevices, startDevicePolling]);

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

  const selectDevice = useCallback(async (device: AccountDeviceInfo) => {
    if (!device.online) return;
    if (localDeviceId && device.device_id === localDeviceId) return;
    setLoading(true);
    setError(null);
    try {
      await enterPeerMode(device.device_id, device.device_name);
      success(t('accountLogin.enteredPeerMode', { name: device.device_name }));
      onClose();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [enterPeerMode, localDeviceId, onClose, success, t]);

  const title = view === 'login' || view === 'overwrite'
    ? t('shared:features.accountLogin')
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

        {loading && view === 'devices' && (
          <div className="account-login-dialog__loading-overlay">
            <RefreshCw size={20} className="spinning" />
            <span>{t('accountLogin.processing')}</span>
          </div>
        )}

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
      </div>
    </Modal>
  );
};

export default AccountLoginDialog;
