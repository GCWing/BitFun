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

import React, { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { Button, Input, Alert } from '@/component-library';
import {
  confirmDanger,
  confirmWarning,
} from '@/component-library/components/ConfirmDialog/confirmService';
import {
  User, Lock, Server, LogIn, Monitor, CloudDownload, Upload,
  ChevronRight, RefreshCw, Eye, EyeOff, X, Rocket, Copy, Check,
  PanelsTopLeft,
} from 'lucide-react';
import { useSceneStore } from '@/app/stores/sceneStore';
import { remoteConnectAPI } from '@/infrastructure/api/service-api/RemoteConnectAPI';
import type { AccountHint, AccountDeviceInfo } from '@/infrastructure/api/service-api/RemoteConnectAPI';
import { RelayDeployWizard } from '@/features/relay-deploy';
import type { RelayDeployResult } from '@/features/relay-deploy';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import { usePeerDeviceMode } from '@/infrastructure/peer-device/peerDeviceContextState';
import { useAccountSyncStore, ensureAccountSyncProgressListener } from '@/infrastructure/account/accountSyncStore';
import type { AccountSyncPhase } from '@/infrastructure/account/accountSyncStore';
import { isAccountAuthFailure } from '@/infrastructure/account/accountErrorUtils';
import { useNotification } from '@/shared/notification-system';
import { copyTextToClipboard } from '@/shared/utils/textSelection';
import { createLogger } from '@/shared/utils/logger';
import './AccountPanel.scss';

const log = createLogger('AccountPanel');

const DEVICE_POLL_FALLBACK_MS = 30_000;

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
  try {
    return await remoteConnectAPI.accountCancelPendingLogin(pendingLoginId);
  } catch (firstError) {
    log.warn('pending login cancel response was ambiguous; retrying', firstError);
    return await remoteConnectAPI.accountCancelPendingLogin(pendingLoginId);
  }
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
  const { enterPeerMode } = usePeerDeviceMode();
  const openScene = useSceneStore((s) => s.openScene);
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
  /** Only true after a successful list_devices response; gates the empty-state copy. */
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
    const requestId = ++refreshRequestRef.current;
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
    } catch (e) {
      if (!isCurrent()) return;
      log.warn('refreshDevices failed', e);
      if (isAccountAuthFailure(e)) {
        await handleSessionExpired(e, epoch);
      } else {
        markRelayUnreachable();
      }
    }
  }, [localDeviceId, handleSessionExpired, isAccountEpochCurrent, markRelayUnreachable]);

  const handleRetryConnect = useCallback(async () => {
    const epoch = accountEpochRef.current;
    setLoading(true);
    setRelayError(null);
    try {
      await remoteConnectAPI.accountConnectDevices();
      if (!isAccountEpochCurrent(epoch)) return;
      await refreshDevices();
    } catch (err) {
      log.warn('retry connect failed', err);
      if (!isAccountEpochCurrent(epoch)) return;
      if (isAccountAuthFailure(err)) {
        await handleSessionExpired(err, epoch);
        return;
      }
      markRelayUnreachable();
    } finally {
      if (isAccountEpochCurrent(epoch)) setLoading(false);
    }
  }, [handleSessionExpired, isAccountEpochCurrent, markRelayUnreachable, refreshDevices]);

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

  /** Connect presence + load the device list for an active account session. */
  const initializeDevices = useCallback(async () => {
    const epoch = accountEpochRef.current;
    try {
      await remoteConnectAPI.accountConnectDevices();
      if (!isAccountEpochCurrent(epoch)) return;
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
    }
    if (!isAccountEpochCurrent(epoch)) return;
    void refreshDevices();
    startDevicePolling();
  }, [handleSessionExpired, isAccountEpochCurrent, markRelayUnreachable, refreshDevices, startDevicePolling]);

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

    // Connect device presence immediately so the device list can populate
    // while the heavier settings/session sync continues.
    void remoteConnectAPI.accountConnectDevices().catch((err) => {
      log.warn('accountConnectDevices failed at sync start', err);
    });

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
        const maxAttempts = 3;
        let result: Awaited<ReturnType<typeof remoteConnectAPI.accountAutoSync>> | null = null;
        let lastError: unknown = null;
        for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
          if (!isCurrentOperation()) return;
          try {
            result = await remoteConnectAPI.accountAutoSync(
              isFirstLogin,
              wp,
              configJson,
              operationId,
            );
            if (!isCurrentOperation()) return;
            lastError = null;
            break;
          } catch (e) {
            if (!isCurrentOperation()) return;
            lastError = e;
            log.warn(`Auto-sync attempt ${attempt}/${maxAttempts} failed`, e);
            if (isNonRetryableSyncError(e)) {
              break;
            }
            if (attempt < maxAttempts) {
              info(t('accountLogin.syncRetrying', { attempt, max: maxAttempts }));
              await new Promise((resolve) => setTimeout(resolve, 2000 * attempt));
              if (!isCurrentOperation()) return;
            }
          }
        }
        if (!result) {
          throw lastError instanceof Error
            ? lastError
            : new Error(String(lastError ?? 'auto-sync failed'));
        }
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
      try {
        await remoteConnectAPI.accountFinalizeLogin(pendingLoginId);
      } catch (firstError) {
        if (!isAccountEpochCurrent(epoch)) return;
        // The backend commit may have succeeded even when its transport
        // response was lost. Retrying the same opaque owner is idempotent and
        // cannot authorize a replacement account generation.
        log.warn('pending login finalize response was ambiguous; retrying', firstError);
        await remoteConnectAPI.accountFinalizeLogin(pendingLoginId);
      }
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

  const handleOpenPages = useCallback(() => {
    onCloseDialog();
    openScene('pages');
  }, [onCloseDialog, openScene]);

  const selectDevice = useCallback(async (device: AccountDeviceInfo) => {
    if (!device.online) return;
    if (localDeviceId && device.device_id === localDeviceId) return;
    if (syncStatus === 'syncing') {
      info(t('accountLogin.syncInProgressHint'));
      return;
    }
    if (syncStatus === 'failed') {
      warning(t('accountLogin.syncFailedPeerHint'));
    }
    setLoading(true);
    setError(null);
    try {
      await enterPeerMode(device.device_id, device.device_name);
      success(t('accountLogin.enteredPeerMode', { name: device.device_name }));
      onCloseDialog();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [enterPeerMode, info, localDeviceId, onCloseDialog, success, syncStatus, t, warning]);

  return (
    <>
      <div className="account-panel">
        {error && (
          <div className="account-panel__error-banner">
            <Alert type="error" message={error} closable onClose={() => setError(null)}
              className="account-panel__error-alert" />
          </div>
        )}

        {loading && view === 'devices' && (
          <div className="account-panel__loading-overlay">
            <RefreshCw size={20} className="spinning" />
            <span>{t('accountLogin.processing')}</span>
          </div>
        )}

        {view === 'login' && (
          <div className="account-panel__scroll">
            <p className="account-panel__value-prop">{t('accountLogin.loginValueProp')}</p>
            <div className="account-panel__form">
              <div className="account-panel__field">
                <Input label={t('accountLogin.username')} type="text" value={username}
                  onChange={(e) => setUsername(e.target.value)} prefix={<User size={16} />}
                  size="medium" disabled={loading} />
              </div>
              <div className="account-panel__field">
                <Input label={t('accountLogin.password')} type={showPassword ? 'text' : 'password'} value={password}
                  onChange={(e) => setPassword(e.target.value)} prefix={<Lock size={16} />}
                  size="medium" disabled={loading}
                  suffix={
                    <button
                      type="button"
                      className="bitfun-input-toggle"
                      onClick={() => setShowPassword(s => !s)}
                      aria-label={showPassword
                        ? t('accountLogin.hidePassword')
                        : t('accountLogin.showPassword')}
                    >
                      {showPassword ? <EyeOff size={16} /> : <Eye size={16} />}
                    </button>
                  } />
              </div>
              <div className="account-panel__field">
                <Input label={t('accountLogin.authServer')} type="url" value={authServer}
                  onChange={(e) => setAuthServer(e.target.value)}
                  placeholder={t('accountLogin.authServerPlaceholder')}
                  prefix={<Server size={16} />} size="medium" disabled={loading} />
              </div>
              <p className="account-panel__security-note">{t('accountLogin.securityNote')}</p>
              <div className="account-panel__deploy-entry">
                <span>{t('relayDeploy.entryHint')}</span>
                <button
                  type="button"
                  className="account-panel__deploy-link"
                  onClick={() => setShowRelayDeploy(true)}
                  disabled={loading}
                >
                  <Rocket size={13} />
                  {t('relayDeploy.entryAction')}
                </button>
              </div>
            </div>
            <div className="account-panel__actions">
              <Button variant="primary" size="small" onClick={handleLogin} disabled={loading}>
                <LogIn size={14} />
                {loading ? t('accountLogin.processing') : t('accountLogin.login')}
              </Button>
            </div>
          </div>
        )}

        {view === 'overwrite' && (
          <div className="account-panel__scroll">
            <div className="account-panel__overwrite-notice">
              <CloudDownload size={32} />
              <p>{t('accountLogin.cloudOverwriteWarning')}</p>
            </div>
            <div className="account-panel__sync-options">
              <button
                className="account-panel__sync-option"
                onClick={handleUseLocalOverwrite}
                disabled={loading}
              >
                <Upload size={20} />
                <div className="account-panel__sync-option-text">
                  <span className="account-panel__sync-option-title">{t('accountLogin.useLocalTitle')}</span>
                  <span className="account-panel__sync-option-desc">{t('accountLogin.useLocalDesc')}</span>
                </div>
              </button>
              <button
                className="account-panel__sync-option"
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
            <div className="account-panel__actions">
              <Button variant="secondary" size="small" onClick={handleCancelOverwrite} disabled={loading}>
                {t('accountLogin.disagree')}
              </Button>
            </div>
          </div>
        )}

        {view === 'devices' && (
          <div className="account-panel__scroll">
            {accountRelayUrl && (
              <div className="account-panel__server-line">
                <Server size={13} />
                <span className="account-panel__server-url" title={accountRelayUrl}>
                  {accountRelayUrl}
                </span>
                <button
                  type="button"
                  className="account-panel__copy-btn"
                  onClick={handleCopyRelayUrl}
                  title={t('accountLogin.copyServerUrl')}
                >
                  {copiedServerUrl ? <Check size={13} /> : <Copy size={13} />}
                </button>
              </div>
            )}
            {syncStatus !== 'idle' && !relayError && (
              <div className={`account-panel__sync-indicator ${syncStatus}`}>
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
                    <button
                      type="button"
                      className="account-panel__sync-retry"
                      onClick={handleRetrySync}
                      disabled={loading}
                    >
                      <RefreshCw size={12} />
                      {t('accountLogin.retrySync')}
                    </button>
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
                    role="progressbar"
                    aria-valuemin={0}
                    aria-valuemax={100}
                    aria-valuenow={syncProgress.percent}
                  >
                    <div
                      className="account-panel__sync-progress-fill"
                      style={{ width: `${Math.max(2, syncProgress.percent)}%` }}
                    />
                  </div>
                )}
              </div>
            )}
            {relayError && (
              <div className="account-panel__error-banner">
                <Alert
                  type="error"
                  message={relayError}
                  className="account-panel__error-alert"
                />
              </div>
            )}
            <div className="account-panel__device-list">
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
                const isSelectable = !isLocal && d.online && syncStatus !== 'syncing';
                const removeLabel = isLocal
                  ? t('accountLogin.removeCurrentDevice')
                  : t('accountLogin.removeDevice');
                const displayName = d.device_name || t('accountLogin.unknownDevice');
                return (
                <div key={d.device_id}
                  className={`account-panel__device-card ${isSelectable ? 'selectable' : ''} ${d.online ? '' : 'offline'} ${isLocal ? 'current' : ''} ${syncStatus === 'syncing' && !isLocal ? 'syncing' : ''}`}>
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
                    {isSelectable && <ChevronRight size={14} />}
                    {!isLocal && d.online && syncStatus === 'syncing' && (
                      <RefreshCw
                        size={14}
                        className="spinning"
                        aria-label={t('accountLogin.syncing')}
                      />
                    )}
                  </button>
                  <button type="button" className="account-panel__device-remove"
                    onClick={(e) => { e.stopPropagation(); handleDeleteDevice(d.device_id, displayName); }}
                    title={removeLabel}
                    aria-label={`${removeLabel}: ${displayName}`}
                    disabled={loading}>
                    <X size={14} aria-hidden="true" />
                  </button>
                </div>
                );
              })}
            </div>
            <div className="account-panel__pages-section">
              <h3 className="account-panel__pages-section-title">
                {t('accountLogin.pagesSectionTitle')}
              </h3>
              <button
                type="button"
                className="account-panel__pages-entry"
                onClick={handleOpenPages}
                aria-label={t('accountLogin.pagesEntryAria')}
                disabled={loading}
              >
                <span className="account-panel__pages-entry-icon" aria-hidden="true">
                  <PanelsTopLeft size={16} />
                </span>
                <span className="account-panel__pages-entry-text">
                  <span className="account-panel__pages-entry-title">
                    {t('accountLogin.pagesEntryTitle')}
                  </span>
                  <span className="account-panel__pages-entry-desc">
                    {t('accountLogin.pagesEntryDesc')}
                  </span>
                </span>
                <span className="account-panel__pages-entry-arrow" aria-hidden="true">
                  <ChevronRight size={15} />
                </span>
              </button>
            </div>
            <div className="account-panel__actions">
              {relayError && (
                <Button variant="primary" size="small" onClick={handleRetryConnect} disabled={loading}>
                  <RefreshCw size={14} />
                  {t('accountLogin.retryConnect')}
                </Button>
              )}
              {!relayError && (
                <Button variant="secondary" size="small" onClick={refreshDevices} disabled={loading}>
                  <RefreshCw size={14} />
                  {t('accountLogin.refreshDevices')}
                </Button>
              )}
              <Button variant="secondary" size="small" onClick={handleLogout} disabled={loading}>
                {t('accountLogin.logout')}
              </Button>
            </div>
          </div>
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
