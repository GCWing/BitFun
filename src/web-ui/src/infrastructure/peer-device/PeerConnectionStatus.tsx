import React, { useEffect, useState } from 'react';
import { Alert, Button } from '@openbitfun/ui';
import { useI18n } from '@/infrastructure/i18n';
import { usePeerDeviceModeOptional } from './peerDeviceContextState';
import './PeerConnectionStatus.scss';

/** Keep the selected host visible while its control link recovers. */
export const PeerConnectionStatus: React.FC = () => {
  const { t } = useI18n('common');
  const peer = usePeerDeviceModeOptional();
  const [returning, setReturning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const deviceId = peer?.peerMode.active ? peer.peerMode.deviceId : null;
  const connection = peer?.attachments.find(item => item.deviceId === deviceId);

  useEffect(() => { setError(null); }, [deviceId, connection?.health]);

  if (!peer?.peerMode.active || connection?.health !== 'degraded') return null;

  const returnLocal = async () => {
    setReturning(true);
    setError(null);
    try {
      await peer.switchToLocal('manual');
    } catch (switchError) {
      setError(switchError instanceof Error ? switchError.message : String(switchError));
    } finally {
      setReturning(false);
    }
  };

  return (
    <div
      className="peer-connection-status"
      data-openbitfun-component="peer-device"
      data-openbitfun-part="connectionStatus"
    >
      <Alert
        tone="warning"
        data-testid="peer-connection-status"
        message={(
          <span
            className="peer-connection-status__content"
            data-openbitfun-component="peer-device"
            data-openbitfun-part="connectionStatusContent"
          >
            <span>{t('peerConnection.reconnecting', { name: peer.peerMode.deviceName })}</span>
            <Button variant="outline" size="sm" disabled={returning} onClick={() => { void returnLocal(); }}>
              {t(returning ? 'deviceOverview.returningToThisDevice' : 'deviceOverview.backToThisDevice')}
            </Button>
          </span>
        )}
        description={error ?? undefined}
      />
    </div>
  );
};
