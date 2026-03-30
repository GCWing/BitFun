import React from 'react';
import { Badge } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import './RemoteConnectDisclaimer.scss';

interface RemoteConnectDisclaimerContentProps {
  agreed: boolean;
  onClose: () => void;
  onAgree?: () => void;
}

export const RemoteConnectDisclaimerContent: React.FC<RemoteConnectDisclaimerContentProps> = ({
  agreed,
  onClose,
  onAgree,
}) => {
  const { t } = useI18n('common');
  const canAgree = !!onAgree && !agreed;

  return (
    <div className="bitfun-remote-disclaimer">
      <div className="bitfun-remote-disclaimer__meta">
        <Badge variant={agreed ? 'success' : 'warning'}>
          {t(agreed ? 'remoteConnect.disclaimerStatusAgreed' : 'remoteConnect.disclaimerStatusPending')}
        </Badge>
      </div>

      <p className="bitfun-remote-disclaimer__text">{t('remoteConnect.disclaimerIntro')}</p>

      <ol className="bitfun-remote-disclaimer__list">
        <li>{t('remoteConnect.disclaimerItemGeneralRisk')}</li>
        <li>{t('remoteConnect.disclaimerItemSecurity')}</li>
        <li>{t('remoteConnect.disclaimerItemEncryption')}</li>
        <li>{t('remoteConnect.disclaimerItemOpenSource')}</li>
        <li>{t('remoteConnect.disclaimerItemPrivacy')}</li>
        <li>{t('remoteConnect.disclaimerItemDataUsage')}</li>
        <li>{t('remoteConnect.disclaimerItemCredentials')}</li>
        <li>{t('remoteConnect.disclaimerItemQrCode')}</li>
        <li>{t('remoteConnect.disclaimerItemNgrok')}</li>
        <li>{t('remoteConnect.disclaimerItemSelfHosted')}</li>
        <li>{t('remoteConnect.disclaimerItemNetwork')}</li>
        <li>{t('remoteConnect.disclaimerItemBot')}</li>
        <li>{t('remoteConnect.disclaimerItemBotPersistence')}</li>
        <li>{t('remoteConnect.disclaimerItemMobileBrowser')}</li>
        <li>{t('remoteConnect.disclaimerItemCompliance')}</li>
        <li>{t('remoteConnect.disclaimerItemLiability')}</li>
      </ol>

      <div className="bitfun-remote-disclaimer__actions">
        <button
          type="button"
          className="bitfun-remote-disclaimer__btn bitfun-remote-disclaimer__btn--secondary"
          onClick={onClose}
        >
          {canAgree ? t('remoteConnect.disclaimerDecline') : t('actions.close')}
        </button>
        {canAgree && (
          <button
            type="button"
            className="bitfun-remote-disclaimer__btn bitfun-remote-disclaimer__btn--primary"
            onClick={onAgree}
          >
            {t('remoteConnect.disclaimerAgree')}
          </button>
        )}
      </div>
    </div>
  );
};
