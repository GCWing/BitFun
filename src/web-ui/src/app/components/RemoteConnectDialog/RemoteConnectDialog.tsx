/**
 * Remote Connect dialog component.
 * Shows a connection type tab switcher and QR code placeholder.
 * Uses component library Modal.
 */

import React, { useState } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { Modal, Badge } from '@/component-library';
import './RemoteConnectDialog.scss';

type ConnectionType = 'nat' | 'relay';

const QrCodePlaceholder: React.FC = () => (
  <svg
    className="bitfun-remote-connect__qr-svg"
    viewBox="0 0 100 100"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    aria-hidden="true"
  >
    <rect x="8" y="8" width="26" height="26" rx="3" fill="none" stroke="currentColor" strokeWidth="3" />
    <rect x="14" y="14" width="14" height="14" rx="1.5" fill="currentColor" />
    <rect x="66" y="8" width="26" height="26" rx="3" fill="none" stroke="currentColor" strokeWidth="3" />
    <rect x="72" y="14" width="14" height="14" rx="1.5" fill="currentColor" />
    <rect x="8" y="66" width="26" height="26" rx="3" fill="none" stroke="currentColor" strokeWidth="3" />
    <rect x="14" y="72" width="14" height="14" rx="1.5" fill="currentColor" />
    <rect x="42" y="8"  width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="50" y="8"  width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="58" y="8"  width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="42" y="16" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="58" y="16" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="50" y="24" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="8"  y="42" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="16" y="42" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="24" y="42" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="8"  y="50" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="24" y="50" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="8"  y="58" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="16" y="58" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="24" y="58" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="42" y="42" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="50" y="42" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="58" y="42" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="66" y="42" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="74" y="42" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="86" y="42" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="42" y="50" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="66" y="50" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="86" y="50" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="42" y="58" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="50" y="58" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="74" y="58" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="86" y="58" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="42" y="66" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="58" y="66" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="74" y="66" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="42" y="74" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="50" y="74" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="66" y="74" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="86" y="74" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="42" y="86" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="58" y="86" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="74" y="86" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
    <rect x="86" y="86" width="6" height="6" rx="1" fill="currentColor" opacity="0.65" />
  </svg>
);

interface RemoteConnectDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

export const RemoteConnectDialog: React.FC<RemoteConnectDialogProps> = ({
  isOpen,
  onClose,
}) => {
  const { t } = useI18n('common');
  const [activeTab, setActiveTab] = useState<ConnectionType>('nat');

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={t('remoteConnect.title')}
      titleExtra={<Badge variant="warning">WIP</Badge>}
      showCloseButton={true}
      size="small"
    >
      <div className="bitfun-remote-connect">

        <div className="bitfun-remote-connect__tabs">
          <button
            type="button"
            className={`bitfun-remote-connect__tab${activeTab === 'nat' ? ' is-active' : ''}`}
            onClick={() => setActiveTab('nat')}
          >
            {t('remoteConnect.typeNatTraversal')}
          </button>
          <span className="bitfun-remote-connect__tab-divider" aria-hidden="true" />
          <button
            type="button"
            className={`bitfun-remote-connect__tab${activeTab === 'relay' ? ' is-active' : ''}`}
            onClick={() => setActiveTab('relay')}
          >
            {t('remoteConnect.typeRelayServer')}
          </button>
        </div>

        <div className="bitfun-remote-connect__body">
          <div className="bitfun-remote-connect__qr-box">
            <QrCodePlaceholder />
          </div>
          <p className="bitfun-remote-connect__hint">
            {t('remoteConnect.hint')}
          </p>
        </div>

      </div>
    </Modal>
  );
};

export default RemoteConnectDialog;
