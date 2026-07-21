/**
 * Peer Remote status row — shown while Peer Device Mode is active.
 * Mounted as a dedicated full-width row above the NavPanel footer.
 */

import React, { useCallback } from 'react';
import { Monitor } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { usePeerDeviceModeOptional } from '@/infrastructure/peer-device/peerDeviceContextState';
import { useNotification } from '@/shared/notification-system';
import './PeerRemoteBadge.scss';

export const PeerRemoteBadge: React.FC = () => {
  const { t } = useI18n('common');
  const { success, warning } = useNotification();
  const peerDevice = usePeerDeviceModeOptional();

  const handleDisconnect = useCallback(async () => {
    if (!peerDevice?.peerMode.active) {
      return;
    }
    try {
      await peerDevice.exitPeerMode();
      success(t('accountLogin.disconnectPeer'));
    } catch (e) {
      warning(e instanceof Error ? e.message : String(e));
    }
  }, [peerDevice, success, t, warning]);

  if (!peerDevice?.peerMode.active) {
    return null;
  }

  const { deviceName } = peerDevice.peerMode;

  return (
    <div
      className="bitfun-peer-remote-badge"
      data-testid="peer-remote-badge"
      title={t('accountLogin.peerRemoteBadgeTitle', { name: deviceName })}
    >
      <Monitor size={13} className="bitfun-peer-remote-badge__icon" aria-hidden="true" />
      <span className="bitfun-peer-remote-badge__label">
        {t('accountLogin.peerRemoteLabel', { name: deviceName })}
      </span>
      <button
        type="button"
        className="bitfun-peer-remote-badge__disconnect"
        onClick={() => {
          void handleDisconnect();
        }}
      >
        {t('accountLogin.disconnectPeer')}
      </button>
    </div>
  );
};

export default PeerRemoteBadge;
