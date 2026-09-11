import React, { useEffect, useRef, useState } from 'react';
import { MobileStatus } from '@openbitfun/ui/mobile';
import PairingForm from '../components/PairingForm';
import { accountDeviceIdFromHash, currentRelayUrl } from '../services/pairingLink';
import { useI18n } from '../i18n';
import { useTheme } from '../theme';
import logoMarkDark from '../assets/openbitfun-mark-dark.png';
import logoMarkLight from '../assets/openbitfun-mark-light.png';
import { CloudAccountClient, type CloudAccountSession } from '../services/CloudAccountClient';
import {
  BrowserAccountChangedError, BrowserAccountStorageError, getBrowserAccountStore, releaseBrowserAccount,
  type BrowserAccountSnapshot, type BrowserAccountStore,
} from '../services/BrowserAccountStore';
import { RelayHttpClient } from '../services/RelayHttpClient';
import { RemoteSessionManager } from '../services/RemoteSessionManager';
import { loadMobileNavigation, type PairedNavigation } from '../services/MobileNavigationStore';
import { useMobileStore } from '../services/store';

export interface BrowserAccountBinding { store: BrowserAccountStore; token: string; }
interface PairingPageProps {
  onPaired: (client: RelayHttpClient, sessionMgr: RemoteSessionManager,
    preferredDeviceId?: string, navigation?: PairedNavigation, account?: BrowserAccountBinding) => void;
}

function routeKey(): string { return `${window.location.pathname}${window.location.hash}`; }

