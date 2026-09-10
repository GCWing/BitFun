import { Button, Icon, StatusPill } from '@openbitfun/ui';
import type { ReactNode } from 'react';
import type { ConnectionResult, RemoteConnectStatus } from '@/infrastructure/api/service-api/RemoteConnectAPI';
import { useI18n } from '@/infrastructure/i18n';
import { normalizeRelayUrl, selectRemoteNetworkConnection } from '@/infrastructure/remote-connect/remoteConnectionState';
import { RemotePairingCard } from './RemotePairingCard';

interface RemoteNetworkConnectionsProps {
  status: RemoteConnectStatus | null;
  method: 'openbitfun_server' | 'lan';
  settings?: ReactNode;
  title: string;
  relayUrl: string;
  invitation: ConnectionResult | null;
  statusState: 'loading' | 'ready' | 'unavailable';
  loading: boolean;
  pairingUrlCopied: boolean;
  error: ReactNode;
  onCopyPairingUrl: () => Promise<void>;
  onConnect: () => void;
  onCancel: () => void;
  onDisconnect: () => void;
}

/** Authenticated Relay connection and controller presence, independent of hosting. */
export function RemoteNetworkConnections({
  status, method, title, relayUrl, settings, invitation, statusState,
  loading, pairingUrlCopied, error, onCopyPairingUrl, onConnect, onCancel,
  onDisconnect,
}: RemoteNetworkConnectionsProps) {
  const { t, formatNumber } = useI18n('common');
  const connection = selectRemoteNetworkConnection(status, invitation);
  const account = connection.connected && connection.method === method
    && connection.relayUrl === normalizeRelayUrl(relayUrl);
  const clients = account ? status?.clients ?? [] : [];
  const count = clients.length;
  const connected = account;

  return <div className="openbitfun-remote-connect__body openbitfun-remote-connect__body--network">
    <section
      className="openbitfun-remote-connect__network-card openbitfun-remote-connect__relay-card"
      data-openbitfun-component="remote-connect-dialog"
      data-openbitfun-part="connections"
      aria-label={title}
    >
      <div className="openbitfun-remote-connect__network-heading">
        <Icon name="browser" size="lg" aria-hidden="true" />
        <h3>{title}</h3>
        <span role="status"><StatusPill tone={statusState === 'ready' && connected ? 'success' : 'neutral'}>
          {t(statusState === 'unavailable' ? 'remoteConnect.statusUnavailable'
            : statusState === 'loading' ? 'remoteConnect.statusChecking'
              : connected ? 'remoteConnect.stateConnected' : 'remoteConnect.notConnected')}
        </StatusPill></span>
      </div>
      {settings && <div className="openbitfun-remote-connect__relay-settings">{settings}</div>}
      {invitation && <div className="openbitfun-remote-connect__relay-invitation">
        <RemotePairingCard
          owner="network"
          qrUrl={invitation.qr_url}
          connected={connection.invitationConnected}
          copied={pairingUrlCopied}
          statusState={statusState}
          onCopyUrl={onCopyPairingUrl}
        />
      </div>}
      {(count > 0 || !invitation) && <div className="openbitfun-remote-connect__connections-content">
        <div className="openbitfun-remote-connect__connections-heading">
          <h4 title={t('remoteConnect.clientCountHint')}>{t('remoteConnect.connectedClients')}</h4>
          <span role="status">
            <StatusPill tone={count > 0 ? 'success' : 'neutral'}>{t('remoteConnect.clientCount', { count, formattedCount: formatNumber(count) })}</StatusPill>
          </span>
        </div>
        {count > 0 && <ul className="openbitfun-remote-connect__connections-list" tabIndex={count > 3 ? 0 : undefined}>
          {clients.map((client, index) => <li key={client.id}>
            <Icon name="browser" size="sm" aria-hidden="true" />
            <strong>{client.name || t('remoteConnect.mobileBrowserTitle')}</strong>
            <span>{t('remoteConnect.clientNumber', { number: formatNumber(index + 1) })}</span>
          </li>)}

        </ul>}
        {count === 0 && <p className="openbitfun-remote-connect__connections-note">{t('remoteConnect.noConnectedClients')}</p>}
      </div>}
      <div className="openbitfun-remote-connect__relay-actions">
        {error}
        <div className="openbitfun-remote-connect__relay-action-row">
          {invitation
            ? <Button variant="fill" size="sm" onClick={onCancel}>{t('remoteConnect.cancelInvitation')}</Button>
            : <Button variant="primary" size="sm" loading={loading} onClick={onConnect}>
              {loading ? t('remoteConnect.connecting') : t('remoteConnect.showConnectionCode')}
            </Button>}
          {connected && <Button variant="outline" size="sm" onClick={onDisconnect}>{t('remoteConnect.disconnect')}</Button>}
        </div>
        {connection.invitationConnected && invitation && <p className="openbitfun-remote-connect__connections-note">{t('remoteConnect.accountConnectedHint')}</p>}
      </div>
    </section>
  </div>;
}
