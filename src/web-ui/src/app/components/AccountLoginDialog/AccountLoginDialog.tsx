/**
 * Account Login + Online Devices
 *
 * Views: login → overwrite (optional) → devices
 * After login / overwrite choice the dialog closes immediately while cloud
 * sync continues in the background; reopening Online Devices shows progress.
 * Clicking an online peer device enters Peer Device Mode and closes the dialog.
 */

import React, { useState, useEffect, useCallback, useRef } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { Modal, Button, Input, Alert } from '@/component-library';
import {
  User, Lock, Server, LogIn, Monitor, CloudDownload, Upload,
  ChevronRight, RefreshCw, Eye, EyeOff, X, Rocket, Copy, Check,
} from 'lucide-react';
import { remoteConnectAPI } from '@/infrastructure/api/service-api/RemoteConnectAPI';
import type { AccountHint, AccountDeviceInfo } from '@/infrastructure/api/service-api/RemoteConnectAPI';
import { RelayDeployWizard } from '@/features/relay-deploy';
import type { RelayDeployResult } from '@/features/relay-deploy';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import { usePeerDeviceMode } from '@/infrastructure/peer-device/PeerDeviceContext';
import { useAccountSyncStore, ensureAccountSyncProgressListener } from '@/infrastructure/account/accountSyncStore';
import type { AccountSyncPhase } from '@/infrastructure/account/accountSyncStore';
import { isAccountAuthFailure } from '@/infrastructure/account/accountErrorUtils';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import './AccountLoginDialog.scss';

const log = createLogger('AccountLoginDialog');

const DEVICE_POLL_FALLBACK_MS = 30_000;