const PairingPageContent: React.FC<PairingPageProps> = ({ onPaired }) => {
  const { t } = useI18n();
  const { isDark } = useTheme();
  const relayUrl = currentRelayUrl();
  const accountStore = getBrowserAccountStore(relayUrl);
  const [restoring, setRestoring] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const generation = useRef(0);
  const connected = useRef(false);
  const loginRevision = useRef<number | null>(null);
  const pending = useRef<AbortController | null>(null);
  const popup = useRef<Window | null>(null);
  const onPairedRef = useRef(onPaired);
  onPairedRef.current = onPaired;
  const targetDeviceId = accountDeviceIdFromHash(window.location.hash) || undefined;

  const friendlyError = (cause: unknown) => cause instanceof BrowserAccountStorageError
    ? t(cause.reason === 'invalid' ? 'pairing.browserStorageInvalid' : 'pairing.browserStorageUnavailable')
    : cause instanceof BrowserAccountChangedError ? t('pairing.signInChanged')
      : cause instanceof Error ? cause.message : t('pairing.loginFailed');

  const connect = (saved: BrowserAccountSnapshot, restore: boolean) => {
    const session = saved.session;
    if (!session || connected.current) return;
    connected.current = true;
    generation.current += 1;
    pending.current?.abort();
    popup.current?.close();
    const controllerDeviceId = saved.controllerDeviceId;
    const client = new RelayHttpClient(relayUrl, { ...session, deviceId: controllerDeviceId });
    const store = useMobileStore.getState();
    store.resetForDeviceSwitch();
    store.setAuthenticatedUserId(session.userId);
    store.setAuthenticatedUserLabel(session.userId);
    store.setControlTarget(null);
    store.setConnectionStatus('paired');
    const scope = { accountId: session.userId, controllerDeviceId, relayUrl, routeKey: routeKey() };
    const navigation = restore ? loadMobileNavigation(scope) : null;
    onPairedRef.current(client, new RemoteSessionManager(client),
      navigation?.deviceId || targetDeviceId || undefined, { scope, restored: navigation },
      { store: accountStore, token: session.token });
  };

  useEffect(() => {
    let disposed = false;
    let readGeneration = 0;
    const restore = async () => {
      const request = ++readGeneration;
      try {
        const saved = await accountStore.read();
        try {
          if (disposed || request !== readGeneration || connected.current) return;
          if (saved.session) {
            connect(saved, true);
          } else {
            // A sign-out in another tab also cancels an already-open OAuth popup.
            if (loginRevision.current !== null && loginRevision.current !== saved.revision) {
              generation.current += 1;
              pending.current?.abort(); popup.current?.close();
              pending.current = null;
              loginRevision.current = null;
              setBusy(false);
            }
            if (saved.lastChange === 'expired') setError(t('pairing.accountSessionExpired'));
          }
        } finally { releaseBrowserAccount(saved); }
      } catch (cause) {
        if (!disposed && request === readGeneration) setError(friendlyError(cause));
      } finally {
        if (!disposed && request === readGeneration) setRestoring(false);
      }
    };
    const unsubscribe = accountStore.subscribe(() => { void restore(); });
    void restore();
    return () => {
      disposed = true;
      unsubscribe();
      generation.current += 1;
      pending.current?.abort();
      popup.current?.close();
    };
    // Each route owns its restore/subscription lifecycle. StrictMode's first
    // async restore is fenced by disposal before it can hand off a connection.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [accountStore]);

  const signIn = async () => {
    if (pending.current || connected.current || restoring) return;
    const authWindow = window.open('about:blank', '_blank');
    if (!authWindow) { setError(t('pairing.allowSignInPopup')); return; }
    authWindow.opener = null;
    popup.current = authWindow;
    const attempt = ++generation.current;
    const controller = new AbortController();
    const isCurrent = () => generation.current === attempt && !controller.signal.aborted && !connected.current;
    pending.current = controller;
    setBusy(true); setError(null);
    const account = new CloudAccountClient(relayUrl);
    let browser: BrowserAccountSnapshot | null = null;
    let candidate: CloudAccountSession | null = null;
    let committed = false;
    try {
      browser = await accountStore.read();
      if (!isCurrent()) return;
      if (browser.session) { connect(browser, true); return; }
      loginRevision.current = browser.revision;
      const accessToken = await account.authorize(authWindow, controller.signal);
      if (!isCurrent()) return;
      candidate = await account.login(accessToken, browser.controllerDeviceId, browser.privateKey);
      if (!isCurrent()) return;
      const saved = await accountStore.saveSession(browser, candidate, isCurrent);
      committed = true;
      try { if (isCurrent()) connect(saved, false); }
      finally { releaseBrowserAccount(saved); }
    } catch (cause) {
      if (isCurrent()) setError(friendlyError(cause));
    } finally {
      if (candidate) {
        if (!committed) {
          void account.logout(candidate.token).catch(() => {
            console.warn('Could not revoke an unused browser sign-in token');
          });
        }
        candidate.masterKey.fill(0);
      }
      if (browser) releaseBrowserAccount(browser);
      authWindow.close();
      if (generation.current === attempt) {
        setBusy(false); pending.current = null; loginRevision.current = null;
      }
    }
  };

  const cancel = () => {
    generation.current += 1;
    pending.current?.abort(); popup.current?.close();
    pending.current = null;
    loginRevision.current = null;
    setBusy(false);
  };

  return <div className="pairing-page"><div className="pairing-page__shell">
    <div className="pairing-page__brand">
      <img src={isDark ? logoMarkLight : logoMarkDark} alt="" width="28" height="28" />
      <span>OpenBitFun</span>
    </div>
    <section className="pairing-page__panel">
      {restoring ? <MobileStatus loading title={t('pairing.restoringAccount')} />
        : <PairingForm busy={busy} error={error} onSignIn={() => void signIn()} onCancel={cancel} />}
    </section>
  </div></div>;
};

const PairingPage: React.FC<PairingPageProps> = (props) => {
  const [route, setRoute] = useState(routeKey);
  useEffect(() => {
    const change = () => setRoute(routeKey());
    window.addEventListener('hashchange', change);
    return () => window.removeEventListener('hashchange', change);
  }, []);
  return <PairingPageContent key={route} {...props} />;
};
export default PairingPage;
