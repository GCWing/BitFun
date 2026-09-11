import React from 'react';
import { MobileBanner, MobileButton, MobileStatus } from '@openbitfun/ui/mobile';
import { useI18n } from '../i18n';

interface PairingFormProps {
  busy: boolean;
  error: string | null;
  onSignIn: () => void;
  onCancel: () => void;
}

/** Presentation for the global GitHub sign-in. */
const PairingForm: React.FC<PairingFormProps> = ({ busy, error, onSignIn, onCancel }) => {
  const { t } = useI18n();
  return (
    <form className="pairing-page__form" onSubmit={(event) => { event.preventDefault(); onSignIn(); }}>
      <div className="pairing-page__scroll">
        <div className="pairing-page__form-content">
          <h1 className="pairing-page__title">{t('pairing.loginTitle')}</h1>
          <p className="pairing-page__intro">{t('pairing.githubDescription')}</p>
          {error && <MobileBanner className="pairing-page__error" tone="danger">{error}</MobileBanner>}
          {busy && <MobileStatus loading title={t('pairing.waitingForGitHub')} />}
        </div>
      </div>
      <div className="pairing-page__action">
        <MobileButton appearance="primary" block className="pairing-page__retry" type="submit" disabled={busy}>
          {t('pairing.githubSignIn')}
        </MobileButton>
        {busy && <MobileButton appearance="plain" onClick={onCancel}>{t('common.cancel')}</MobileButton>}
      </div>
    </form>
  );
};
export default PairingForm;
