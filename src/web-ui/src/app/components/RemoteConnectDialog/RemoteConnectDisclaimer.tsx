import { Button, ScrollArea, StatusPill } from '@openbitfun/ui';
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
    <div data-openbitfun-component="remote-connect-disclaimer" data-openbitfun-part="root" className="openbitfun-remote-disclaimer">
      <div className="openbitfun-remote-disclaimer__meta" data-openbitfun-component="remote-connect-disclaimer" data-openbitfun-part="meta">
        <StatusPill tone={agreed ? 'success' : 'warning'}>
          {t(agreed ? 'remoteConnect.disclaimerStatusAgreed' : 'remoteConnect.disclaimerStatusPending')}
        </StatusPill>
      </div>

      <p className="openbitfun-remote-disclaimer__text" data-openbitfun-component="remote-connect-disclaimer" data-openbitfun-part="intro">{t('remoteConnect.disclaimerIntro')}</p>

      <h3 className="openbitfun-remote-disclaimer__section-title" data-openbitfun-component="remote-connect-disclaimer" data-openbitfun-part="title">
        {t('remoteConnect.disclaimerKeyRisks')}
      </h3>
      <ol className="openbitfun-remote-disclaimer__list openbitfun-remote-disclaimer__list--key" data-openbitfun-component="remote-connect-disclaimer" data-openbitfun-part="riskList">
        <li>{t('remoteConnect.disclaimerItemGeneralRisk')}</li>
        <li>{t('remoteConnect.disclaimerItemSecurity')}</li>
        <li>{t('remoteConnect.disclaimerItemEncryption')}</li>
        <li>{t('remoteConnect.disclaimerItemPrivacy')}</li>
      </ol>

      <details className="openbitfun-remote-disclaimer__details" data-openbitfun-component="remote-connect-disclaimer" data-openbitfun-part="details">
        <summary>{t('remoteConnect.disclaimerFullDetails')}</summary>
        <ScrollArea className="openbitfun-remote-disclaimer__list-scroll">
          <ol className="openbitfun-remote-disclaimer__list" start={5}>
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

      <div className="openbitfun-remote-disclaimer__actions" data-openbitfun-component="remote-connect-disclaimer" data-openbitfun-part="actions">
        <Button
          className="openbitfun-remote-disclaimer__action"
          variant="outline"
          size="sm"
          onClick={onClose}
        >
          {canAgree ? t('remoteConnect.disclaimerDecline') : t('actions.close')}
        </Button>
        {canAgree && (
          <Button
            className="openbitfun-remote-disclaimer__action"
            variant="fill"
            size="sm"
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
