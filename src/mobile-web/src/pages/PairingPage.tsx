import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { MobileIconButton, MobileStatus } from '@openbitfun/ui/mobile';
import PairingForm from '../components/PairingForm';
import QrScannerSheet from '../components/QrScannerSheet';
import { useI18n } from '../i18n';
import { CloudAccountClient, CloudAccountRequestError } from '../services/CloudAccountClient';
import {
  loadMatchingCloudAccountSession,
  saveCloudAccountSession,
  type StoredCloudAccountSession,
} from '../services/CloudAccountSessionStore';
import { normalizeRelayUrl, validPairingSecret } from '../services/pairingLink';
import { RelayHttpClient } from '../services/RelayHttpClient';
import { RemoteSessionManager } from '../services/RemoteSessionManager';
import { useMobileStore } from '../services/store';

interface PairingPageProps {
  onPaired: (client: RelayHttpClient, sessionMgr: RemoteSessionManager, preferredDeviceId?: string) => void;
}

interface PairAttemptOptions {
  autoReconnect?: boolean;
  installId?: string;
  accountSession?: StoredCloudAccountSession;
}

const MOBILE_INSTALL_ID_KEY = 'openbitfun.mobile.install_id';
const MOBILE_USER_ID_KEY = 'openbitfun.mobile.user_id';
const MOBILE_LOCK_UNTIL_KEY = 'openbitfun.mobile.user_id_lock_until';
const MOBILE_FAILURE_COUNT_KEY = 'openbitfun.mobile.user_id_failure_count';
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
  const [advancedOpen, setAdvancedOpen] = useState(false);
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
    options?: PairAttemptOptions,
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
    options?: PairAttemptOptions,
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
    if (requiresAccountAuth && !passwordValue && !options?.accountSession) {
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
        const restoredAccount = options?.accountSession;
        const accountSession = restoredAccount
          ? {
            token: restoredAccount.session.token,
            userId: restoredAccount.session.userId,
            masterKey: restoredAccount.session.masterKey.slice(),
          }
          : await new CloudAccountClient().login(
            httpBaseUrl,
            userIdValue,
            passwordValue,
            currentInstallId,
          );
        restoredAccount?.session.masterKey.fill(0);
        if (!isCurrentAttempt()) {
          accountSession.masterKey.fill(0);
          return;
        }
        client.installDirectAccountIdentity({
          ...accountSession,
          deviceId: currentInstallId,
        });
        saveCloudAccountSession({
          relayUrl: httpBaseUrl,
          username: userIdValue,
          controllerDeviceId: currentInstallId,
          session: accountSession,
        });
        accountSession.masterKey.fill(0);

        // Account authentication is complete. Device discovery and connection
        // belong to the authenticated directory, including empty/offline/error
        // states; none of them may return a successful login to this form.
        const store = useMobileStore.getState();
        store.resetForDeviceSwitch();
        store.setAuthenticatedUserId(accountSession.userId);
        store.setAuthenticatedUserLabel(userIdValue);
        store.setControlTarget(null);
        setConnectionStatus('paired');
        localStorage.setItem(MOBILE_USER_ID_KEY, userIdValue);
        localStorage.removeItem(MOBILE_FAILURE_COUNT_KEY);
        localStorage.removeItem(MOBILE_LOCK_UNTIL_KEY);
        setFailureCount(0);
        setLockUntil(null);
        setPassword('');
        onPairedRef.current(client, new RemoteSessionManager(client), pairingTarget.targetDeviceId?.trim() || undefined);
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
      // ownership checks. A QR pairing id is only a connection credential; it
      // must never be presented as a browser-authenticated account.
      setAuthenticatedUserId(initialSync.authenticated_user_id ?? null);
      setAuthenticatedUserLabel(requiresAccountAuth ? userIdValue : null);

      const sessionMgr = new RemoteSessionManager(client, initialSync.capabilities);
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
      const errorMessage = status === 401 && !!options?.accountSession
        ? t('pairing.accountSessionExpired')
        : rawErrorMessage.includes('timed out')
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
    const currentInstallId = getOrCreateInstallId();
    // Reuse is scan-driven. A plain account landing page must stay idle after
    // an explicit disconnect instead of immediately reconnecting itself.
    const hasScannedAccountTarget = !!pairingTarget.targetDeviceId;
    const restoredAccount = requiresAccountAuth && hasScannedAccountTarget
      ? loadMatchingCloudAccountSession(
        pairingTarget.httpBaseUrl,
        qrUsername,
        currentInstallId,
      )
      : null;
    const prefilledUserId = qrUsername || restoredAccount?.username || savedUserId;
    const persistedFailureCount = Number(localStorage.getItem(MOBILE_FAILURE_COUNT_KEY) || '0');
    const persistedLockUntil = Number(localStorage.getItem(MOBILE_LOCK_UNTIL_KEY) || '0');
    const normalizedLockUntil = persistedLockUntil > Date.now() ? persistedLockUntil : null;
    if (persistedLockUntil && !normalizedLockUntil) {
      localStorage.removeItem(MOBILE_LOCK_UNTIL_KEY);
      localStorage.removeItem(MOBILE_FAILURE_COUNT_KEY);
    }
    // A valid account session is already proof for a same-account QR. Legacy
    // room pairing remains auto-reconnectable only in its passwordless mode.
    const shouldRestoreAccount = !!restoredAccount;
    const shouldRestoreRoom = !requiresAccountAuth
      && !!savedUserId
      && !!currentInstallId
      && !!pairingTarget.room
      && !!pairingTarget.pk;
    setUserId(prefilledUserId);
    setFailureCount(normalizedLockUntil ? persistedFailureCount : 0);
    setLockUntil(normalizedLockUntil);
    setError(null);

    if (shouldRestoreAccount || shouldRestoreRoom) {
      // Show the spinner immediately; attemptPair also sets pairing when the
      // network attempt actually starts (after validation).
      setConnectionStatus('pairing');
      void attemptPairRef.current(prefilledUserId, '', {
        autoReconnect: true,
        installId: currentInstallId,
        accountSession: restoredAccount ?? undefined,
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
            <MobileIconButton
              appearance="plain"
              className="pairing-page__back"
              onClick={() => history.length > 1 && history.back()}
              aria-label={t('common.close')}
              icon={<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" aria-hidden="true">
                <path d="m6 6 12 12M18 6 6 18" />
              </svg>}
            />
          </header>

          {showForm && (
            <PairingForm
              advancedOpen={advancedOpen}
              error={error}
              hasPairingDescriptor={pairingTarget.hasPairingDescriptor}
              isLocked={isLocked}
              password={password}
              passwordInputRef={passwordInputRef}
              relayUrl={relayUrl}
              remainingLockSeconds={remainingLockSeconds}
              requiresAccountAuth={requiresAccountAuth}
              showPassword={showPassword}
              showSpinner={showSpinner}
              submitting={submitting}
              userId={userId}
              usernameInputRef={usernameInputRef}
              onAdvancedOpenChange={setAdvancedOpen}
              onConnect={() => void handleConnect()}
              onOpenScanner={() => setScannerOpen(true)}
              onPasswordChange={setPassword}
              onRelayUrlChange={setRelayUrl}
              onShowPasswordChange={setShowPassword}
              onUserIdChange={setUserId}
            />
          )}

          {!showForm && (
            <MobileStatus
              className="pairing-page__progress"
              loading
              title={connectionStatus === 'paired'
                ? t(requiresAccountAuth ? 'devices.accountReady' : 'pairing.pairedLoadingSessions')
                : t('pairing.connectingAndPairing')}
            />
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
