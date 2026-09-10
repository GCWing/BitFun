/** Account login and authenticated device connections. */

import { OverflowText, Alert, Button, Icon, IconButton, ScrollArea, StatusPill } from '@openbitfun/ui';
import React, { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import {
  confirmDanger,
} from '@/infrastructure/confirm-dialog';
import { LogIn, Monitor } from 'lucide-react';
import { remoteConnectAPI } from '@/infrastructure/api/service-api/RemoteConnectAPI';
import type {
  AccountDeviceInfo,
  OnlineDeviceInfo,
} from '@/infrastructure/api/service-api/RemoteConnectAPI';
import { accountIdentityService, useAccountIdentity } from '@/infrastructure/account-identity';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import { usePeerDeviceMode } from '@/infrastructure/peer-device/peerDeviceContextState';
import {
  isAccountAuthFailure,
  isRelayUnreachable,
} from '@/infrastructure/account/accountErrorUtils';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import './AccountPanel.scss';

const log = createLogger('AccountPanel');

const DEVICE_POLL_FALLBACK_MS = 30_000;
const DEVICE_CONNECT_MAX_ATTEMPTS = 5;
const DEVICE_CONNECT_RECOVERY_INTERVAL_MS = 30_000;
const DEVICE_LIST_FAILURE_THRESHOLD = 3;

async function connectDevicesWithRetry(
  isCurrent: () => boolean,
): Promise<OnlineDeviceInfo[]> {
  let lastError: unknown = null;
  for (let attempt = 1; attempt <= DEVICE_CONNECT_MAX_ATTEMPTS; attempt += 1) {
    if (!isCurrent()) {
      throw new Error('account context changed');
    }
    try {
      return await remoteConnectAPI.accountConnectDevices();
    } catch (error) {
      lastError = error;
      if (isAccountAuthFailure(error)
        || !isRelayUnreachable(error)
        || attempt === DEVICE_CONNECT_MAX_ATTEMPTS) {
        throw error;
      }
      const delayMs = 500 * (2 ** (attempt - 1));
      log.warn(
        `Device connection attempt ${attempt}/${DEVICE_CONNECT_MAX_ATTEMPTS} failed; retrying`,
        error,
      );
      await new Promise(resolve => setTimeout(resolve, delayMs));
    }
  }
  throw lastError;
}

interface AccountPanelProps {
  /** Close the whole Remote Connect dialog (used when entering peer mode). */
  onCloseDialog: () => void;
}

type View = 'login' | 'devices';

export const AccountPanel: React.FC<AccountPanelProps> = ({
  onCloseDialog,
}) => {
  const { t, formatRelativeTime } = useI18n('common');
  const { success } = useNotification();
  const { peerMode, switchToDevice, switchToLocal } = usePeerDeviceMode();
  const identity = useAccountIdentity();
  const username = identity.me?.user.login ?? '';
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [view, setView] = useState<View>('login');

  const [devices, setDevices] = useState<AccountDeviceInfo[]>([]);
  const [localDeviceId, setLocalDeviceId] = useState<string | null>(null);
  /** True after either device presence or a list_devices response is available. */
  const [devicesReady, setDevicesReady] = useState(false);
  const [relayError, setRelayError] = useState<string | null>(null);
  /** Account epoch whose presence events may update the device list. */
  const [activeAccountEpoch, setActiveAccountEpoch] = useState<number | null>(null);
  const refreshTimer = useRef<ReturnType<typeof setInterval> | null>(null);
  /** Reject late responses after unmount or an account login/logout transition. */
  const mountedRef = useRef(false);
  const accountEpochRef = useRef(0);
  const refreshRequestRef = useRef(0);
  /** Allow at most one list_devices request per account epoch. */
  const refreshInFlightRef = useRef<{ epoch: number; requestId: number } | null>(null);
  /** A working device-routing WS is independent evidence that Relay is reachable. */
  const deviceRoutingReadyRef = useRef(false);
  const deviceListFailureCountRef = useRef(0);
  /** Coalesce manual and background recovery so they never replace each other's WS. */
  const deviceReconnectInFlightRef = useRef(false);
  const invalidateAccountRequests = useCallback(() => {
    accountEpochRef.current += 1;
    refreshRequestRef.current += 1;
    refreshInFlightRef.current = null;
    deviceRoutingReadyRef.current = false;
    deviceListFailureCountRef.current = 0;
    setActiveAccountEpoch(null);
    return accountEpochRef.current;
  }, []);

  const isAccountEpochCurrent = useCallback((epoch: number) => (
    mountedRef.current && accountEpochRef.current === epoch
  ), []);

  const sortedDevices = useMemo(() => [...devices].sort((left, right) => {
    const leftLocal = left.device_id === localDeviceId;
    const rightLocal = right.device_id === localDeviceId;
    if (leftLocal !== rightLocal) return leftLocal ? -1 : 1;
    if (left.online !== right.online) return left.online ? -1 : 1;
    return (left.device_name || left.device_id).localeCompare(right.device_name || right.device_id);
  }), [devices, localDeviceId]);

  const resetState = useCallback(() => {
    setActiveAccountEpoch(null);
    setDevices([]);
    setLocalDeviceId(null);
    setDevicesReady(false);
    setRelayError(null);
    refreshInFlightRef.current = null;
    deviceRoutingReadyRef.current = false;
    deviceListFailureCountRef.current = 0;
    if (refreshTimer.current) { clearInterval(refreshTimer.current); refreshTimer.current = null; }
  }, []);

  const handleSessionExpired = useCallback(async (_error: unknown, expectedEpoch: number) => {
    if (!isAccountEpochCurrent(expectedEpoch)) return;
    invalidateAccountRequests();
    // Authenticated backend commands invalidate only the generation/token that
    // produced their 401. Do not issue a second unconditional logout here: a
    // late frontend response must never clear a newer login.
    resetState();
    setView('login');
    setError(t('accountLogin.sessionExpired'));
  }, [invalidateAccountRequests, isAccountEpochCurrent, resetState, t]);

  const markRelayUnreachable = useCallback(() => {
    setDevicesReady(false);
    setRelayError(t('accountLogin.relayUnreachable'));
  }, [t]);

  const refreshDevices = useCallback(async () => {
    const epoch = accountEpochRef.current;
    if (refreshInFlightRef.current?.epoch === epoch) {
      log.debug('Device list refresh already in flight; coalescing duplicate request');
      return;
    }
    const requestId = ++refreshRequestRef.current;
    refreshInFlightRef.current = { epoch, requestId };
    const isCurrent = () => (
      isAccountEpochCurrent(epoch) && refreshRequestRef.current === requestId
    );
    try {
      let list = await remoteConnectAPI.accountListDevices();
      if (!isCurrent()) return;
      const localOffline = list.some(d => d.device_id === localDeviceId && !d.online);
      if (localOffline && localDeviceId) {
        await new Promise(r => setTimeout(r, 1500));
        if (!isCurrent()) return;
        list = await remoteConnectAPI.accountListDevices();
        if (!isCurrent()) return;
      }
      setDevices(list);
      setDevicesReady(true);
      setRelayError(null);
      deviceListFailureCountRef.current = 0;
    } catch (e) {
      if (!isCurrent()) return;
      log.warn('refreshDevices failed', e);
      if (isAccountAuthFailure(e)) {
        await handleSessionExpired(e, epoch);
      } else {
        deviceListFailureCountRef.current += 1;
        // list_devices is an HTTP snapshot while account device routing uses
        // WebSocket. Keep the last/presence-derived list when WS is healthy;
        // one failed snapshot must not be reported as a total Relay outage.
        if (!deviceRoutingReadyRef.current
          && deviceListFailureCountRef.current >= DEVICE_LIST_FAILURE_THRESHOLD) {
          markRelayUnreachable();
        }
      }
    } finally {
      if (refreshInFlightRef.current?.epoch === epoch
        && refreshInFlightRef.current.requestId === requestId) {
        refreshInFlightRef.current = null;
      }
    }
  }, [localDeviceId, handleSessionExpired, isAccountEpochCurrent, markRelayUnreachable]);

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
            last_seen_at: Math.floor(Date.now() / 1000),
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

  /** Latest refreshDevices for the polling interval (avoids stale closures). */
  const refreshDevicesRef = useRef(refreshDevices);
  refreshDevicesRef.current = refreshDevices;

  const startDevicePolling = useCallback(() => {
    if (refreshTimer.current) {
      clearInterval(refreshTimer.current);
    }
    refreshTimer.current = setInterval(
      () => { void refreshDevicesRef.current(); },
      DEVICE_POLL_FALLBACK_MS,
    );
  }, []);

  const attemptDeviceReconnect = useCallback(async (showLoading: boolean) => {
    if (deviceReconnectInFlightRef.current) {
      log.debug('Device routing recovery already in flight; coalescing duplicate request');
      return;
    }
    const epoch = accountEpochRef.current;
    deviceReconnectInFlightRef.current = true;
    if (showLoading) {
      setLoading(true);
      setRelayError(null);
    }
    try {
      const onlineDevices = await connectDevicesWithRetry(
        () => isAccountEpochCurrent(epoch),
      );
      if (!isAccountEpochCurrent(epoch)) return;
      deviceRoutingReadyRef.current = true;
      deviceListFailureCountRef.current = 0;
      applyPresenceOnline(onlineDevices);
      setDevicesReady(true);
      setRelayError(null);
      try {
        const info = await remoteConnectAPI.getDeviceInfo();
        if (!isAccountEpochCurrent(epoch)) return;
        setLocalDeviceId(info.device_id);
      } catch (error) {
        log.warn('getDeviceInfo after reconnect failed', error);
      }
      if (!isAccountEpochCurrent(epoch)) return;
      await refreshDevices();
      startDevicePolling();
    } catch (err) {
      log.warn(
        showLoading ? 'manual device reconnect failed' : 'background device reconnect failed',
        err,
      );
      if (!isAccountEpochCurrent(epoch)) return;
      if (isAccountAuthFailure(err)) {
        await handleSessionExpired(err, epoch);
        return;
      }
      markRelayUnreachable();
    } finally {
      deviceReconnectInFlightRef.current = false;
      if (showLoading && isAccountEpochCurrent(epoch)) setLoading(false);
    }
  }, [
    applyPresenceOnline,
    handleSessionExpired,
    isAccountEpochCurrent,
    markRelayUnreachable,
    refreshDevices,
    startDevicePolling,
  ]);

  const handleRetryConnect = useCallback(() => {
    void attemptDeviceReconnect(true);
  }, [attemptDeviceReconnect]);

  // Initial dial failures have no RelayClient instance to run its built-in
  // reconnect loop. Keep recovering in the background while this account view
  // is active; once a socket succeeds, RelayClient owns subsequent reconnects.
  useEffect(() => {
    if (activeAccountEpoch === null || !relayError) return undefined;
    const timer = setInterval(
      () => { void attemptDeviceReconnect(false); },
      DEVICE_CONNECT_RECOVERY_INTERVAL_MS,
    );
    return () => clearInterval(timer);
  }, [activeAccountEpoch, attemptDeviceReconnect, relayError]);

  /** Connect presence + load the device list for an active account session. */
  const initializeDevices = useCallback(async () => {
    const epoch = accountEpochRef.current;
    try {
      const onlineDevices = await connectDevicesWithRetry(
        () => isAccountEpochCurrent(epoch),
      );
      if (!isAccountEpochCurrent(epoch)) return;
      deviceRoutingReadyRef.current = true;
      deviceListFailureCountRef.current = 0;
      applyPresenceOnline(onlineDevices);
      setDevicesReady(true);
      setRelayError(null);
      // Re-read after AuthOk may have adopted the account-bound device_id.
      try {
        const info = await remoteConnectAPI.getDeviceInfo();
        if (!isAccountEpochCurrent(epoch)) return;
        setLocalDeviceId(info.device_id);
      } catch (e) {
        log.warn('getDeviceInfo after connect failed', e);
      }
    } catch (err) {
      if (!isAccountEpochCurrent(epoch)) return;
      log.warn('accountConnectDevices failed', err);
      if (isAccountAuthFailure(err)) {
        await handleSessionExpired(err, epoch);
        return;
      }
      markRelayUnreachable();
      return;
    }
    if (!isAccountEpochCurrent(epoch)) return;
    void refreshDevices();
    startDevicePolling();
  }, [
    applyPresenceOnline,
    handleSessionExpired,
    isAccountEpochCurrent,
    markRelayUnreachable,
    refreshDevices,
    startDevicePolling,
  ]);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      accountEpochRef.current += 1;
      refreshRequestRef.current += 1;
    };
  }, []);

  useEffect(() => {
    const epoch = accountEpochRef.current;
    remoteConnectAPI.getDeviceInfo().then((info) => {
      if (isAccountEpochCurrent(epoch)) setLocalDeviceId(info.device_id);
    }).catch((e) => { log.warn('getDeviceInfo failed', e); });
    remoteConnectAPI.accountStatus().then(async (status) => {
      if (isAccountEpochCurrent(epoch) && status.logged_in && status.user_id) {
        setActiveAccountEpoch(epoch);
        setView('devices');
        await initializeDevices();
      }
    }).catch((e) => {
      // A failed status probe must not synthesize a logged-out transition.
      log.warn('account status initialization failed', e);
    });

    return () => {
      if (refreshTimer.current) { clearInterval(refreshTimer.current); refreshTimer.current = null; }
    };
  }, [
    initializeDevices,
    isAccountEpochCurrent,
  ]);

  // Subscribe only while a specific account epoch is active. The callback
  // captures that epoch; invalidation flips the ref synchronously, so an old
  // listener cannot update the next account before React runs its cleanup.
  useEffect(() => {
    if (activeAccountEpoch === null) return undefined;
    const subscribedEpoch = activeAccountEpoch;
    const unlistenPresence = api.listen<{
      devices: Array<{ device_id: string; device_name: string }>;
    }>(
      'account://device-presence',
      (payload) => {
        if (isAccountEpochCurrent(subscribedEpoch) && payload?.devices) {
          deviceRoutingReadyRef.current = payload.devices.length > 0;
          if (deviceRoutingReadyRef.current) {
            deviceListFailureCountRef.current = 0;
            setDevicesReady(true);
            setRelayError(null);
          } else {
            // Start a fresh debounce window after routing loss; HTTP failures
            // observed while WS was healthy must not count against it.
            deviceListFailureCountRef.current = 0;
          }
          applyPresenceOnline(payload.devices);
        }
      },
    );
    return unlistenPresence;
  }, [activeAccountEpoch, applyPresenceOnline, isAccountEpochCurrent]);

  /** Show the authenticated device list and connect routing. */
  const completeLogin = useCallback((
    accountEpoch: number,
  ) => {
    if (!isAccountEpochCurrent(accountEpoch)) return;
    setActiveAccountEpoch(accountEpoch);
    setView('devices');
    void initializeDevices();
  }, [initializeDevices, isAccountEpochCurrent]);

  const handleLogin = useCallback(async () => {
    const epoch = invalidateAccountRequests();
    setLoading(true); setError(null);
    try {
      const me = await accountIdentityService.signIn();
      if (!isAccountEpochCurrent(epoch)) return;
      await remoteConnectAPI.accountLogin();
      if (!isAccountEpochCurrent(epoch)) return;
      success(t('accountLogin.loginSuccess', { user_id: me.user.login }));
      completeLogin(epoch);
    } catch (e: unknown) {
      if (isAccountEpochCurrent(epoch)) setError(e instanceof Error ? e.message : String(e));
    } finally {
      if (isAccountEpochCurrent(epoch)) setLoading(false);
    }
  }, [completeLogin, invalidateAccountRequests, isAccountEpochCurrent, success, t]);

  const handleLogout = useCallback(async () => {
    const epoch = invalidateAccountRequests();
    setLoading(true);
    try {
      await accountIdentityService.logout();
      if (!isAccountEpochCurrent(epoch)) return;
      resetState();
      setView('login');
    } catch (e: unknown) {
      if (!isAccountEpochCurrent(epoch)) return;
      // Logout failed before the backend changed the account; resume presence
      // delivery for the still-current frontend epoch.
      setActiveAccountEpoch(epoch);
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      if (isAccountEpochCurrent(epoch)) setLoading(false);
    }
  }, [invalidateAccountRequests, isAccountEpochCurrent, resetState]);

  const handleDeleteDevice = useCallback(async (deviceId: string, deviceName: string) => {
    const isLocal = localDeviceId === deviceId;
    const confirmation = isLocal
      ? t('accountLogin.confirmRemoveCurrentDevice', { name: deviceName })
      : t('accountLogin.confirmRemoveDevice', { name: deviceName });
    const confirmed = await confirmDanger(
      isLocal
        ? t('accountLogin.removeCurrentDevice')
        : t('accountLogin.removeDevice'),
      confirmation,
      {
        confirmText: isLocal
          ? t('accountLogin.removeCurrentDevice')
          : t('accountLogin.removeDevice'),
        cancelText: t('accountLogin.cancel'),
      },
    );
    if (!confirmed) return;
    setLoading(true);
    setError(null);
    const epoch = isLocal ? invalidateAccountRequests() : accountEpochRef.current;
    try {
      await remoteConnectAPI.accountDeleteDevice(deviceId);
      if (!isAccountEpochCurrent(epoch)) return;
      if (isLocal) {
        success(t('accountLogin.currentDeviceRemoved'));
        resetState();
        setView('login');
      } else {
        success(t('accountLogin.deviceRemoved', { name: deviceName }));
        void refreshDevices();
      }
    } catch (e: unknown) {
      if (!isAccountEpochCurrent(epoch)) return;
      if (isAccountAuthFailure(e)) {
        await handleSessionExpired(e, epoch);
      } else {
        const message = e instanceof Error ? e.message : String(e);
        if (isLocal) setActiveAccountEpoch(epoch);
        setError(message);
      }
    } finally {
      if (isAccountEpochCurrent(epoch)) setLoading(false);
    }
  }, [
    handleSessionExpired,
    invalidateAccountRequests,
    isAccountEpochCurrent,
    localDeviceId,
    refreshDevices,
    resetState,
    success,
    t,
  ]);

  const selectDevice = useCallback(async (device: AccountDeviceInfo) => {
    if (!device.online) return;
    // Picking this machine is a normal surface switch back, not a no-op: the
    // window may currently be rendering a peer.
    const isLocalDevice = Boolean(localDeviceId) && device.device_id === localDeviceId;
    setLoading(true);
    setError(null);
    try {
      let outcome: 'activated' | 'superseded';
      if (isLocalDevice) {
        outcome = await switchToLocal();
        if (outcome === 'activated') {
          success(t('accountLogin.deviceSwitcher.switchedLocal'));
        }
      } else {
        outcome = await switchToDevice(device.device_id, device.device_name);
        if (outcome === 'activated') {
          success(t('accountLogin.enteredPeerMode', { name: device.device_name }));
        }
      }
      if (outcome === 'activated') {
        onCloseDialog();
      }
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [
    localDeviceId,
    onCloseDialog,
    success,
    switchToDevice,
    switchToLocal,
    t,
  ]);

  return (
    <>
      <div data-openbitfun-component="remote-account-panel" data-openbitfun-part="root" data-openbitfun-view={view} className="account-panel">
        {error && (
          <div className="account-panel__error-banner" data-openbitfun-component="remote-account-panel" data-openbitfun-part="error">
            <Alert tone="error" message={error} closable onClose={() => setError(null)} />
          </div>
        )}

        {loading && view === 'devices' && (
          <div className="account-panel__loading-overlay" data-openbitfun-component="remote-account-panel" data-openbitfun-part="loading">
            <Icon name="refresh" size="lg" className="spinning" style={{ width: 20, height: 20 }} />
            <span>{t('accountLogin.processing')}</span>
          </div>
        )}

        {view === 'login' && (
          <ScrollArea className="account-panel__scroll" data-openbitfun-component="remote-account-panel" data-openbitfun-part="scroll">
            <div className="account-panel__login-card" data-openbitfun-component="remote-account-panel" data-openbitfun-part="form">
              <span className="account-panel__login-icon" aria-hidden="true"><Icon name="user" size="lg" /></span>
              <p className="account-panel__value-prop">{t('accountLogin.loginValueProp')}</p>
              <p className="account-panel__security-note">{t('accountLogin.securityNote')}</p>
              <div className="account-panel__actions" data-openbitfun-component="remote-account-panel" data-openbitfun-part="actions">
                <Button variant="primary" size="sm" leadingIcon={<LogIn />} onClick={handleLogin} loading={loading}>
                  {loading ? t('accountLogin.processing') : t('accountLogin.login')}
                </Button>
              </div>
            </div>
          </ScrollArea>
        )}

        {view === 'devices' && (
          <ScrollArea className="account-panel__scroll" data-openbitfun-component="remote-account-panel" data-openbitfun-part="scroll">
            <div className="account-panel__identity-line">
              <Icon name="user" size="lg" aria-hidden="true" />
              <span className="account-panel__identity-copy">
                <span className="account-panel__identity-label">{t('accountLogin.signedInAccount')}</span>
                <OverflowText className="account-panel__identity-name" title={username}>{username.trim()}</OverflowText>
              </span>
              <Button variant="text" size="sm" onClick={handleLogout} disabled={loading}>
                {t('accountLogin.logout')}
              </Button>
            </div>
            <div className="account-panel__section-heading">
              <h3>{t('accountLogin.linkedDevices')}</h3>
              <Button variant="text" size="sm" leadingIcon={<Icon name="refresh" size="sm" />}
                onClick={relayError ? handleRetryConnect : refreshDevices} disabled={loading}>
                {t(relayError ? 'accountLogin.retryConnect' : 'accountLogin.refreshDevices')}
              </Button>
            </div>
            <div className="account-panel__devices-card">
              {relayError && (
                <div className="account-panel__error-banner" data-openbitfun-component="remote-account-panel" data-openbitfun-part="error">
                  <Alert
                    tone="error"
                    message={relayError}
                  />
                </div>
              )}
              <div className="account-panel__device-list" data-openbitfun-component="remote-account-panel" data-openbitfun-part="deviceList">
                {!relayError && devicesReady && devices.length === 0 && (
                  <div className="account-panel__empty">{t('accountLogin.noDevices')}</div>
                )}
                {!relayError && !devicesReady && (
                  <div className="account-panel__empty account-panel__empty--loading" role="status">
                    <Icon name="refresh" size="sm" className="spinning" />
                    {t('accountLogin.loadingDevices')}
                  </div>
                )}
                {!relayError && sortedDevices.map((d) => {
                  const isLocal = localDeviceId === d.device_id;
                  // This machine is selectable while the window renders a peer,
                  // so the dialog can bring the UI back without disconnecting.
                  const isSelectable = isLocal
                    ? peerMode.active
                    : d.online;
                  const removeLabel = isLocal
                    ? t('accountLogin.removeCurrentDevice')
                    : t('accountLogin.removeDevice');
                  const displayName = d.device_name || t('accountLogin.unknownDevice');
                  const DeviceEntry = isSelectable ? 'button' : 'div';
                  return (
                  <div data-openbitfun-component="remote-account-panel" data-openbitfun-part="deviceCard" key={d.device_id}
                    data-openbitfun-state={[
                      !d.online && 'offline',
                      isLocal && 'current',
                    ].filter(Boolean).join(' ') || undefined}
                    className={`account-panel__device-card ${isSelectable ? 'selectable' : ''} ${d.online ? '' : 'offline'} ${isLocal ? 'current' : ''}`}>
                    <DeviceEntry
                      className="account-panel__device-select"
                      {...(isSelectable ? {
                        type: 'button' as const,
                        onClick: () => void selectDevice(d),
                        disabled: loading,
                        'aria-label': t('accountLogin.openDevice', { name: displayName }),
                      } : {})}
                    >
                      <Monitor size={16} />
                      <span className="account-panel__device-info">
                        <span className="account-panel__device-name">
                          <OverflowText title={displayName}>{displayName}</OverflowText>
                          {isLocal && <StatusPill tone="neutral" className="account-panel__device-badge">{t('accountLogin.thisDevice')}</StatusPill>}
                        </span>
                        <span className="account-panel__device-meta">
                          <span className="account-panel__device-status">
                            {d.online
                              ? t('accountLogin.online')
                              : d.last_seen_at
                                ? t('accountLogin.lastSeen', {
                                  time: formatRelativeTime(d.last_seen_at * 1000),
                                })
                                : t('accountLogin.offline')}
                          </span>
                        </span>
                      </span>
                      {isSelectable && <Icon name="chevron-right" size="sm" />}
                    </DeviceEntry>
                    <IconButton
                      aria-label={`${removeLabel}: ${displayName}`}
                      disabled={loading}
                      icon={<Icon name="delete" size="sm" />}
                      onClick={(e) => { e.stopPropagation(); handleDeleteDevice(d.device_id, displayName); }}
                      size="sm"
                      title={removeLabel}
                      variant="quiet"
                    />
                  </div>
                  );
                })}
              </div>
            </div>
          </ScrollArea>
        )}
      </div>


    </>
  );
};

export default AccountPanel;
