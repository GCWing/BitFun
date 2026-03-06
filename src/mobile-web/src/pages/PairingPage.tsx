import React, { useEffect, useRef } from 'react';
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

const PairingPage: React.FC<PairingPageProps> = ({ onPaired }) => {
  const { connectionStatus, setConnectionStatus, setError, error } = useMobileStore();
  const pairedRef = useRef(false);

  useEffect(() => {
    const hash = window.location.hash;
    const params = new URLSearchParams(hash.replace(/^#\/pair\?/, ''));
    const room = params.get('room');
    const pk = params.get('pk');
    const relayParam = params.get('relay');

    if (!room || !pk) {
      setError('Invalid QR code: missing room or public key');
      setConnectionStatus('error');
      return;
    }

    let httpBaseUrl: string;
    if (relayParam) {
      httpBaseUrl = relayParam
        .replace(/^wss:\/\//, 'https://')
        .replace(/^ws:\/\//, 'http://')
        .replace(/\/ws\/?$/, '')
        .replace(/\/$/, '');
    } else {
      const origin = window.location.origin;
      const pathname = window.location.pathname
        .replace(/\/[^/]*$/, '')
        .replace(/\/r\/[^/]*$/, '');
      httpBaseUrl = origin + pathname;
    }

    const client = new RelayHttpClient(httpBaseUrl, room);

    (async () => {
      try {
        setConnectionStatus('pairing');
        const initialSync = await client.pair(pk);
        pairedRef.current = true;
        setConnectionStatus('paired');

        const sessionMgr = new RemoteSessionManager(client);

        const store = useMobileStore.getState();
        if (initialSync.has_workspace) {
          store.setCurrentWorkspace({
            has_workspace: true,
            path: initialSync.path,
            project_name: initialSync.project_name,
            git_branch: initialSync.git_branch,
          });
        }
        if (initialSync.sessions) {
          store.setSessions(initialSync.sessions);
        }

        onPaired(client, sessionMgr);
      } catch (e: any) {
        setError(e?.message || 'Pairing failed');
        setConnectionStatus('error');
      }
    })();
  }, []);

  const stateLabels: Record<string, string> = {
    pairing: 'Connecting and pairing...',
    paired: 'Paired! Loading sessions...',
    error: 'Connection error',
  };

  const handleRetry = () => {
    window.location.reload();
  };

  const showRetry = connectionStatus === 'error';
  const showSpinner = connectionStatus === 'pairing';

  return (
    <div className="pairing-page">
      <CubeLogo />
      <div className="pairing-page__brand">BitFun Remote</div>

      <div className="pairing-page__spinner-wrap">
        {showSpinner && <div className="spinner" />}
      </div>

      <div className="pairing-page__state">
        {stateLabels[connectionStatus] || connectionStatus}
      </div>

      {error && <div className="pairing-page__error">{error}</div>}

      {showRetry && (
        <button className="pairing-page__retry" onClick={handleRetry}>
          Retry
        </button>
      )}
    </div>
  );
};

export default PairingPage;
