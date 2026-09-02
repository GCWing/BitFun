import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import QrScannerSheet from '../components/QrScannerSheet';
import { useI18n } from '../i18n';
import { CloudAccountClient, CloudAccountRequestError } from '../services/CloudAccountClient';
import { normalizeRelayUrl, validPairingSecret } from '../services/pairingLink';
import { RelayHttpClient } from '../services/RelayHttpClient';
import { RemoteSessionManager } from '../services/RemoteSessionManager';
import { useMobileStore } from '../services/store';

interface PairingPageProps {
  onPaired: (client: RelayHttpClient, sessionMgr: RemoteSessionManager) => void;
}

const MOBILE_INSTALL_ID_KEY = 'bitfun.mobile.install_id';
const MOBILE_USER_ID_KEY = 'bitfun.mobile.user_id';
const MOBILE_LOCK_UNTIL_KEY = 'bitfun.mobile.user_id_lock_until';
const MOBILE_FAILURE_COUNT_KEY = 'bitfun.mobile.user_id_failure_count';
const MAX_FAILED_USER_ID_ATTEMPTS = 3;
const USER_ID_LOCKOUT_MS = 60_000;

function isProtectedUserIdError(message: string): boolean {
  return message.includes('This remote URL is already protected')
    || message.includes('This mobile device must continue using the previously confirmed user ID')
    || message.includes('Invalid username or password')
    || message.includes('Missing password')
    || message.includes('Missing username')
    || message.includes('Too many pairing attempts');
}

