import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import LanguageToggleButton from '../components/LanguageToggleButton';
import { useI18n } from '../i18n';
import { RelayHttpClient } from '../services/RelayHttpClient';
import { RemoteSessionManager } from '../services/RemoteSessionManager';
import { useMobileStore } from '../services/store';

interface PairingPageProps {
  onPaired: (client: RelayHttpClient, sessionMgr: RemoteSessionManager) => void;
}

const CubeLogo: React.FC = () => (
  <div className="pairing-page__cube">
    <div className="pairing-page__cube-inner">
      <div className="pairing-page__cube-face pairing-page__cube-face--front" />
      <div className="pairing-page__cube-face pairing-page__cube-face--back" />
      <div className="pairing-page__cube-face pairing-page__cube-face--right" />
      <div className="pairing-page__cube-face pairing-page__cube-face--left" />
      <div className="pairing-page__cube-face pairing-page__cube-face--top" />
      <div className="pairing-page__cube-face pairing-page__cube-face--bottom" />
    </div>
  </div>
);

const MOBILE_INSTALL_ID_KEY = 'bitfun.mobile.install_id';
const MOBILE_USER_ID_KEY = 'bitfun.mobile.user_id';
const MOBILE_LOCK_UNTIL_KEY = 'bitfun.mobile.user_id_lock_until';
const MOBILE_FAILURE_COUNT_KEY = 'bitfun.mobile.user_id_failure_count';
const MAX_FAILED_USER_ID_ATTEMPTS = 3;
const USER_ID_LOCKOUT_MS = 60_000;

