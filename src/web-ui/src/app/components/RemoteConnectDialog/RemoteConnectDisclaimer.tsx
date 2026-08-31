import { Button, ScrollArea, StatusPill } from '@bitfun/ui';
import React from 'react';
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
    <div data-bf-component="remote-connect-disclaimer" data-bf-part="root" className="bitfun-remote-disclaimer">
      <div className="bitfun-remote-disclaimer__meta" data-bf-component="remote-connect-disclaimer" data-bf-part="meta">
        <StatusPill tone={agreed ? 'success' : 'warning'}>
          {t(agreed ? 'remoteConnect.disclaimerStatusAgreed' : 'remoteConnect.disclaimerStatusPending')}
        </StatusPill>
      </div>

      <p className="bitfun-remote-disclaimer__text" data-bf-component="remote-connect-disclaimer" data-bf-part="intro">{t('remoteConnect.disclaimerIntro')}</p>

      <h3 className="bitfun-remote-disclaimer__section-title" data-bf-component="remote-connect-disclaimer" data-bf-part="title">
        {t('remoteConnect.disclaimerKeyRisks')}
      </h3>
      <ol className="bitfun-remote-disclaimer__list bitfun-remote-disclaimer__list--key" data-bf-component="remote-connect-disclaimer" data-bf-part="riskList">
        <li>{t('remoteConnect.disclaimerItemGeneralRisk')}</li>
        <li>{t('remoteConnect.disclaimerItemSecurity')}</li>
        <li>{t('remoteConnect.disclaimerItemEncryption')}</li>
        <li>{t('remoteConnect.disclaimerItemPrivacy')}</li>
      </ol>

      <details className="bitfun-remote-disclaimer__details" data-bf-component="remote-connect-disclaimer" data-bf-part="details">
        <summary>{t('remoteConnect.disclaimerFullDetails')}</summary>
        <ScrollArea className="bitfun-remote-disclaimer__list-scroll">
          <ol className="bitfun-remote-disclaimer__list" start={5}>
            <li>{t('remoteConnect.disclaimerItemOpenSource')}</li>
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
        </ScrollArea>
      </details>

      <div className="bitfun-remote-disclaimer__actions" data-bf-component="remote-connect-disclaimer" data-bf-part="actions">
        <Button
          className="bitfun-remote-disclaimer__action"
          variant="outline"
          size="md"
          onClick={onClose}
        >
          {canAgree ? t('remoteConnect.disclaimerDecline') : t('actions.close')}
        </Button>
        {canAgree && (
          <Button
            className="bitfun-remote-disclaimer__action"
            variant="fill"
            size="md"
            onClick={onAgree}
            data-testid="remote-connect-disclaimer-agree"
          >
            {t('remoteConnect.disclaimerAgree')}
          </Button>
        )}
      </div>
    </div>
  );
};
