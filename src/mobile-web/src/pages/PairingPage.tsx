import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import LanguageToggleButton from '../components/LanguageToggleButton';
import { useI18n } from '../i18n';
import { RelayHttpClient } from '../services/RelayHttpClient';
import { RemoteSessionManager } from '../services/RemoteSessionManager';
import { useMobileStore } from '../services/store';
import { useTheme } from '../theme';
import logoIcon from '../assets/Logo-ICON.png';

interface PairingPageProps {
  onPaired: (client: RelayHttpClient, sessionMgr: RemoteSessionManager) => void;
}

const ThemeToggleIcon: React.FC<{ isDark: boolean }> = ({ isDark }) => (
  <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
    {isDark ? (
      <path d="M8 1.5a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13ZM3 8a5 5 0 0 1 5-5v10a5 5 0 0 1-5-5Z" fill="currentColor"/>
    ) : (
      <path d="M8 1a.5.5 0 0 1 .5.5v1a.5.5 0 0 1-1 0v-1A.5.5 0 0 1 8 1Zm0 11a.5.5 0 0 1 .5.5v1a.5.5 0 0 1-1 0v-1A.5.5 0 0 1 8 12Zm7-4a.5.5 0 0 1-.5.5h-1a.5.5 0 0 1 0-1h1A.5.5 0 0 1 15 8ZM3 8a.5.5 0 0 1-.5.5h-1a.5.5 0 0 1 0-1h1A.5.5 0 0 1 3 8Zm9.95-3.54a.5.5 0 0 1 0 .71l-.71.7a.5.5 0 1 1-.7-.7l.7-.71a.5.5 0 0 1 .71 0ZM5.46 11.24a.5.5 0 0 1 0 .71l-.7.71a.5.5 0 0 1-.71-.71l.7-.71a.5.5 0 0 1 .71 0Zm7.08 1.42a.5.5 0 0 1-.7 0l-.71-.71a.5.5 0 0 1 .7-.7l.71.7a.5.5 0 0 1 0 .71ZM5.46 4.76a.5.5 0 0 1-.71 0l-.71-.7a.5.5 0 0 1 .71-.71l.7.7a.5.5 0 0 1 0 .71ZM8 5a3 3 0 1 1 0 6 3 3 0 0 1 0-6Z" fill="currentColor"/>
    )}
  </svg>
);

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