function syncPhaseLabel(
  t: (key: string, options?: Record<string, string | number>) => string,
  phase: AccountSyncPhase,
  current: number | null,
  total: number | null,
): string {
  switch (phase) {
    case 'uploading_settings':
      return t('accountLogin.syncPhaseUploadingSettings');
    case 'downloading_settings':
      return t('accountLogin.syncPhaseDownloadingSettings');
    case 'applying_settings':
      return t('accountLogin.syncPhaseApplyingSettings');
    case 'settings_done':
      return t('accountLogin.syncPhaseSettingsDone');
    case 'listing_sessions':
      return t('accountLogin.syncPhaseListingSessions');
    case 'exporting_sessions':
      return t('accountLogin.syncPhaseExportingSessions', {
        current: current ?? 0,
        total: total ?? 0,
      });
    case 'done':
      return t('accountLogin.syncDoneShort');
    case 'failed':
      return t('accountLogin.syncFailed');
    case 'starting':
    default:
      return t('accountLogin.syncing');
  }
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
  const syncStatus = useAccountSyncStore((s) => s.status);
  const syncProgress = useAccountSyncStore((s) => s.progress);
  const setSyncing = useAccountSyncStore((s) => s.setSyncing);
  const setSyncDone = useAccountSyncStore((s) => s.setDone);
  const setSyncFailed = useAccountSyncStore((s) => s.setFailed);

  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [authServer, setAuthServer] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [showPassword, setShowPassword] = useState(false);
  const [view, setView] = useState<View>('login');
  const [showRelayDeploy, setShowRelayDeploy] = useState(false);

  const [devices, setDevices] = useState<AccountDeviceInfo[]>([]);
  const [localDeviceId, setLocalDeviceId] = useState<string | null>(null);
  /** Only true after a successful list_devices response; gates the empty-state copy. */
  const [devicesReady, setDevicesReady] = useState(false);
  const [relayError, setRelayError] = useState<string | null>(null);
  /** Relay URL of the current account session, shown in the devices view. */
  const [accountRelayUrl, setAccountRelayUrl] = useState('');
  const [copiedServerUrl, setCopiedServerUrl] = useState(false);
  const refreshTimer = useRef<ReturnType<typeof setInterval> | null>(null);
  /** Prevent overlapping background syncs from rapid clicks. */
  const syncInFlightRef = useRef(false);

  const resetState = useCallback(() => {
    setDevices([]);
    setLocalDeviceId(null);
    setDevicesReady(false);
    setRelayError(null);
    setAccountRelayUrl('');
    setCopiedServerUrl(false);
    if (refreshTimer.current) { clearInterval(refreshTimer.current); refreshTimer.current = null; }
  }, []);

  const handleCopyRelayUrl = useCallback(async () => {
    if (!accountRelayUrl) return;
    try {
      await navigator.clipboard.writeText(accountRelayUrl);
      setCopiedServerUrl(true);
      window.setTimeout(() => setCopiedServerUrl(false), 1500);
    } catch (e) {
      log.warn('copy relay url failed', e);
    }
  }, [accountRelayUrl]);

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

  const markRelayUnreachable = useCallback(() => {
    setDevicesReady(false);
    setRelayError(t('accountLogin.relayUnreachable'));
  }, [t]);

  const refreshDevices = useCallback(async () => {
    try {
      let list = await remoteConnectAPI.accountListDevices();
      const localOffline = list.some(d => d.device_id === localDeviceId && !d.online);
      if (localOffline && localDeviceId) {
        await new Promise(r => setTimeout(r, 1500));
        list = await remoteConnectAPI.accountListDevices();
      }
      setDevices(list);
      setDevicesReady(true);
      setRelayError(null);
    } catch (e) {
      log.warn('refreshDevices failed', e);
      if (isAccountAuthFailure(e)) {
        await handleSessionExpired(e);
      } else {
        markRelayUnreachable();
      }
    }
  }, [localDeviceId, handleSessionExpired, markRelayUnreachable]);

  const handleRetryConnect = useCallback(async () => {
    setLoading(true);
    setRelayError(null);
    try {
      await remoteConnectAPI.accountConnectDevices();
      await refreshDevices();
    } catch (err) {
      log.warn('retry connect failed', err);
      if (isAccountAuthFailure(err)) {
        await handleSessionExpired(err);
        return;
      }
      markRelayUnreachable();
    } finally {
      setLoading(false);
    }
  }, [handleSessionExpired, markRelayUnreachable, refreshDevices]);

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
    ensureAccountSyncProgressListener();
  }, []);

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
      if (hint) { setUsername(hint.username); setAuthServer(hint.relay_url); setAccountRelayUrl(hint.relay_url); }
    });
    remoteConnectAPI.accountStatus().then(async (status) => {
      if (status.logged_in && status.user_id) {
        setView('devices');
        try {
          await remoteConnectAPI.accountConnectDevices();
          // Re-read after AuthOk may have adopted the account-bound device_id.
          try {
            const info = await remoteConnectAPI.getDeviceInfo();
            setLocalDeviceId(info.device_id);
          } catch (e) {
            log.warn('getDeviceInfo after connect failed', e);
          }
        } catch (err) {
          log.warn('accountConnectDevices failed', err);
          if (isAccountAuthFailure(err)) {
            await handleSessionExpired(err);
            return;
          }
          markRelayUnreachable();
        }
        void refreshDevices();
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

    return () => {
      if (refreshTimer.current) { clearInterval(refreshTimer.current); refreshTimer.current = null; }
      unlistenPresence();
    };
  }, [
    isOpen,
    refreshDevices,
    resetState,
    startDevicePolling,
    applyPresenceOnline,
    handleSessionExpired,
    markRelayUnreachable,
  ]);

  const validate = useCallback(() => {
    if (!username.trim() || !password.trim() || !authServer.trim()) {
      setError(t('accountLogin.emptyFields'));
      return false;
    }
    setError(null);
    return true;
  }, [username, password, authServer, t]);

  /**
   * Run cloud sync + device connect in the background. Dialog can close
   * immediately; progress is visible when Online Devices is reopened.
   */
  const startBackgroundSync = useCallback((isFirstLogin: boolean) => {
    if (syncInFlightRef.current) {
      log.warn('Account sync already in flight; skipping duplicate start');
      return;
    }
    syncInFlightRef.current = true;
    ensureAccountSyncProgressListener();
    setSyncing();
    info(t('accountLogin.syncStarted'));

    // Connect device presence immediately so Online Devices can populate
    // while the heavier settings/session sync continues.
    void remoteConnectAPI.accountConnectDevices().catch((err) => {
      log.warn('accountConnectDevices failed at sync start', err);
    });

    void (async () => {
      try {
        let configJson = '{}';
        if (isFirstLogin) {
          useAccountSyncStore.getState().applyProgress({
            phase: 'uploading_settings',
            percent: 2,
          });
          try {
            const exported = await configAPI.exportConfig();
            configJson = JSON.stringify(exported);
          } catch (e) {
            log.warn('export config failed', e);
          }
        }
        const wp = workspacePath || '/';
        const maxAttempts = 3;
        let result: Awaited<ReturnType<typeof remoteConnectAPI.accountAutoSync>> | null = null;
        let lastError: unknown = null;
        for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
          try {
            result = await remoteConnectAPI.accountAutoSync(isFirstLogin, wp, configJson);
            lastError = null;
            break;
          } catch (e) {
            lastError = e;
            log.warn(`Auto-sync attempt ${attempt}/${maxAttempts} failed`, e);
            if (attempt < maxAttempts) {
              info(t('accountLogin.syncRetrying', { attempt, max: maxAttempts }));
              await new Promise((resolve) => setTimeout(resolve, 2000 * attempt));
            }
          }
        }
        if (!result) {
          throw lastError instanceof Error
            ? lastError
            : new Error(String(lastError ?? 'auto-sync failed'));
        }
        log.info(
          `Auto-sync done: settings=${result.settings_synced} exported=${result.sessions_exported}`,
        );
        if (result.settings_synced && !isFirstLogin) {
          try {
            await configAPI.reloadConfig();
            configManager.clearCache();
            success(t('accountLogin.settingsApplied'));
          } catch (e) {
            log.warn('reloadConfig after sync failed', e);
          }
        }
        setSyncDone(result);
        success(t('accountLogin.syncDone', {
          exported: result.sessions_exported,
        }));
      } catch (e) {
        log.error('Auto-sync failed', e);
        setSyncFailed(e instanceof Error ? e.message : String(e));
        warning(t('accountLogin.syncFailed'));
      } finally {
        syncInFlightRef.current = false;
      }
    })();
  }, [
    info,
    setSyncDone,
    setSyncFailed,
    setSyncing,
    success,
    t,
    warning,
    workspacePath,
  ]);

  const performLogin = useCallback(async (server: string, user: string, pass: string) => {
    setLoading(true); setError(null);
    try {
      const result = await remoteConnectAPI.accountLogin(server, user, pass);
      if (result.has_cloud_settings) {
        setView('overwrite');
        setLoading(false);
        return;
      }
      success(t('accountLogin.loginSuccess', { user_id: result.user_id }));
      setAccountRelayUrl(server);
      startBackgroundSync(true);
      onClose();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally { setLoading(false); }
  }, [startBackgroundSync, success, t, onClose]);

  const handleLogin = useCallback(async () => {
    if (!validate()) return;
    await performLogin(authServer.trim(), username.trim(), password);
  }, [validate, authServer, username, password, performLogin]);

  /**
   * Deploy wizard finished: relay deployed and the first account registered.
   * Fill the form and sign in against the new relay right away.
   */
  const handleRelayRegistered = useCallback((result: RelayDeployResult) => {
    setShowRelayDeploy(false);
    setAuthServer(result.relayUrl);
    setUsername(result.username);
    setPassword(result.password);
    void performLogin(result.relayUrl, result.username, result.password);
  }, [performLogin]);

  const handleConfirmOverwrite = useCallback(() => {
    setError(null);
    success(t('accountLogin.loginSuccess', { user_id: username }));
    startBackgroundSync(false);
    onClose();
  }, [onClose, startBackgroundSync, success, t, username]);

  const handleUseLocalOverwrite = useCallback(() => {
    setError(null);
    success(t('accountLogin.loginSuccess', { user_id: username }));
    startBackgroundSync(true);
    onClose();
  }, [onClose, startBackgroundSync, success, t, username]);

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
    if (syncStatus === 'syncing') {
      info(t('accountLogin.syncInProgressHint'));
      return;
    }
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
  }, [enterPeerMode, info, localDeviceId, onClose, success, syncStatus, t]);

  const title = view === 'login' || view === 'overwrite'
    ? t('shared:features.accountLogin')
    : t('accountLogin.devices');

  return (
    <>
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
              <div className="account-login-dialog__deploy-entry">
                <span>{t('relayDeploy.entryHint')}</span>
                <button
                  type="button"
                  className="account-login-dialog__deploy-link"
                  onClick={() => setShowRelayDeploy(true)}
                  disabled={loading}
                >
                  <Rocket size={13} />
                  {t('relayDeploy.entryAction')}
                </button>
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
            {accountRelayUrl && (
              <div className="account-login-dialog__server-line">
                <Server size={13} />
                <span className="account-login-dialog__server-url" title={accountRelayUrl}>
                  {accountRelayUrl}
                </span>
                <button
                  type="button"
                  className="account-login-dialog__copy-btn"
                  onClick={handleCopyRelayUrl}
                  title={t('accountLogin.copyServerUrl')}
                >
                  {copiedServerUrl ? <Check size={13} /> : <Copy size={13} />}
                </button>
              </div>
            )}
            {syncStatus !== 'idle' && !relayError && (
              <div className={`account-login-dialog__sync-indicator ${syncStatus}`}>
                <div className="account-login-dialog__sync-indicator-row">
                  {syncStatus === 'syncing' && <RefreshCw size={14} className="spinning" />}
                  {syncStatus === 'done' && <span>✓</span>}
                  {syncStatus === 'failed' && <span>⚠</span>}
                  <span className="account-login-dialog__sync-indicator-text">
                    {syncStatus === 'syncing' && syncPhaseLabel(
                      t,
                      syncProgress.phase,
                      syncProgress.current,
                      syncProgress.total,
                    )}
                    {syncStatus === 'done' && t('accountLogin.syncDoneShort')}
                    {syncStatus === 'failed' && t('accountLogin.syncFailed')}
                  </span>
                  {syncStatus === 'syncing' && (
                    <span className="account-login-dialog__sync-indicator-percent">
                      {t('accountLogin.syncProgressPercent', { percent: syncProgress.percent })}
                    </span>
                  )}
                </div>
                {syncStatus === 'syncing' && (
                  <div
                    className="account-login-dialog__sync-progress-track"
                    role="progressbar"
                    aria-valuemin={0}
                    aria-valuemax={100}
                    aria-valuenow={syncProgress.percent}
                  >
                    <div
                      className="account-login-dialog__sync-progress-fill"
                      style={{ width: `${Math.max(2, syncProgress.percent)}%` }}
                    />
                  </div>
                )}
              </div>
            )}
            {relayError && (
              <div className="account-login-dialog__error-banner">
                <Alert
                  type="error"
                  message={relayError}
                  className="account-login-dialog__error-alert"
                />
              </div>
            )}
            <div className="account-login-dialog__device-list">
              {!relayError && devicesReady && devices.length === 0 && (
                <div className="account-login-dialog__empty">{t('accountLogin.noDevices')}</div>
              )}
              {!relayError && devices.map((d) => {
                const isLocal = localDeviceId === d.device_id;
                return (
                <div key={d.device_id}
                  className={`account-login-dialog__device-card ${d.online ? '' : 'offline'} ${isLocal ? 'current' : ''} ${syncStatus === 'syncing' && !isLocal ? 'syncing' : ''}`}
                  onClick={() => !isLocal && selectDevice(d)}>
                  <Monitor size={16} />
                  <div className="account-login-dialog__device-info">
                    <span className="account-login-dialog__device-name">
                      {d.device_name}
                      {isLocal && <span className="account-login-dialog__device-badge">{t('accountLogin.thisDevice')}</span>}
                    </span>
                    <span className="account-login-dialog__device-meta">
                      <span className="account-login-dialog__device-id">
                        {d.device_id.slice(0, 8)}
                      </span>
                      <span className="account-login-dialog__device-status">
                        {' · '}
                        {d.online ? t('accountLogin.online') : t('accountLogin.offline')}
                      </span>
                    </span>
                  </div>
                  {isLocal
                    ? null
                    : <>
                        {d.online && syncStatus !== 'syncing' && <ChevronRight size={14} />}
                        {d.online && syncStatus === 'syncing' && (
                          <RefreshCw size={14} className="spinning" aria-label={t('accountLogin.syncing')} />
                        )}
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
              {relayError && (
                <Button variant="primary" size="small" onClick={handleRetryConnect} disabled={loading}>
                  <RefreshCw size={14} />
                  {t('accountLogin.retryConnect')}
                </Button>
              )}
              <Button variant="secondary" size="small" onClick={handleLogout} disabled={loading}>
                {t('accountLogin.logout')}
              </Button>
            </div>
          </div>
        )}
      </div>
      </Modal>

      {showRelayDeploy && (
        <RelayDeployWizard
          isOpen={showRelayDeploy}
          onClose={() => setShowRelayDeploy(false)}
          onRegistered={handleRelayRegistered}
        />
      )}
    </>
  );
};

export default AccountLoginDialog;