function isProtectedUserIdError(message: string): boolean {
  return message.includes('This remote URL is already protected')
    || message.includes('This mobile device must continue using the previously confirmed user ID');
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

function resolveRelayBaseUrl(): { room: string | null; pk: string | null; httpBaseUrl: string } {
  const hash = window.location.hash;
  const params = new URLSearchParams(hash.replace(/^#\/pair\?/, ''));
  const room = params.get('room');
  const pk = params.get('pk');
  const relayParam = params.get('relay');

  if (relayParam) {
    return {
      room,
      pk,
      httpBaseUrl: relayParam
        .replace(/^wss:\/\//, 'https://')
        .replace(/^ws:\/\//, 'http://')
        .replace(/\/ws\/?$/, '')
        .replace(/\/$/, ''),
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
  };
}

const PairingPage: React.FC<PairingPageProps> = ({ onPaired }) => {
  const { t } = useI18n();
  const {
    connectionStatus,
    setConnectionStatus,
    setError,
    error,
    setAuthenticatedUserId,
  } = useMobileStore();
  const [userId, setUserId] = useState('');
  const [mobileInstallId, setMobileInstallId] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [failureCount, setFailureCount] = useState(0);
  const [lockUntil, setLockUntil] = useState<number | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const autoReconnectAttemptedRef = useRef(false);
  const failureCountRef = useRef(0);
  const lockUntilRef = useRef<number | null>(null);

  const pairingTarget = useMemo(() => resolveRelayBaseUrl(), []);
  const isLocked = !!lockUntil && lockUntil > now;
  const remainingLockSeconds = isLocked
    ? Math.max(1, Math.ceil((lockUntil - now) / 1000))
    : 0;

  const attemptPair = useCallback(async (
    providedUserId: string,
    options?: { autoReconnect?: boolean; installId?: string },
  ) => {
    const userIdValue = providedUserId.trim();
    const autoReconnect = options?.autoReconnect === true;
    const currentInstallId = options?.installId || mobileInstallId || getOrCreateInstallId();
    const activeLockUntil = lockUntilRef.current;
    const lockActive = !!activeLockUntil && activeLockUntil > Date.now();
    const currentRemainingLockSeconds = lockActive
      ? Math.max(1, Math.ceil((activeLockUntil - Date.now()) / 1000))
      : 0;
    if (!pairingTarget.room || !pairingTarget.pk) {
      setError(t('pairing.invalidQrCode'));
      setConnectionStatus('error');
      return;
    }
    if (!userIdValue) {
      setError(t('pairing.userIdRequired'));
      setConnectionStatus('error');
      return;
    }
    if (!autoReconnect && lockActive) {
      setError(t('pairing.tooManyAttempts', { seconds: currentRemainingLockSeconds }));
      setConnectionStatus('error');
      return;
    }

    setMobileInstallId(currentInstallId);
    setSubmitting(true);

    const client = new RelayHttpClient(pairingTarget.httpBaseUrl, pairingTarget.room);

    try {
      setError(null);
      setConnectionStatus('pairing');
      const initialSync = await client.pair(pairingTarget.pk, {
        userId: userIdValue,
        mobileInstallId: currentInstallId,
      });
      setConnectionStatus('paired');
      localStorage.setItem(MOBILE_USER_ID_KEY, userIdValue);
      localStorage.removeItem(MOBILE_FAILURE_COUNT_KEY);
      localStorage.removeItem(MOBILE_LOCK_UNTIL_KEY);
      setFailureCount(0);
      setLockUntil(null);
      setAuthenticatedUserId(initialSync.authenticated_user_id ?? userIdValue);

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
          });
        }
      }
      if (initialSync.sessions) {
        store.setSessions(initialSync.sessions);
      }
      onPaired(client, sessionMgr);
    } catch (e: any) {
      const errorMessage = e?.message || t('pairing.pairingFailed');
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
            : errorMessage,
        );
      } else {
        setError(errorMessage);
      }
      setConnectionStatus('error');
    } finally {
      setSubmitting(false);
    }
  }, [mobileInstallId, pairingTarget.httpBaseUrl, pairingTarget.pk, pairingTarget.room, setAuthenticatedUserId, setConnectionStatus, setError, t]);

  useEffect(() => {
    const savedUserId = localStorage.getItem(MOBILE_USER_ID_KEY)?.trim() ?? '';
    const currentInstallId = getOrCreateInstallId();
    const persistedFailureCount = Number(localStorage.getItem(MOBILE_FAILURE_COUNT_KEY) || '0');
    const persistedLockUntil = Number(localStorage.getItem(MOBILE_LOCK_UNTIL_KEY) || '0');
    const normalizedLockUntil = persistedLockUntil > Date.now() ? persistedLockUntil : null;
    if (persistedLockUntil && !normalizedLockUntil) {
      localStorage.removeItem(MOBILE_LOCK_UNTIL_KEY);
      localStorage.removeItem(MOBILE_FAILURE_COUNT_KEY);
    }
    const shouldAutoReconnect = !!savedUserId && !!currentInstallId && !!pairingTarget.room && !!pairingTarget.pk;
    setUserId(savedUserId);
    setMobileInstallId(currentInstallId);
    setFailureCount(normalizedLockUntil ? persistedFailureCount : 0);
    setLockUntil(normalizedLockUntil);
    setConnectionStatus(shouldAutoReconnect ? 'pairing' : 'idle');
    setError(null);
    if (shouldAutoReconnect && !autoReconnectAttemptedRef.current) {
      autoReconnectAttemptedRef.current = true;
      void attemptPair(savedUserId, { autoReconnect: true, installId: currentInstallId });
    }
  }, [attemptPair, pairingTarget.pk, pairingTarget.room, setConnectionStatus, setError]);

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
    autoReconnectAttemptedRef.current = true;
    await attemptPair(userId, { autoReconnect: false });
  };

  const stateLabels: Record<string, string> = {
    idle: t('pairing.enterUserIdToContinue'),
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
      </div>
      <CubeLogo />
      <div className="pairing-page__brand">{t('common.appName')}</div>

      <div className="pairing-page__spinner-wrap">
        {showSpinner && <div className="spinner" />}
      </div>

      <div className="pairing-page__state">
        {stateLabels[connectionStatus] || connectionStatus}
      </div>

      {showForm && (
        <div className="pairing-page__form">
          <label className="pairing-page__field">
            <span className="pairing-page__field-label">{t('pairing.fieldLabel')}</span>
            <input
              className="pairing-page__input"
              type="text"
              value={userId}
              onChange={(e) => setUserId(e.target.value)}
              placeholder={t('pairing.placeholder')}
              autoCapitalize="off"
              autoCorrect="off"
              autoComplete="username"
              disabled={submitting || isLocked}
            />
          </label>
          <p className="pairing-page__note">
            {t('pairing.note')}
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
