import { Icon, IconButton, StatusPill } from '@openbitfun/ui';
import { QRCodeSVG } from 'qrcode.react';
import { useI18n } from '@/infrastructure/i18n';

interface RemotePairingCardProps {
  qrUrl?: string | null;
  pairingCode?: string | null;
  owner: 'bot' | 'network';
  copied: boolean;
  onCopyUrl: () => void | Promise<void>;
}

/** One shared pending state for chat-app codes, network QR links, and restored connections. */
export function RemotePairingCard({ qrUrl, pairingCode, owner, copied, onCopyUrl }: RemotePairingCardProps) {
  const { t } = useI18n('common');
  const hasCopiedUrl = Boolean(qrUrl && copied);

  return (
    <div
      className={`openbitfun-remote-connect__pairing-card${qrUrl ? '' : ' openbitfun-remote-connect__pairing-card--compact'}`}
      data-openbitfun-component="remote-connect-dialog"
      data-openbitfun-part="pairingCard"
    >
      {(qrUrl || pairingCode) && (
        <div className="openbitfun-remote-connect__pairing-visual">
          {qrUrl && (
            <button
              type="button"
              className="openbitfun-remote-connect__qr-box"
              title={t('remoteConnect.copyUrl')}
              aria-label={t('remoteConnect.copyUrl')}
              onClick={() => void onCopyUrl()}
            >
              <QRCodeSVG value={qrUrl} size={180} level="M" includeMargin />
            </button>
          )}
          {pairingCode && (
            <div className="openbitfun-remote-connect__pairing-code" dir="ltr">
              {pairingCode}
            </div>
          )}
        </div>
      )}
      <div className="openbitfun-remote-connect__pairing-details">
        <div className="openbitfun-remote-connect__pairing-status" role="status">
          <StatusPill tone={hasCopiedUrl ? 'success' : 'warning'}>
            {hasCopiedUrl
              ? t('remoteConnect.urlCopied')
              : owner === 'bot'
                ? t('remoteConnect.stateWaitingBot')
                : t('remoteConnect.stateWaiting')}
          </StatusPill>
        </div>
        {qrUrl ? (
          <>
            <span className="openbitfun-remote-connect__pairing-label">
              {t('remoteConnect.workspaceAddress')}
            </span>
            <div className="openbitfun-remote-connect__pairing-url-row">
              <span title={qrUrl}>{qrUrl}</span>
              <IconButton
                aria-label={t('remoteConnect.copyUrl')}
                title={t('remoteConnect.copyUrl')}
                icon={<Icon name={hasCopiedUrl ? 'check-line' : 'duplicate'} size="lg" />}
                onClick={() => void onCopyUrl()}
                size="sm"
                variant="quiet"
              />
            </div>
            <div className="openbitfun-remote-connect__pairing-instruction">
              <Icon name="browser" size="lg" aria-hidden="true" />
              <p>{t('remoteConnect.scanHint')}</p>
            </div>
            <div className="openbitfun-remote-connect__pairing-instruction">
              <Icon name="link" size="lg" aria-hidden="true" />
              <p>{t('remoteConnect.mobileBrowserDescription')}</p>
            </div>
          </>
        ) : owner === 'bot' && pairingCode ? (
          <p className="openbitfun-remote-connect__hint">{t('remoteConnect.botHint')}</p>
        ) : null}
      </div>
    </div>
  );
}