function generateInstallId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  return `mobile-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
}

function getOrCreateInstallId(): string {
  const existing = localStorage.getItem(MOBILE_INSTALL_ID_KEY)?.trim();
  if (existing) return existing;
  const created = generateInstallId();
  localStorage.setItem(MOBILE_INSTALL_ID_KEY, created);
  return created;
}

function currentPairingRouteKey(): string {
  return `${window.location.pathname}${window.location.hash}`;
}

function resolvePairingTarget(): {
  room: string | null;
  pk: string | null;
  httpBaseUrl: string;
  accountAuth: boolean;
  accountUsername: string | null;
  targetDeviceId: string | null;
  targetDeviceName: string | null;
  directAccountLogin: boolean;
  hasPairingDescriptor: boolean;
} {
  const hash = window.location.hash;
  const params = new URLSearchParams(hash.replace(/^#\/pair\?/, ''));
  const room = params.get('room');
  const pk = params.get('pk');
  const relayParam = params.get('relay');
  const authMode = params.get('auth');
  const isPairingRoute = hash === '#/pair' || hash.startsWith('#/pair?');
  // A direct visit is the account-facing product entry, so it must expose the
  // same username/password form as the native mobile app. QR links from older
  // Desktop builds remain legacy-compatible when they omit `auth`; they can
  // also opt in explicitly with `auth=legacy`.
  const accountAuth = authMode === 'account' || (!isPairingRoute && authMode !== 'legacy');
  const accountUsername = params.get('user')?.trim() || null;
  const targetDeviceId = params.get('did')?.trim() || null;
  const targetDeviceName = params.get('dn')?.trim() || null;
  const directAccountLogin = accountAuth && !isPairingRoute;

  if (relayParam) {
    const httpBaseUrl = normalizeRelayUrl(relayParam) ?? '';
    return {
      room,
      pk,
      httpBaseUrl,
      accountAuth,
      accountUsername,
      targetDeviceId,
      targetDeviceName,
      directAccountLogin,
      hasPairingDescriptor: validPairingSecret(room, pk) && !!httpBaseUrl,
    };
  }

  const origin = window.location.origin;
  const pathname = window.location.pathname
    .replace(/\/[^/]*$/, '')
    .replace(/\/r\/[^/]*$/, '');
  const httpBaseUrl = directAccountLogin ? `${origin}/relay` : origin + pathname;
  return {
    room,
    pk,
    httpBaseUrl,
    accountAuth,
    accountUsername,
    targetDeviceId,
    targetDeviceName,
    directAccountLogin,
    hasPairingDescriptor: validPairingSecret(room, pk) && !!normalizeRelayUrl(httpBaseUrl),
  };
}

const PairingPageContent: React.FC<PairingPageProps> = ({ onPaired }) => {
  const { t } = useI18n();
  const {
    connectionStatus,
    setConnectionStatus,
    setError,
    error,
    setAuthenticatedUserId,
    setAuthenticatedUserLabel,
  } = useMobileStore();
  const [userId, setUserId] = useState('');
  const [password, setPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [scannerOpen, setScannerOpen] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [failureCount, setFailureCount] = useState(0);
  const [lockUntil, setLockUntil] = useState<number | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const failureCountRef = useRef(0);
  const lockUntilRef = useRef<number | null>(null);
  const usernameInputRef = useRef<HTMLInputElement>(null);
  const passwordInputRef = useRef<HTMLInputElement>(null);
  // Generation token so a superseded or unmounted pairing attempt cannot
  // overwrite UI after a later bootstrap/manual attempt owns the page.
  const pairAttemptGenerationRef = useRef(0);
  const attemptPairRef = useRef<(
    providedUserId: string,
    providedPassword: string,
    options?: { autoReconnect?: boolean; installId?: string },
  ) => Promise<void>>(async () => {});
  const onPairedRef = useRef(onPaired);
  onPairedRef.current = onPaired;

  const pairingTarget = useMemo(() => resolvePairingTarget(), []);
  const [relayUrl, setRelayUrl] = useState(pairingTarget.httpBaseUrl);
  const requiresAccountAuth = pairingTarget.accountAuth;
  const isLocked = !!lockUntil && lockUntil > now;
  const remainingLockSeconds = isLocked
    ? Math.max(1, Math.ceil((lockUntil - now) / 1000))
    : 0;

  useEffect(() => {
    // Password managers can restore values without dispatching React change
    // events. Reconcile the visible controls so enabled/disabled state stays
    // identical to the native login page.
    const reconcileAutofill = () => {
      const restoredUsername = usernameInputRef.current?.value ?? '';
      const restoredPassword = passwordInputRef.current?.value ?? '';
      if (restoredUsername && !userId) setUserId(restoredUsername);
      if (restoredPassword && !password) setPassword(restoredPassword);
    };
    const frame = window.requestAnimationFrame(reconcileAutofill);
    const timer = window.setTimeout(reconcileAutofill, 250);
    return () => {
      window.cancelAnimationFrame(frame);
      window.clearTimeout(timer);
    };
  }, [password, userId]);

  const attemptPair = useCallback(async (
    providedUserId: string,
    providedPassword: string,
    options?: { autoReconnect?: boolean; installId?: string },
  ) => {
    const roomId = pairingTarget.room;
    const desktopPublicKey = pairingTarget.pk;
    const httpBaseUrl = normalizeRelayUrl(relayUrl) ?? '';
    const userIdValue = providedUserId.trim();
    // Passwords are opaque credentials: preserve intentional leading or
    // trailing spaces exactly as entered.
    const passwordValue = providedPassword;
    const autoReconnect = options?.autoReconnect === true;
    // Prefer the explicit installId from the caller; fall back to the stable
    // localStorage-backed id. Do not close over React state here — that used
    // to recreate this callback and re-trigger bootstrap side effects.
    const currentInstallId = options?.installId || getOrCreateInstallId();
    const activeLockUntil = lockUntilRef.current;
    const lockActive = !!activeLockUntil && activeLockUntil > Date.now();
    const currentRemainingLockSeconds = lockActive
      ? Math.max(1, Math.ceil((activeLockUntil - Date.now()) / 1000))
      : 0;
    const attemptGeneration = ++pairAttemptGenerationRef.current;
    const isCurrentAttempt = () => pairAttemptGenerationRef.current === attemptGeneration;

    if (!pairingTarget.directAccountLogin && (!roomId
      || !desktopPublicKey
      || !validPairingSecret(roomId, desktopPublicKey)
      || !httpBaseUrl)) {
      if (!isCurrentAttempt()) return;
      setError(t('pairing.invalidQrCode'));
      setConnectionStatus('error');
      return;
    }
    if (!userIdValue) {
      if (!isCurrentAttempt()) return;
      setError(requiresAccountAuth ? t('pairing.usernameRequired') : t('pairing.userIdRequired'));
      setConnectionStatus('error');
      return;
    }
    if (userIdValue.length > 128 || passwordValue.length > 1024) {
      if (!isCurrentAttempt()) return;
      setError(t('pairing.fieldsTooLong'));
      setConnectionStatus('error');
      return;
    }
    if (requiresAccountAuth && !passwordValue) {
      if (!isCurrentAttempt()) return;
      setError(t('pairing.passwordRequired'));
      setConnectionStatus('error');
      return;
    }
    if (!autoReconnect && lockActive) {
      if (!isCurrentAttempt()) return;
      setError(t('pairing.tooManyAttempts', { seconds: currentRemainingLockSeconds }));
      setConnectionStatus('error');
      return;
    }

    setSubmitting(true);
    setError(null);
    setConnectionStatus('pairing');

    const client = new RelayHttpClient(httpBaseUrl, roomId ?? '');

    try {
      // HarmonyOS treats an account-auth QR as an account-device selection:
      // once the account proof exists, `did` identifies the exact desktop and
      // the room is no longer the data plane. Direct account login uses the
      // same route but falls back to the first available desktop.
      if (requiresAccountAuth
        && (pairingTarget.directAccountLogin || !!pairingTarget.targetDeviceId)) {
        const accountSession = await new CloudAccountClient().login(
          httpBaseUrl,
          userIdValue,
          passwordValue,
          currentInstallId,
        );
        if (!isCurrentAttempt()) {
          accountSession.masterKey.fill(0);
          return;
        }
        client.installDirectAccountIdentity({
          ...accountSession,
          deviceId: currentInstallId,
        });
        accountSession.masterKey.fill(0);

        const devices = await client.listDevices();
        if (!isCurrentAttempt()) return;
        const remoteDevices = devices.filter((device) => (
          device.device_id !== client.controllerDeviceId
        ));
        const targetDeviceId = pairingTarget.targetDeviceId?.trim() ?? '';
        const targetDevice = targetDeviceId
          ? remoteDevices.find((device) => device.device_id === targetDeviceId)
          : remoteDevices.find((device) => device.online) ?? remoteDevices[0];
        if (!targetDevice) {
          throw new Error(targetDeviceId
            ? t('devices.deviceUnavailable')
            : t('devices.noDevices'));
        }
        if (!targetDevice.online) {
          throw new Error(t('devices.deviceUnavailable'));
        }
        client.setPairedDeviceId(targetDevice.device_id);

        const store = useMobileStore.getState();
        store.setAuthenticatedUserId(accountSession.userId);
        store.setAuthenticatedUserLabel(userIdValue);
        store.setControlTarget({
          deviceId: targetDevice.device_id,
          deviceName: targetDevice.device_name || pairingTarget.targetDeviceName || null,
          isHome: false,
        });
        setConnectionStatus('paired');
        localStorage.setItem(MOBILE_USER_ID_KEY, userIdValue);
        localStorage.removeItem(MOBILE_FAILURE_COUNT_KEY);
        localStorage.removeItem(MOBILE_LOCK_UNTIL_KEY);
        setFailureCount(0);
        setLockUntil(null);
        setPassword('');
        onPairedRef.current(client, new RemoteSessionManager(client));
        return;
      }

      const initialSync = await client.pair(desktopPublicKey!, {
        userId: userIdValue,
        mobileInstallId: currentInstallId,
        password: requiresAccountAuth ? passwordValue : undefined,
      });
      if (!isCurrentAttempt()) return;

      setConnectionStatus('paired');
      localStorage.setItem(MOBILE_USER_ID_KEY, userIdValue);
      localStorage.removeItem(MOBILE_FAILURE_COUNT_KEY);
      localStorage.removeItem(MOBILE_LOCK_UNTIL_KEY);
      setFailureCount(0);
      setLockUntil(null);
      setPassword('');
      // `authenticated_user_id` is the canonical account UUID used for
      // ownership checks. The submitted value is the verified username in
      // account mode and is the appropriate user-facing label.
      setAuthenticatedUserId(
        initialSync.authenticated_user_id
        ?? (requiresAccountAuth ? null : userIdValue),
      );
      setAuthenticatedUserLabel(userIdValue);

      const sessionMgr = new RemoteSessionManager(client);
      const store = useMobileStore.getState();
      if (initialSync.has_workspace) {
        if (initialSync.workspace_kind === 'assistant' && initialSync.path) {
          store.setPairedDisplayMode('assistant');
          store.setCurrentAssistant({
            path: initialSync.path,
            name: initialSync.project_name ?? 'Claw',
            assistant_id: initialSync.assistant_id,
          });
          store.setCurrentWorkspace(null);
        } else {
          store.setPairedDisplayMode('pro');
          store.setCurrentWorkspace({
            has_workspace: true,
            path: initialSync.path,
            project_name: initialSync.project_name,
            git_branch: initialSync.git_branch,
            workspace_kind: initialSync.workspace_kind,
            assistant_id: initialSync.assistant_id,
            remote_connection_id: initialSync.remote_connection_id,
            remote_ssh_host: initialSync.remote_ssh_host,
          });
        }
      }
      if (initialSync.sessions) {
        store.setSessions(initialSync.sessions);
      }

      // Inherit the desktop's logged-in account identity (best-effort).
      // When granted, the mobile can list and control same-account devices.
      // Soft timeout so a slow/unsupported desktop never blocks pairing;
      // DevicesPage retries identity acquisition on demand.
      try {
        const delegated = await Promise.race<boolean>([
          client.requestDelegatedIdentity(),
          new Promise<boolean>((resolve) => {
            window.setTimeout(() => resolve(false), 10_000);
          }),
        ]);
        if (!isCurrentAttempt()) return;
        const homeDeviceId = client.homeDeviceId;
        if (delegated && homeDeviceId) {
          store.setControlTarget({ deviceId: homeDeviceId, deviceName: null, isHome: true });
          const accountEpoch = client.delegatedAccountEpoch;
          const target = client.getControlTargetSnapshot();
          void client
            .listDevices()
            .then((devices) => {
              if (
                client.delegatedAccountEpoch !== accountEpoch
                || !client.isControlTargetCurrent(target)
                || client.pairedDeviceId !== homeDeviceId
              ) return;
              const home = devices.find((d) => d.device_id === homeDeviceId);
              if (home) {
                useMobileStore.getState().setControlTarget({
                  deviceId: homeDeviceId,
                  deviceName: home.device_name,
                  isHome: true,
                });
              }
            })
            .catch(() => {
              // Device name resolution is cosmetic; ignore failures.
            });
        }
      } catch {
        // Desktop without account login (or delegation failure) is a normal
        // single-device pairing; continue without device switching.
      }

      if (!isCurrentAttempt()) return;
      onPairedRef.current(client, sessionMgr);
    } catch (e: any) {
      if (!isCurrentAttempt()) return;
      const rawErrorMessage = e?.message || '';
      const status = e instanceof CloudAccountRequestError ? e.status : e?.status;
      const errorMessage = rawErrorMessage.includes('timed out')
        ? t('pairing.requestTimedOut')
        : status === 404 || rawErrorMessage.includes('HTTP 404')
          ? t('pairing.qrExpired')
          : status === 429 || rawErrorMessage.includes('HTTP 429')
            ? t('pairing.rateLimited')
            : status === 503 || status === 504
              || rawErrorMessage.includes('HTTP 503') || rawErrorMessage.includes('HTTP 504')
              ? t('pairing.relayUnavailable')
              : rawErrorMessage || t('pairing.pairingFailed');
      if (!autoReconnect && isProtectedUserIdError(errorMessage)) {
        const nextFailureCount = failureCountRef.current + 1;
        const shouldLock = nextFailureCount >= MAX_FAILED_USER_ID_ATTEMPTS;
        const nextLockUntil = shouldLock ? Date.now() + USER_ID_LOCKOUT_MS : null;
        localStorage.setItem(MOBILE_FAILURE_COUNT_KEY, String(nextFailureCount));
        if (nextLockUntil) {
          localStorage.setItem(MOBILE_LOCK_UNTIL_KEY, String(nextLockUntil));
        } else {
          localStorage.removeItem(MOBILE_LOCK_UNTIL_KEY);
        }
        setFailureCount(nextFailureCount);
        setLockUntil(nextLockUntil);
        setError(
          shouldLock
            ? t('pairing.tooManyAttempts', { seconds: Math.ceil(USER_ID_LOCKOUT_MS / 1000) })
            : rawErrorMessage.includes('Too many pairing attempts')
              ? t('pairing.rateLimited')
              : t('pairing.credentialsRejected'),
        );
      } else {
        setError(errorMessage);
      }
      setConnectionStatus('error');
    } finally {
      if (isCurrentAttempt()) {
        setSubmitting(false);
      }
    }
  }, [
    pairingTarget.directAccountLogin,
    pairingTarget.pk,
    pairingTarget.room,
    pairingTarget.targetDeviceId,
    pairingTarget.targetDeviceName,
    relayUrl,
    requiresAccountAuth,
    setAuthenticatedUserId,
    setAuthenticatedUserLabel,
    setConnectionStatus,
    setError,
    t,
  ]);

  attemptPairRef.current = attemptPair;

  // Mount-once bootstrap: restore form fields and optionally auto-reconnect.
  // Must NOT depend on `attemptPair` identity — a later callback recreation
  // used to reset status to `pairing` without starting a new request, which
  // left the page spinning forever after a fast reconnect failure.
  useEffect(() => {
    const savedUserId = localStorage.getItem(MOBILE_USER_ID_KEY)?.trim() ?? '';
    const qrUsername = pairingTarget.accountUsername?.trim() ?? '';
    const prefilledUserId = qrUsername || savedUserId;
    const currentInstallId = getOrCreateInstallId();
    const persistedFailureCount = Number(localStorage.getItem(MOBILE_FAILURE_COUNT_KEY) || '0');
    const persistedLockUntil = Number(localStorage.getItem(MOBILE_LOCK_UNTIL_KEY) || '0');
    const normalizedLockUntil = persistedLockUntil > Date.now() ? persistedLockUntil : null;
    if (persistedLockUntil && !normalizedLockUntil) {
      localStorage.removeItem(MOBILE_LOCK_UNTIL_KEY);
      localStorage.removeItem(MOBILE_FAILURE_COUNT_KEY);
    }
    // Account mode always needs a password — never auto-reconnect without it.
    const shouldAutoReconnect = !requiresAccountAuth
      && !!savedUserId
      && !!currentInstallId
      && !!pairingTarget.room
      && !!pairingTarget.pk;
    setUserId(prefilledUserId);
    setFailureCount(normalizedLockUntil ? persistedFailureCount : 0);
    setLockUntil(normalizedLockUntil);
    setError(null);

    if (shouldAutoReconnect) {
      // Show the spinner immediately; attemptPair also sets pairing when the
      // network attempt actually starts (after validation).
      setConnectionStatus('pairing');
      void attemptPairRef.current(savedUserId, '', {
        autoReconnect: true,
        installId: currentInstallId,
      });
    } else {
      setConnectionStatus('idle');
    }

    return () => {
      // Invalidate in-flight pairing so unmount / StrictMode remount cannot
      // apply stale success/error onto the next page instance.
      pairAttemptGenerationRef.current += 1;
    };
    // pairingTarget is resolved once from the URL hash on mount.
    // eslint-disable-next-line react-hooks/exhaustive-deps -- mount-once bootstrap
  }, []);

  useEffect(() => {
    failureCountRef.current = failureCount;
    lockUntilRef.current = lockUntil;
  }, [failureCount, lockUntil]);

  useEffect(() => {
    if (!lockUntil) return;
    if (lockUntil <= Date.now()) {
      setLockUntil(null);
      setFailureCount(0);
      localStorage.removeItem(MOBILE_LOCK_UNTIL_KEY);
      localStorage.removeItem(MOBILE_FAILURE_COUNT_KEY);
      return;
    }
    const timer = window.setInterval(() => {
      const currentNow = Date.now();
      setNow(currentNow);
      if (lockUntil <= currentNow) {
        setLockUntil(null);
        setFailureCount(0);
        localStorage.removeItem(MOBILE_LOCK_UNTIL_KEY);
        localStorage.removeItem(MOBILE_FAILURE_COUNT_KEY);
      }
    }, 1000);
    return () => window.clearInterval(timer);
  }, [lockUntil]);

  const handleConnect = async () => {
    await attemptPair(
      usernameInputRef.current?.value ?? userId,
      passwordInputRef.current?.value ?? password,
      { autoReconnect: false },
    );
  };

  const showSpinner = connectionStatus === 'pairing';
  const showForm = connectionStatus === 'idle' || connectionStatus === 'error';

  return (
    <div className="pairing-page">
      <div className="pairing-page__shell">
        <aside className="pairing-page__hero" aria-labelledby="pairing-desktop-title">
          <div className="pairing-page__hero-copy">
            <div className="pairing-page__eyebrow">{t('pairing.secureRemote')}</div>
            <h2 id="pairing-desktop-title">{t('pairing.heroTitle')}</h2>
            <p>{t('pairing.heroDescription')}</p>
          </div>
          <div className="pairing-page__connection-visual" aria-hidden="true">
            <div className="pairing-page__device pairing-page__device--desktop">
              <span className="pairing-page__device-screen" />
              <span className="pairing-page__device-base" />
            </div>
            <span className="pairing-page__connection-line"><i /><i /><i /></span>
            <div className="pairing-page__device pairing-page__device--phone">
              <span className="pairing-page__device-screen" />
            </div>
          </div>
          <div className="pairing-page__security-note">
            <span className="pairing-page__security-dot" />
            {t('pairing.encryptedConnection')}
          </div>
        </aside>
        <section className="pairing-page__panel">
          <header className="pairing-page__header">
            <span className="pairing-page__header-spacer" aria-hidden="true" />
            <button
              type="button"
              className="pairing-page__back"
              onClick={() => history.length > 1 && history.back()}
              aria-label={t('common.close')}
            >
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" aria-hidden="true">
                <path d="m6 6 12 12M18 6 6 18" />
              </svg>
            </button>
          </header>

          {showForm && (
            <form className="pairing-page__form" onSubmit={(event) => { event.preventDefault(); handleConnect(); }}>
              <div className="pairing-page__scroll">
                <div className="pairing-page__form-content">
                  <h1 className="pairing-page__title">
                    {requiresAccountAuth ? t('pairing.loginTitle') : t('pairing.connectTitle')}
                  </h1>
                  <p className="pairing-page__intro">
                    {requiresAccountAuth ? t('pairing.loginDescription') : t('pairing.note')}
                  </p>
                  <div className={`pairing-page__credentials${requiresAccountAuth ? '' : ' pairing-page__credentials--single'}`}>
                    <label className="pairing-page__field">
                      <span className="pairing-page__field-icon" aria-hidden="true">
                        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="8" r="4"/><path d="M4.5 21a7.5 7.5 0 0 1 15 0"/></svg>
                      </span>
                      <input
                        ref={usernameInputRef}
                        className="pairing-page__input pairing-page__input--username"
                        type="text"
                        value={userId}
                        onChange={(e) => setUserId(e.target.value)}
                        onAnimationStart={(e) => {
                          if (e.animationName === 'pairingAutofillReconcile') {
                            setUserId(e.currentTarget.value);
                          }
                        }}
                        placeholder={requiresAccountAuth ? t('pairing.usernamePlaceholder') : t('pairing.placeholder')}
                        autoCapitalize="off"
                        autoCorrect="off"
                        autoComplete="username"
                        maxLength={128}
                        disabled={submitting || isLocked}
                      />
                    </label>
                    {requiresAccountAuth && (
                      <div className="pairing-page__field pairing-page__password-field">
                      <label className="pairing-page__sr-only" htmlFor="pairing-password">
                        {t('pairing.passwordLabel')}
                      </label>
                      <span className="pairing-page__field-icon" aria-hidden="true">
                        <svg width="21" height="21" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><rect x="4" y="10" width="16" height="11" rx="2.5"/><path d="M8 10V7a4 4 0 0 1 8 0v3"/></svg>
                      </span>
                      <input
                        ref={passwordInputRef}
                        id="pairing-password"
                        className="pairing-page__input pairing-page__input--password"
                        type={showPassword ? 'text' : 'password'}
                        value={password}
                        onChange={(e) => setPassword(e.target.value)}
                        onAnimationStart={(e) => {
                          if (e.animationName === 'pairingAutofillReconcile') {
                            setPassword(e.currentTarget.value);
                          }
                        }}
                        placeholder={t('pairing.passwordPlaceholder')}
                        autoComplete="current-password"
                        maxLength={1024}
                        disabled={submitting || isLocked}
                      />
                      <button
                        type="button"
                        className="pairing-page__password-toggle"
                        aria-label={showPassword ? t('pairing.hidePassword') : t('pairing.showPassword')}
                        onClick={() => setShowPassword((visible) => !visible)}
                      >
                        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                          {showPassword ? (
                            <>
                              <path d="M2 12s3.5-6 10-6 10 6 10 6-3.5 6-10 6S2 12 2 12Z" />
                              <circle cx="12" cy="12" r="2.6" />
                            </>
                          ) : (
                            <>
                              <path d="m3 3 18 18" />
                              <path d="M10.6 6.2A10.7 10.7 0 0 1 12 6c6.5 0 10 6 10 6a17.8 17.8 0 0 1-2.2 2.8" />
                              <path d="M6.2 6.2C3.5 8 2 12 2 12s3.5 6 10 6a10 10 0 0 0 4-.8" />
                            </>
                          )}
                        </svg>
                      </button>
                      </div>
                    )}
                  </div>
                  <details className="pairing-page__advanced">
                    <summary>
                      <span className="pairing-page__advanced-icon" aria-hidden="true">
                        <svg width="21" height="21" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06-2.83 2.83-.06-.06A1.7 1.7 0 0 0 15 19.4a1.7 1.7 0 0 0-1 .6 1.7 1.7 0 0 0-.4 1.1V21h-4v-.09A1.7 1.7 0 0 0 8.6 19.4a1.7 1.7 0 0 0-1.88.34l-.06.06-2.83-2.83.06-.06A1.7 1.7 0 0 0 4.2 15a1.7 1.7 0 0 0-.6-1 1.7 1.7 0 0 0-1.1-.4H2.4v-4h.09A1.7 1.7 0 0 0 4.2 8.6a1.7 1.7 0 0 0-.34-1.88l-.06-.06 2.83-2.83.06.06A1.7 1.7 0 0 0 8.6 4.2a1.7 1.7 0 0 0 1-.6 1.7 1.7 0 0 0 .4-1.1V2.4h4v.09A1.7 1.7 0 0 0 15 4.2a1.7 1.7 0 0 0 1.88-.34l.06-.06 2.83 2.83-.06.06A1.7 1.7 0 0 0 19.4 8.6a1.7 1.7 0 0 0 .6 1 1.7 1.7 0 0 0 1.1.4h.09v4h-.09a1.7 1.7 0 0 0-1.7 1z"/></svg>
                      </span>
                      <span>{t('pairing.advancedOptions')}</span>
                    </summary>
                    <div className="pairing-page__advanced-actions">
                      <label className="pairing-page__relay-field">
                        <span>{t('pairing.loginServer')}</span>
                        <input
                          type="url"
                          value={relayUrl}
                          placeholder={t('pairing.relayUrlPlaceholder')}
                          onChange={(event) => setRelayUrl(event.target.value)}
                          disabled={submitting || isLocked}
                        />
                      </label>
                    </div>
                  </details>
                  {error && <div className="pairing-page__error">{error}</div>}
                </div>
              </div>
              <div className="pairing-page__action">
                <button
                  className="pairing-page__retry"
                  type="submit"
                  disabled={submitting || isLocked}
                >
                  {showSpinner && <span className="spinner spinner--sm" aria-hidden="true" />}
                  {submitting
                    ? t('pairing.connecting')
                    : isLocked
                      ? t('pairing.retryIn', { seconds: remainingLockSeconds })
                      : requiresAccountAuth
                        ? t('pairing.loginAction')
                        : t('pairing.continue')}
                </button>
                {!pairingTarget.hasPairingDescriptor && (
                  <button
                    className="pairing-page__scan-action"
                    type="button"
                    onClick={() => setScannerOpen(true)}
                    disabled={submitting}
                  >
                    <svg width="21" height="21" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                      <path d="M3 8V5a2 2 0 0 1 2-2h3M16 3h3a2 2 0 0 1 2 2v3M21 16v3a2 2 0 0 1-2 2h-3M8 21H5a2 2 0 0 1-2-2v-3" />
                      <path d="M8 8h8v8H8z" />
                    </svg>
                    {t('pairing.scanAction')}
                  </button>
                )}
              </div>
            </form>
          )}

          {!showForm && (
            <div className="pairing-page__progress" role="status">
              <div className="spinner" />
              <span>{connectionStatus === 'paired' ? t('pairing.pairedLoadingSessions') : t('pairing.connectingAndPairing')}</span>
            </div>
          )}
        </section>
      </div>
      {scannerOpen && (
        <QrScannerSheet
          onClose={() => setScannerOpen(false)}
          onDetected={(url) => window.location.assign(url)}
        />
      )}
    </div>
  );
};

/**
 * A scanner result commonly changes only the hash on the current Mobile Web
 * document. Hash navigation does not remount React by itself, but pairing
 * bootstrap is intentionally mount-scoped so stale attempts cannot cross
 * targets. Key the content by the complete pairing route to give every
 * scanned descriptor a fresh, single-owner connection lifecycle.
 */
const PairingPage: React.FC<PairingPageProps> = (props) => {
  const [routeKey, setRouteKey] = useState(currentPairingRouteKey);

  useEffect(() => {
    const handleHashChange = () => setRouteKey(currentPairingRouteKey());
    window.addEventListener('hashchange', handleHashChange);
    return () => window.removeEventListener('hashchange', handleHashChange);
  }, []);

  return <PairingPageContent key={routeKey} {...props} />;
};

export default PairingPage;