function normalizeRelayUrl(value: string): string | null {
  try {
    const normalized = value
      .replace(/^wss:\/\//, 'https://')
      .replace(/^ws:\/\//, 'http://')
      .replace(/\/ws\/?$/, '')
      .replace(/\/$/, '');
    const url = new URL(normalized);
    if (!['http:', 'https:'].includes(url.protocol)
      || !url.hostname
      || url.username
      || url.password
      || url.search
      || url.hash) {
      return null;
    }
    return url.toString().replace(/\/$/, '');
  } catch {
    return null;
  }
}

function validPairingSecret(room: string | null, publicKey: string | null): boolean {
  return !!room
    && room.length <= 128
    && /^[A-Za-z0-9_-]+$/.test(room)
    && !['_store', 'page-data', 'pages'].includes(room)
    && !!publicKey
    && publicKey.length <= 512
    && /^[A-Za-z0-9+/=_-]+$/.test(publicKey);
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

function resolvePairingTarget(): {
  room: string | null;
  pk: string | null;
  httpBaseUrl: string;
  accountAuth: boolean;
  accountUsername: string | null;
} {
  const hash = window.location.hash;
  const params = new URLSearchParams(hash.replace(/^#\/pair\?/, ''));
  const room = params.get('room');
  const pk = params.get('pk');
  const relayParam = params.get('relay');
  const accountAuth = params.get('auth') === 'account';
  const accountUsername = params.get('user')?.trim() || null;

  if (relayParam) {
    return {
      room,
      pk,
      httpBaseUrl: normalizeRelayUrl(relayParam) ?? '',
      accountAuth,
      accountUsername,
    };
  }

  const origin = window.location.origin;
  const pathname = window.location.pathname
    .replace(/\/[^/]*$/, '')
    .replace(/\/r\/[^/]*$/, '');
  return {
    room,
    pk,
    httpBaseUrl: origin + pathname,
    accountAuth,
    accountUsername,
  };
}

const PairingPage: React.FC<PairingPageProps> = ({ onPaired }) => {
  const { t } = useI18n();
  const { isDark, toggleTheme } = useTheme();
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
  const [submitting, setSubmitting] = useState(false);
  const [failureCount, setFailureCount] = useState(0);
  const [lockUntil, setLockUntil] = useState<number | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const failureCountRef = useRef(0);
  const lockUntilRef = useRef<number | null>(null);
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
  const requiresAccountAuth = pairingTarget.accountAuth;
  const isLocked = !!lockUntil && lockUntil > now;
  const remainingLockSeconds = isLocked
    ? Math.max(1, Math.ceil((lockUntil - now) / 1000))
    : 0;

  const attemptPair = useCallback(async (
    providedUserId: string,
    providedPassword: string,
    options?: { autoReconnect?: boolean; installId?: string },
  ) => {
    const roomId = pairingTarget.room;
    const desktopPublicKey = pairingTarget.pk;
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

    if (!roomId
      || !desktopPublicKey
      || !validPairingSecret(roomId, desktopPublicKey)
      || !pairingTarget.httpBaseUrl) {
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

    const client = new RelayHttpClient(pairingTarget.httpBaseUrl, roomId);

    try {
      const initialSync = await client.pair(desktopPublicKey, {
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
      const errorMessage = rawErrorMessage.includes('timed out')
        ? t('pairing.requestTimedOut')
        : rawErrorMessage.includes('HTTP 404')
          ? t('pairing.qrExpired')
          : rawErrorMessage.includes('HTTP 429')
            ? t('pairing.rateLimited')
            : rawErrorMessage.includes('HTTP 503') || rawErrorMessage.includes('HTTP 504')
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
    pairingTarget.httpBaseUrl,
    pairingTarget.pk,
    pairingTarget.room,
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
    await attemptPair(userId, password, { autoReconnect: false });
  };

  const stateLabels: Record<string, string> = {
    idle: requiresAccountAuth
      ? t('pairing.enterAccountToContinue')
      : t('pairing.enterUserIdToContinue'),
    pairing: t('pairing.connectingAndPairing'),
    paired: t('pairing.pairedLoadingSessions'),
    error: t('pairing.connectionError'),
  };
  const showSpinner = connectionStatus === 'pairing';
  const showForm = connectionStatus === 'idle' || connectionStatus === 'error';

  return (
    <div className="pairing-page">
      <div className="pairing-page__actions">
        <LanguageToggleButton />
        <button
          className="pairing-page__theme-btn"
          onClick={toggleTheme}
          aria-label={t('common.toggleTheme')}
        >
          <ThemeToggleIcon isDark={isDark} />
        </button>
      </div>
      <img src={logoIcon} alt="BitFun" className="pairing-page__logo" />
      <div className="pairing-page__brand">{t('shared.product.remote')}</div>

      <div className="pairing-page__spinner-wrap">
        {showSpinner && <div className="spinner" />}
      </div>

      <div className="pairing-page__state">
        {stateLabels[connectionStatus] || connectionStatus}
      </div>

      {showForm && (
        <div className="pairing-page__form">
          <label className="pairing-page__field">
            <span className="pairing-page__field-label">
              {requiresAccountAuth ? t('pairing.usernameLabel') : t('pairing.fieldLabel')}
            </span>
            <input
              className="pairing-page__input"
              type="text"
              value={userId}
              onChange={(e) => setUserId(e.target.value)}
              placeholder={
                requiresAccountAuth
                  ? t('pairing.usernamePlaceholder')
                  : t('pairing.placeholder')
              }
              autoCapitalize="off"
              autoCorrect="off"
              autoComplete="username"
              maxLength={128}
              disabled={submitting || isLocked}
            />
          </label>
          {requiresAccountAuth && (
            <label className="pairing-page__field">
              <span className="pairing-page__field-label">{t('pairing.passwordLabel')}</span>
              <input
                className="pairing-page__input"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder={t('pairing.passwordPlaceholder')}
                autoComplete="current-password"
                maxLength={1024}
                disabled={submitting || isLocked}
              />
            </label>
          )}
          <p className="pairing-page__note">
            {requiresAccountAuth ? t('pairing.accountNote') : t('pairing.note')}
          </p>
          <button
            className="pairing-page__retry"
            onClick={handleConnect}
            disabled={submitting || isLocked}
          >
            {submitting
              ? t('pairing.connecting')
              : isLocked
                ? t('pairing.retryIn', { seconds: remainingLockSeconds })
                : t('pairing.continue')}
          </button>
        </div>
      )}

      {error && <div className="pairing-page__error">{error}</div>}
    </div>
  );
};

export default PairingPage;
