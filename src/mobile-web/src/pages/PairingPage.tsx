import React, { useEffect, useRef, useState } from 'react';
import PairingForm from '../components/PairingForm';
import { accountDeviceIdFromHash, currentRelayUrl } from '../services/pairingLink';
import { useI18n } from '../i18n';
import { useTheme } from '../theme';
import logoMarkDark from '../assets/openbitfun-mark-dark.png';
import logoMarkLight from '../assets/openbitfun-mark-light.png';
import { CloudAccountClient, generateRequestId, type CloudAccountSession } from '../services/CloudAccountClient';
import { loadMatchingCloudAccountSession, saveCloudAccountSession } from '../services/CloudAccountSessionStore';
import { RelayHttpClient } from '../services/RelayHttpClient';
import { RemoteSessionManager } from '../services/RemoteSessionManager';
import { loadMobileNavigation, type PairedNavigation } from '../services/MobileNavigationStore';
import { useMobileStore } from '../services/store';

interface PairingPageProps {
  onPaired: (client: RelayHttpClient, sessionMgr: RemoteSessionManager,
    preferredDeviceId?: string, navigation?: PairedNavigation) => void;
}

function installId(): string {
  const key = 'openbitfun.mobile.controller_id';
  const existing = sessionStorage.getItem(key);
  if (existing) return existing;
  const created = generateRequestId();
  sessionStorage.setItem(key, created);
  return created;
}

function routeKey(): string { return `${window.location.pathname}${window.location.hash}`; }

const PairingPageContent: React.FC<PairingPageProps> = ({ onPaired }) => {
  const { t } = useI18n();
  const { isDark } = useTheme();
  const relayUrl = currentRelayUrl();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const generation = useRef(0);
  const pending = useRef<AbortController | null>(null);
  const popup = useRef<Window | null>(null);
  const onPairedRef = useRef(onPaired);
  onPairedRef.current = onPaired;
  const targetDeviceId = accountDeviceIdFromHash(window.location.hash) || undefined;

  const connect = (session: CloudAccountSession, controllerDeviceId: string, restore: boolean) => {
    const client = new RelayHttpClient(relayUrl, { ...session, deviceId: controllerDeviceId });
    saveCloudAccountSession({ relayUrl: relayUrl, username: session.userId,
      controllerDeviceId, session });
    const store = useMobileStore.getState();
    store.resetForDeviceSwitch();
    store.setAuthenticatedUserId(session.userId);
    store.setAuthenticatedUserLabel(session.userId);
    store.setControlTarget(null);
    store.setConnectionStatus('paired');
    const scope = { accountId: session.userId, controllerDeviceId,
      relayUrl: relayUrl, routeKey: routeKey() };
    const navigation = restore ? loadMobileNavigation(scope) : null;
    session.masterKey.fill(0);
    onPairedRef.current(client, new RemoteSessionManager(client),
      targetDeviceId || navigation?.deviceId || undefined, { scope, restored: navigation });
  };

  useEffect(() => {
    // Only opening a target invitation resumes the tab's authenticated controller.
    if (targetDeviceId) {
      const id = installId();
      const saved = loadMatchingCloudAccountSession(relayUrl, '', id);
      if (saved) connect(saved.session, id, true);
    }
    return () => {
      generation.current += 1;
      pending.current?.abort();
      popup.current?.close();
    };
    // Each QR route remounts this component and owns one connection attempt.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const signIn = async () => {
    if (busy) return;
    const authWindow = window.open('about:blank', '_blank');
    if (!authWindow) { setError(t('pairing.allowSignInPopup')); return; }
    authWindow.opener = null;
    popup.current = authWindow;
    const attempt = ++generation.current;
    const controller = new AbortController();
    pending.current = controller;
    setBusy(true); setError(null);
    try {
      const account = new CloudAccountClient(relayUrl);
      const accessToken = await account.authorize(authWindow, controller.signal);
      if (generation.current !== attempt) return;
      const id = installId();
      const session = await account.login(accessToken, id);
      if (generation.current !== attempt) { session.masterKey.fill(0); return; }
      connect(session, id, false);
    } catch (cause) {
      if (generation.current === attempt) setError(cause instanceof Error ? cause.message : t('pairing.loginFailed'));
    } finally {
      authWindow.close();
      if (generation.current === attempt) { setBusy(false); pending.current = null; }
    }
  };

  const cancel = () => {
    generation.current += 1;
    pending.current?.abort(); popup.current?.close();
    setBusy(false);
  };

  return <div className="pairing-page"><div className="pairing-page__shell">
    <div className="pairing-page__brand">
      <img src={isDark ? logoMarkLight : logoMarkDark} alt="" width="28" height="28" />
      <span>OpenBitFun</span>
    </div>
    <section className="pairing-page__panel">
      <PairingForm busy={busy} error={error} onSignIn={() => void signIn()} onCancel={cancel} />
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
