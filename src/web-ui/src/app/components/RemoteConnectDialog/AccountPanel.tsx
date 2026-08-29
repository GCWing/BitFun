/**
 * Account ("My BitFun") panel inside the Remote Connect dialog.
 *
 * Views: login → overwrite (optional) → devices
 * Unlike the old standalone dialog, a successful login keeps the panel open
 * and lands on the devices view so sync progress stays visible in place.
 *
 * Sync-choice invariants (do not regress):
 * - When the relay already has cloud settings, `account_login` keeps the
 *   session memory-only until `account_finalize_login`. Canceling the
 *   overwrite view, switching away from this panel, or closing the dialog
 *   must conditionally cancel its opaque owner so a killed process does not
 *   restore login.
 * - One-click deploy opens `RelayDeployWizard` (same feature as the Network
 *   group), not an external README. See `src/features/relay-deploy/README.md`.
 */

import { Alert, Button, Field, Icon, IconButton, Input, ScrollArea } from '@bitfun/ui';
import React, { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import {
  confirmDanger,
  confirmWarning,
} from '@/infrastructure/confirm-dialog';
import {
  Lock,
  Server,
  LogIn,
  Monitor,
  CloudDownload,
  RefreshCw,
  EyeOff,
  Rocket,
} from 'lucide-react';
import { remoteConnectAPI } from '@/infrastructure/api/service-api/RemoteConnectAPI';
import type {
  AccountHint,
  AccountDeviceInfo,
  OnlineDeviceInfo,
} from '@/infrastructure/api/service-api/RemoteConnectAPI';
import { RelayDeployWizard } from '@/features/relay-deploy';
import type { RelayDeployResult } from '@/features/relay-deploy';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import { usePeerDeviceMode } from '@/infrastructure/peer-device/peerDeviceContextState';
import { useAccountSyncStore, ensureAccountSyncProgressListener } from '@/infrastructure/account/accountSyncStore';
import type { AccountSyncPhase } from '@/infrastructure/account/accountSyncStore';
import {
  isAccountAuthFailure,
  isRelayUnreachable,
} from '@/infrastructure/account/accountErrorUtils';
import { useNotification } from '@/shared/notification-system';
import { copyTextToClipboard } from '@/shared/utils/textSelection';
import { createLogger } from '@/shared/utils/logger';
import './AccountPanel.scss';

const log = createLogger('AccountPanel');

const DEVICE_POLL_FALLBACK_MS = 30_000;
const DEVICE_CONNECT_MAX_ATTEMPTS = 5;
const DEVICE_CONNECT_RECOVERY_INTERVAL_MS = 30_000;
const DEVICE_LIST_FAILURE_THRESHOLD = 3;
const ACCOUNT_TRANSITION_MAX_ATTEMPTS = 4;

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

function parseRelayServer(value: string): URL | null {
  try {
    const url = new URL(value.trim());
    if (!['http:', 'https:'].includes(url.protocol)
      || !url.hostname
      || url.username
      || url.password
      || url.search
      || url.hash) {
      return null;
    }
    return url;
  } catch {
    return null;
  }
}

async function cancelPendingLoginWithRetry(pendingLoginId: string): Promise<boolean> {
  let lastError: unknown = null;
  for (let attempt = 1; attempt <= ACCOUNT_TRANSITION_MAX_ATTEMPTS; attempt += 1) {
    try {
      return await remoteConnectAPI.accountCancelPendingLogin(pendingLoginId);
    } catch (error) {
      lastError = error;
      if (attempt === ACCOUNT_TRANSITION_MAX_ATTEMPTS) break;
      log.warn(
        `Pending login cancel attempt ${attempt}/${ACCOUNT_TRANSITION_MAX_ATTEMPTS} was ambiguous; retrying`,
        error,
      );
      await new Promise(resolve => setTimeout(resolve, 250 * (2 ** (attempt - 1))));
    }
  }
  throw lastError;
}

async function finalizePendingLoginWithRetry(pendingLoginId: string): Promise<void> {
  let lastError: unknown = null;
  for (let attempt = 1; attempt <= ACCOUNT_TRANSITION_MAX_ATTEMPTS; attempt += 1) {
    try {
      await remoteConnectAPI.accountFinalizeLogin(pendingLoginId);
      return;
    } catch (error) {
      lastError = error;
      if (attempt === ACCOUNT_TRANSITION_MAX_ATTEMPTS) break;
      log.warn(
        `Pending login finalize attempt ${attempt}/${ACCOUNT_TRANSITION_MAX_ATTEMPTS} was ambiguous; retrying`,
        error,
      );
      await new Promise(resolve => setTimeout(resolve, 250 * (2 ** (attempt - 1))));
    }
  }
  throw lastError;
}

/** Quota / payload-limit failures will not succeed on blind retry. */
function isNonRetryableSyncError(error: unknown): boolean {
  const msg = error instanceof Error ? error.message : String(error ?? '');
  const lower = msg.toLowerCase();
  return (
    lower.includes('http 507')
    || lower.includes('insufficient storage')
    || lower.includes('quota is full')
    || lower.includes('http 413')
    || lower.includes('payload too large')
  );
}

function syncFailureMessage(
  t: (key: string, options?: Record<string, string | number>) => string,
  error: unknown,
): string {
  if (isNonRetryableSyncError(error)) {
    const msg = error instanceof Error ? error.message : String(error ?? '');
    if (
      msg.toLowerCase().includes('http 413')
      || msg.toLowerCase().includes('payload too large')
    ) {
      return t('accountLogin.syncPayloadTooLarge');
    }
    return t('accountLogin.syncQuotaFull');
  }
  return t('accountLogin.syncFailed');
}

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

interface AccountPanelProps {
  /** Close the whole Remote Connect dialog (used when entering peer mode). */
  onCloseDialog: () => void;
}

type View = 'login' | 'overwrite' | 'devices';

export const AccountPanel: React.FC<AccountPanelProps> = ({
  onCloseDialog,
}) => {
  const { t, formatRelativeTime } = useI18n('common');
  const { success, info, warning } = useNotification();
  const { workspacePath } = useCurrentWorkspace();
  const { peerMode, switchToDevice, switchToLocal } = usePeerDeviceMode();
  const syncStatus = useAccountSyncStore((s) => s.status);
  const syncProgress = useAccountSyncStore((s) => s.progress);
  const lastSyncError = useAccountSyncStore((s) => s.lastError);
  const lastSyncIsFirstLogin = useAccountSyncStore((s) => s.lastSyncIsFirstLogin);
  const setSyncing = useAccountSyncStore((s) => s.setSyncing);
  const setSyncDone = useAccountSyncStore((s) => s.setDone);
  const setSyncFailed = useAccountSyncStore((s) => s.setFailed);
  const clearSync = useAccountSyncStore((s) => s.clear);

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
  /** True after either device presence or a list_devices response is available. */
  const [devicesReady, setDevicesReady] = useState(false);
  const [relayError, setRelayError] = useState<string | null>(null);
  /** Relay URL of the current account session, shown in the devices view. */
  const [accountRelayUrl, setAccountRelayUrl] = useState('');
  const [copiedServerUrl, setCopiedServerUrl] = useState(false);
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
  /** Prevent overlapping background syncs from rapid clicks. */
  const syncInFlightRef = useRef(false);
  /** Opaque backend owner ID for the memory-only overwrite decision. */
  const pendingLoginIdRef = useRef<string | null>(null);
  /** Track the overwrite view for conditional unmount cleanup. */
  const viewRef = useRef<View>(view);
  viewRef.current = view;

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
    setAccountRelayUrl('');
    setCopiedServerUrl(false);
    refreshInFlightRef.current = null;
    deviceRoutingReadyRef.current = false;
    deviceListFailureCountRef.current = 0;
    if (refreshTimer.current) { clearInterval(refreshTimer.current); refreshTimer.current = null; }
  }, []);

  const handleCopyRelayUrl = useCallback(async () => {
    if (!accountRelayUrl) return;
    const copied = await copyTextToClipboard(accountRelayUrl);
    if (copied) {
      setCopiedServerUrl(true);
      window.setTimeout(() => setCopiedServerUrl(false), 1500);
    } else {
      warning(t('accountLogin.copyServerFailed'));
    }
  }, [accountRelayUrl, t, warning]);

  const handleSessionExpired = useCallback(async (_error: unknown, expectedEpoch: number) => {
    if (!isAccountEpochCurrent(expectedEpoch)) return;
    invalidateAccountRequests();
    // Invalidate detached retries before the logout request yields control.
    syncInFlightRef.current = false;
    pendingLoginIdRef.current = null;
    clearSync();
    // Authenticated backend commands invalidate only the generation/token that
    // produced their 401. Do not issue a second unconditional logout here: a
    // late frontend response must never clear a newer login.
    resetState();
    setView('login');
    setError(t('accountLogin.sessionExpired'));
  }, [clearSync, invalidateAccountRequests, isAccountEpochCurrent, resetState, t]);

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
    ensureAccountSyncProgressListener();
    return () => {
      mountedRef.current = false;
      accountEpochRef.current += 1;
      refreshRequestRef.current += 1;
    };
  }, []);

  // Unmounting (dialog close or group switch) during the sync-choice step
  // abandons the incomplete login — pair with `account_finalize_login`.
  // The dialog tree unmounts this panel directly, so this must be a cleanup,
  // not an effect gated on a prop flip.
  useEffect(() => {
    return () => {
      if (viewRef.current === 'overwrite') {
        syncInFlightRef.current = false;
        clearSync();
        const pendingLoginId = pendingLoginIdRef.current;
        if (pendingLoginId) {
          void cancelPendingLoginWithRetry(pendingLoginId)
            .then(() => {
              if (pendingLoginIdRef.current === pendingLoginId) {
                pendingLoginIdRef.current = null;
              }
            })
            .catch((e) => {
              log.warn('pending login cancel on overwrite abandon failed', e);
            });
        }
      }
    };
  }, [clearSync]);

  useEffect(() => {
    const epoch = accountEpochRef.current;
    remoteConnectAPI.getDeviceInfo().then((info) => {
      if (isAccountEpochCurrent(epoch)) setLocalDeviceId(info.device_id);
    }).catch((e) => { log.warn('getDeviceInfo failed', e); });
    remoteConnectAPI.accountGetCredentialHint().then((hint: AccountHint | null) => {
      if (hint && isAccountEpochCurrent(epoch)) {
        setUsername(hint.username);
        setAuthServer(hint.relay_url);
        setAccountRelayUrl(hint.relay_url);
      }
    });
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

  const validate = useCallback(() => {
    if (!username.trim() || !password || !authServer.trim()) {
      setError(t('accountLogin.emptyFields'));
      return false;
    }
    if (username.trim().length > 128 || password.length > 1024) {
      setError(t('accountLogin.invalidCredentialsLength'));
      return false;
    }
    if (!parseRelayServer(authServer)) {
      setError(t('accountLogin.invalidServer'));
      return false;
    }
    setError(null);
    return true;
  }, [username, password, authServer, t]);

  /**
   * Run cloud sync + device connect in the background. Progress is visible
   * in the devices view while it continues.
   */
  const startBackgroundSync = useCallback((isFirstLogin: boolean) => {
    if (syncInFlightRef.current) {
      log.warn('Account sync already in flight; skipping duplicate start');
      return;
    }
    syncInFlightRef.current = true;
    ensureAccountSyncProgressListener();
    setSyncing(isFirstLogin);
    const operationId = useAccountSyncStore.getState().operationId;
    const isCurrentOperation = () => (
      useAccountSyncStore.getState().operationId === operationId
    );
    info(t('accountLogin.syncStarted'));

    void (async () => {
      try {
        let configJson = '{}';
        if (isFirstLogin) {
          useAccountSyncStore.getState().applyProgress({
            operation_id: operationId,
            phase: 'uploading_settings',
            percent: 2,
          });
          try {
            const exported = await configAPI.exportConfig();
            configJson = JSON.stringify(exported);
          } catch (e) {
            log.warn('export config failed', e);
          }
          if (!isCurrentOperation()) return;
        }
        const wp = workspacePath || '/';
        // AccountClient owns transient Relay retries with one shared deadline.
        // Replaying this entire workflow would also repeat deterministic local
        // config/filesystem work and multiply the transport retry budget.
        const result = await remoteConnectAPI.accountAutoSync(
          isFirstLogin,
          wp,
          configJson,
          operationId,
        );
        if (!isCurrentOperation()) return;
        log.info(
          `Auto-sync done: settings=${result.settings_synced} exported=${result.sessions_exported}`,
        );
        if (result.settings_synced && !isFirstLogin) {
          if (!isCurrentOperation()) return;
          try {
            await configAPI.reloadConfig();
            if (!isCurrentOperation()) return;
            configManager.clearCache();
            success(t('accountLogin.settingsApplied'));
          } catch (e) {
            log.warn('reloadConfig after sync failed', e);
          }
        }
        if (!isCurrentOperation()) return;
        setSyncDone(result);
        success(t('accountLogin.syncDone', {
          exported: result.sessions_exported,
        }));
      } catch (e) {
        if (!isCurrentOperation()) return;
        log.error('Auto-sync failed', e);
        setSyncFailed(e instanceof Error ? e.message : String(e));
        warning(syncFailureMessage(t, e));
      } finally {
        if (isCurrentOperation()) {
          syncInFlightRef.current = false;
        }
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

  const handleRetrySync = useCallback(() => {
    if (syncStatus !== 'failed' || syncInFlightRef.current) return;
    startBackgroundSync(lastSyncIsFirstLogin ?? false);
  }, [lastSyncIsFirstLogin, startBackgroundSync, syncStatus]);

  /** Landing path after a completed login: devices view + background sync. */
  const completeLogin = useCallback((
    relayUrl: string,
    isFirstLogin: boolean,
    accountEpoch: number,
  ) => {
    if (!isAccountEpochCurrent(accountEpoch)) return;
    setActiveAccountEpoch(accountEpoch);
    setAccountRelayUrl(relayUrl);
    setView('devices');
    void initializeDevices();
    startBackgroundSync(isFirstLogin);
  }, [initializeDevices, isAccountEpochCurrent, startBackgroundSync]);

  const performLogin = useCallback(async (server: string, user: string, pass: string) => {
    const epoch = invalidateAccountRequests();
    // Invalidate detached sync retries before the backend begins replacing the
    // account. The store operation id fences any completion from the old run.
    syncInFlightRef.current = false;
    clearSync();
    setLoading(true); setError(null);
    try {
      const stalePendingLoginId = pendingLoginIdRef.current;
      if (stalePendingLoginId) {
        await cancelPendingLoginWithRetry(stalePendingLoginId);
        if (pendingLoginIdRef.current === stalePendingLoginId) {
          pendingLoginIdRef.current = null;
        }
        if (!isAccountEpochCurrent(epoch)) return;
      }
      const result = await remoteConnectAPI.accountLogin(server, user, pass);
      if (!isAccountEpochCurrent(epoch)) {
        if (result.pending_login_id) {
          await cancelPendingLoginWithRetry(result.pending_login_id);
        }
        return;
      }
      if (result.has_cloud_settings) {
        if (!result.pending_login_id) {
          throw new Error(t('accountLogin.sessionExpired'));
        }
        pendingLoginIdRef.current = result.pending_login_id;
        setView('overwrite');
        setLoading(false);
        return;
      }
      success(t('accountLogin.loginSuccess', { user_id: user }));
      completeLogin(server, true, epoch);
    } catch (e: unknown) {
      if (!isAccountEpochCurrent(epoch)) return;
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      // The account session has its own token after this call; retaining the
      // password in React state while the device list is open is unnecessary.
      if (isAccountEpochCurrent(epoch)) {
        setPassword('');
        setLoading(false);
      }
    }
  }, [clearSync, completeLogin, invalidateAccountRequests, isAccountEpochCurrent, success, t]);

  const handleLogin = useCallback(async () => {
    if (!validate()) return;
    const relayUrl = parseRelayServer(authServer);
    if (!relayUrl) return;
    const isLoopback = ['localhost', '127.0.0.1', '[::1]', '::1'].includes(relayUrl.hostname);
    if (relayUrl.protocol === 'http:' && !isLoopback) {
      const confirmed = await confirmWarning(
        t('accountLogin.insecureServerTitle'),
        t('accountLogin.insecureServerConfirm'),
        {
          confirmText: t('accountLogin.continueInsecure'),
          cancelText: t('accountLogin.cancel'),
        },
      );
      if (!confirmed) return;
    }
    await performLogin(authServer.trim(), username.trim(), password);
  }, [validate, authServer, username, password, performLogin, t]);

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

  const finalizeAndSync = useCallback(async (isFirstLogin: boolean) => {
    const epoch = accountEpochRef.current;
    const pendingLoginId = pendingLoginIdRef.current;
    if (!pendingLoginId) {
      setError(t('accountLogin.sessionExpired'));
      return;
    }
    setLoading(true);
    setError(null);
    try {
      // The backend records the exact pending owner after commit, so retrying
      // the same opaque owner remains fenced from a replacement account.
      await finalizePendingLoginWithRetry(pendingLoginId);
      if (!isAccountEpochCurrent(epoch)) return;
      if (pendingLoginIdRef.current === pendingLoginId) {
        pendingLoginIdRef.current = null;
      }
      success(t('accountLogin.loginSuccess', { user_id: username }));
      completeLogin(authServer.trim(), isFirstLogin, epoch);
    } catch (e: unknown) {
      if (!isAccountEpochCurrent(epoch)) return;
      if (isAccountAuthFailure(e)) {
        await handleSessionExpired(e, epoch);
        return;
      }
      setError(e instanceof Error ? e.message : String(e));
      // Stop any detached work before accountLogout can yield.
      syncInFlightRef.current = false;
      clearSync();
      const cleanupEpoch = invalidateAccountRequests();
      try {
        await cancelPendingLoginWithRetry(pendingLoginId);
        if (pendingLoginIdRef.current === pendingLoginId) {
          pendingLoginIdRef.current = null;
        }
      } catch (cancelErr) {
        log.warn('pending login cancel after finalize failure failed', cancelErr);
        if (isAccountEpochCurrent(cleanupEpoch)) setLoading(false);
        return;
      }
      if (!isAccountEpochCurrent(cleanupEpoch)) return;
      resetState();
      setView('login');
      setLoading(false);
    } finally {
      if (isAccountEpochCurrent(epoch)) setLoading(false);
    }
  }, [authServer, clearSync, completeLogin, handleSessionExpired, invalidateAccountRequests, isAccountEpochCurrent, resetState, success, t, username]);

  const handleConfirmOverwrite = useCallback(() => {
    void finalizeAndSync(false);
  }, [finalizeAndSync]);

  const handleUseLocalOverwrite = useCallback(() => {
    void finalizeAndSync(true);
  }, [finalizeAndSync]);

  const handleCancelOverwrite = useCallback(async () => {
    const epoch = invalidateAccountRequests();
    syncInFlightRef.current = false;
    clearSync();
    const pendingLoginId = pendingLoginIdRef.current;
    if (pendingLoginId) {
      try {
        await cancelPendingLoginWithRetry(pendingLoginId);
        if (pendingLoginIdRef.current === pendingLoginId) {
          pendingLoginIdRef.current = null;
        }
      } catch (e) {
        log.warn('pending login cancel failed', e);
        if (isAccountEpochCurrent(epoch)) {
          setError(e instanceof Error ? e.message : String(e));
        }
        return;
      }
    }
    if (!isAccountEpochCurrent(epoch)) return;
    resetState();
    setView('login');
  }, [clearSync, invalidateAccountRequests, isAccountEpochCurrent, resetState]);

  const handleLogout = useCallback(async () => {
    const epoch = invalidateAccountRequests();
    setLoading(true);
    syncInFlightRef.current = false;
    clearSync();
    pendingLoginIdRef.current = null;
    try {
      await remoteConnectAPI.accountLogout();
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
  }, [clearSync, invalidateAccountRequests, isAccountEpochCurrent, resetState]);

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
    const previousSyncStatus = syncStatus;
    const previousSyncDirection = lastSyncIsFirstLogin;
    setLoading(true);
    setError(null);
    const epoch = isLocal ? invalidateAccountRequests() : accountEpochRef.current;
    if (isLocal) {
      // A current-device removal is also a logout. Invalidate retries and
      // late progress before the backend request yields.
      syncInFlightRef.current = false;
      clearSync();
    }
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
        if (
          isLocal
          && previousSyncDirection !== null
          && (previousSyncStatus === 'syncing' || previousSyncStatus === 'failed')
        ) {
          // Preserve the direction so Retry remains meaningful after a failed
          // current-device removal invalidated the previous generation.
          setSyncing(previousSyncDirection);
          setSyncFailed(message);
        }
      }
    } finally {
      if (isAccountEpochCurrent(epoch)) setLoading(false);
    }
  }, [
    clearSync,
    handleSessionExpired,
    invalidateAccountRequests,
    isAccountEpochCurrent,
    lastSyncIsFirstLogin,
    localDeviceId,
    refreshDevices,
    resetState,
    setSyncFailed,
    setSyncing,
    success,
    syncStatus,
    t,
  ]);

  const selectDevice = useCallback(async (device: AccountDeviceInfo) => {
    if (!device.online) return;
    // Picking this machine is a normal surface switch back, not a no-op: the
    // window may currently be rendering a peer.
    const isLocalDevice = Boolean(localDeviceId) && device.device_id === localDeviceId;
    if (!isLocalDevice) {
      if (syncStatus === 'failed') {
        warning(t('accountLogin.syncFailedPeerHint'));
      }
    }
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
    syncStatus,
    t,
    warning,
  ]);

  return (
    <>
      <div data-bf-component="remote-account-panel" data-bf-part="root" data-bf-view={view} className="account-panel">
        {error && (
          <div className="account-panel__error-banner" data-bf-component="remote-account-panel" data-bf-part="error">
            <Alert type="error" message={error} closable onClose={() => setError(null)}
              className="account-panel__error-alert" />
          </div>
        )}

        {loading && view === 'devices' && (
          <div className="account-panel__loading-overlay" data-bf-component="remote-account-panel" data-bf-part="loading">
            <RefreshCw size={20} className="spinning" />
            <span>{t('accountLogin.processing')}</span>
          </div>
        )}

        {view === 'login' && (
          <ScrollArea className="account-panel__scroll" data-bf-component="remote-account-panel" data-bf-part="scroll">
            <p className="account-panel__value-prop">{t('accountLogin.loginValueProp')}</p>
            <div className="account-panel__form" data-bf-component="remote-account-panel" data-bf-part="form">
              <Field
                className="account-panel__field"
                controlClassName="account-panel__field-control"
                label={t('accountLogin.username')}
              >
                <Input
                  className="account-panel__input"
                  disabled={loading}
                  leading={<Icon name="user" size="lg" />}
                  onValueChange={setUsername}
                  size="md"
                  type="text"
                  value={username}
                />
              </Field>
              <Field
                className="account-panel__field"
                controlClassName="account-panel__field-control"
                label={t('accountLogin.password')}
              >
                <Input
                  className="account-panel__input"
                  disabled={loading}
                  leading={<Lock />}
                  onValueChange={setPassword}
                  size="md"
                  trailing={
                    <IconButton
                      aria-label={showPassword
                        ? t('accountLogin.hidePassword')
                        : t('accountLogin.showPassword')}
                      icon={showPassword ? <EyeOff /> : <Icon name="eye" size="lg" />}
                      onClick={() => setShowPassword(s => !s)}
                      size="sm"
                      variant="quiet"
                    />
                  }
                  type={showPassword ? 'text' : 'password'}
                  value={password}
                />
              </Field>
              <Field
                className="account-panel__field"
                controlClassName="account-panel__field-control"
                label={t('accountLogin.authServer')}
              >
                <Input
                  className="account-panel__input"
                  disabled={loading}
                  leading={<Server />}
                  onValueChange={setAuthServer}
                  placeholder={t('accountLogin.authServerPlaceholder')}
                  size="md"
                  type="url"
                  value={authServer}
                />
              </Field>
              <p className="account-panel__security-note">{t('accountLogin.securityNote')}</p>
              <div className="account-panel__deploy-entry">
                <span>{t('relayDeploy.entryHint')}</span>
                <Button
                  variant="outline"
                  size="sm"
                  leadingIcon={<Rocket />}
                  onClick={() => setShowRelayDeploy(true)}
                  disabled={loading}
                >
                  {t('relayDeploy.entryAction')}
                </Button>
              </div>
            </div>
            <div className="account-panel__actions" data-bf-component="remote-account-panel" data-bf-part="actions">
              <Button
                variant="fill"
                size="sm"
                leadingIcon={<LogIn />}
                onClick={handleLogin}
                disabled={loading}
              >
                {loading ? t('accountLogin.processing') : t('accountLogin.login')}
              </Button>
            </div>
          </ScrollArea>
        )}

        {view === 'overwrite' && (
          <ScrollArea className="account-panel__scroll" data-bf-component="remote-account-panel" data-bf-part="scroll">
            <div className="account-panel__overwrite-notice">
              <CloudDownload size={32} />
              <p>{t('accountLogin.cloudOverwriteWarning')}</p>
            </div>
            <div className="account-panel__sync-options" data-bf-component="remote-account-panel" data-bf-part="syncOptions">
              <button
                className="account-panel__sync-option"
                data-bf-component="remote-account-panel"
                data-bf-part="syncOption"
                onClick={handleUseLocalOverwrite}
                disabled={loading}
              >
                <Icon name="upload" size="lg" />
                <div className="account-panel__sync-option-text">
                  <span className="account-panel__sync-option-title">{t('accountLogin.useLocalTitle')}</span>
                  <span className="account-panel__sync-option-desc">{t('accountLogin.useLocalDesc')}</span>
                </div>
              </button>
              <button
                className="account-panel__sync-option"
                data-bf-component="remote-account-panel"
                data-bf-part="syncOption"
                onClick={handleConfirmOverwrite}
                disabled={loading}
              >
                <CloudDownload size={20} />
                <div className="account-panel__sync-option-text">
                  <span className="account-panel__sync-option-title">{t('accountLogin.useCloudTitle')}</span>
                  <span className="account-panel__sync-option-desc">{t('accountLogin.useCloudDesc')}</span>
                </div>
              </button>
            </div>
            <div className="account-panel__actions" data-bf-component="remote-account-panel" data-bf-part="actions">
              <Button variant="outline" size="sm" onClick={handleCancelOverwrite} disabled={loading}>
                {t('accountLogin.disagree')}
              </Button>
            </div>
          </ScrollArea>
        )}

        {view === 'devices' && (
          <ScrollArea className="account-panel__scroll" data-bf-component="remote-account-panel" data-bf-part="scroll">
            {accountRelayUrl && (
              <div className="account-panel__server-line" data-bf-component="remote-account-panel" data-bf-part="server">
                <Server size={13} />
                <span className="account-panel__server-url" title={accountRelayUrl}>
                  {accountRelayUrl}
                </span>
                <IconButton
                  aria-label={t('accountLogin.copyServerUrl')}
                  icon={copiedServerUrl ? <Icon name="check-line" size="lg" /> : <Icon name="duplicate" size="lg" />}
                  onClick={handleCopyRelayUrl}
                  size="sm"
                  title={t('accountLogin.copyServerUrl')}
                  variant="quiet"
                />
              </div>
            )}
            {syncStatus !== 'idle' && !relayError && (
              <div className={`account-panel__sync-indicator ${syncStatus}`} data-bf-component="remote-account-panel" data-bf-part="syncStatus" data-bf-state={syncStatus === 'syncing' ? 'syncing' : undefined}>
                <div className="account-panel__sync-indicator-row">
                  {syncStatus === 'syncing' && <RefreshCw size={14} className="spinning" />}
                  {syncStatus === 'done' && <span>✓</span>}
                  {syncStatus === 'failed' && <span>⚠</span>}
                  <span className="account-panel__sync-indicator-text">
                    {syncStatus === 'syncing' && syncPhaseLabel(
                      t,
                      syncProgress.phase,
                      syncProgress.current,
                      syncProgress.total,
                    )}
                    {syncStatus === 'done' && t('accountLogin.syncDoneShort')}
                    {syncStatus === 'failed' && syncFailureMessage(t, lastSyncError)}
                  </span>
                  {syncStatus === 'failed' && (
                    <Button
                      variant="outline"
                      size="sm"
                      leadingIcon={<RefreshCw />}
                      className="account-panel__sync-retry"
                      onClick={handleRetrySync}
                      disabled={loading}
                    >
                      {t('accountLogin.retrySync')}
                    </Button>
                  )}
                  {syncStatus === 'syncing' && (
                    <span className="account-panel__sync-indicator-percent">
                      {t('accountLogin.syncProgressPercent', { percent: syncProgress.percent })}
                    </span>
                  )}
                </div>
                {syncStatus === 'syncing' && (
                  <div
                    className="account-panel__sync-progress-track"
                    data-bf-component="remote-account-panel"
                    data-bf-part="progressTrack"
                    role="progressbar"
                    aria-valuemin={0}
                    aria-valuemax={100}
                    aria-valuenow={syncProgress.percent}
                  >
                    <div
                      className="account-panel__sync-progress-fill"
                      data-bf-component="remote-account-panel"
                      data-bf-part="progressFill"
                      style={{ width: `${Math.max(2, syncProgress.percent)}%` }}
                    />
                  </div>
                )}
              </div>
            )}
            {relayError && (
              <div className="account-panel__error-banner" data-bf-component="remote-account-panel" data-bf-part="error">
                <Alert
                  type="error"
                  message={relayError}
                  className="account-panel__error-alert"
                />
              </div>
            )}
            <div className="account-panel__device-list" data-bf-component="remote-account-panel" data-bf-part="deviceList">
              {!relayError && devicesReady && devices.length === 0 && (
                <div className="account-panel__empty">{t('accountLogin.noDevices')}</div>
              )}
              {!relayError && !devicesReady && (
                <div className="account-panel__empty account-panel__empty--loading" role="status">
                  <RefreshCw size={14} className="spinning" />
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
                return (
                <div data-bf-component="remote-account-panel" data-bf-part="deviceCard" key={d.device_id}
                  data-bf-state={[
                    !d.online && 'offline',
                    isLocal && 'current',
                  ].filter(Boolean).join(' ') || undefined}
                  className={`account-panel__device-card ${isSelectable ? 'selectable' : ''} ${d.online ? '' : 'offline'} ${isLocal ? 'current' : ''}`}>
                  <button
                    type="button"
                    className="account-panel__device-select"
                    onClick={() => void selectDevice(d)}
                    disabled={!isSelectable || loading}
                    aria-label={isSelectable
                      ? t('accountLogin.openDevice', { name: displayName })
                      : undefined}
                  >
                    <Monitor size={16} />
                    <span className="account-panel__device-info">
                      <span className="account-panel__device-name">
                        {displayName}
                        {isLocal && <span className="account-panel__device-badge">{t('accountLogin.thisDevice')}</span>}
                      </span>
                      <span className="account-panel__device-meta">
                        <span className="account-panel__device-id">
                          {d.device_id.slice(0, 8)}
                        </span>
                        <span className="account-panel__device-status">
                          {' · '}
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
                  </button>
                  <IconButton
                    aria-label={`${removeLabel}: ${displayName}`}
                    disabled={loading}
                    icon={<Icon name="xmark" size="lg" />}
                    onClick={(e) => { e.stopPropagation(); handleDeleteDevice(d.device_id, displayName); }}
                    size="sm"
                    tone="danger"
                    title={removeLabel}
                    variant="quiet"
                  />
                </div>
                );
              })}
            </div>
            <div className="account-panel__actions" data-bf-component="remote-account-panel" data-bf-part="actions">
              {relayError && (
                <Button
                  variant="fill"
                  size="sm"
                  leadingIcon={<RefreshCw />}
                  onClick={handleRetryConnect}
                  disabled={loading}
                >
                  {t('accountLogin.retryConnect')}
                </Button>
              )}
              {!relayError && (
                <Button
                  variant="outline"
                  size="sm"
                  leadingIcon={<RefreshCw />}
                  onClick={refreshDevices}
                  disabled={loading}
                >
                  {t('accountLogin.refreshDevices')}
                </Button>
              )}
              <Button variant="outline" size="sm" onClick={handleLogout} disabled={loading}>
                {t('accountLogin.logout')}
              </Button>
            </div>
          </ScrollArea>
        )}
      </div>

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

export default AccountPanel;
