import React from 'react';
import {
  MobileBanner,
  MobileButton,
  MobileDisclosure,
  MobileIconButton,
  MobileTextField,
} from '@openbitfun/ui/mobile';
import { useI18n } from '../i18n';

interface PairingFormProps {
  advancedOpen: boolean;
  error: string | null;
  hasPairingDescriptor: boolean;
  isLocked: boolean;
  password: string;
  passwordInputRef: React.RefObject<HTMLInputElement>;
  relayUrl: string;
  remainingLockSeconds: number;
  requiresAccountAuth: boolean;
  showPassword: boolean;
  showSpinner: boolean;
  submitting: boolean;
  userId: string;
  usernameInputRef: React.RefObject<HTMLInputElement>;
  onAdvancedOpenChange: (open: boolean) => void;
  onConnect: () => void;
  onOpenScanner: () => void;
  onPasswordChange: (value: string) => void;
  onRelayUrlChange: (value: string) => void;
  onShowPasswordChange: (visible: boolean) => void;
  onUserIdChange: (value: string) => void;
}

/** Visual contract for manual account/pairing authentication. */
const PairingForm: React.FC<PairingFormProps> = ({
  advancedOpen,
  error,
  hasPairingDescriptor,
  isLocked,
  password,
  passwordInputRef,
  relayUrl,
  remainingLockSeconds,
  requiresAccountAuth,
  showPassword,
  showSpinner,
  submitting,
  userId,
  usernameInputRef,
  onAdvancedOpenChange,
  onConnect,
  onOpenScanner,
  onPasswordChange,
  onRelayUrlChange,
  onShowPasswordChange,
  onUserIdChange,
}) => {
  const { t } = useI18n();
  const disabled = submitting || isLocked;

  return (
    <form className="pairing-page__form" onSubmit={(event) => { event.preventDefault(); onConnect(); }}>
      <div className="pairing-page__scroll">
        <div className="pairing-page__form-content">
          <h1 className="pairing-page__title">
            {requiresAccountAuth ? t('pairing.loginTitle') : t('pairing.connectTitle')}
          </h1>
          <p className="pairing-page__intro">
            {requiresAccountAuth ? t('pairing.loginDescription') : t('pairing.note')}
          </p>
          <div className={`pairing-page__credentials${requiresAccountAuth ? '' : ' pairing-page__credentials--single'}`}>
            <MobileTextField
              appearance="surface"
              className="pairing-page__field"
              leading={<span className="pairing-page__field-icon" aria-hidden="true">
                <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="8" r="4"/><path d="M4.5 21a7.5 7.5 0 0 1 15 0"/></svg>
              </span>}
              ref={usernameInputRef}
              inputClassName="pairing-page__input pairing-page__input--username"
              type="text"
              value={userId}
              onChange={(event) => onUserIdChange(event.target.value)}
              onAnimationStart={(event) => {
                if (event.animationName === 'pairingAutofillReconcile') onUserIdChange(event.currentTarget.value);
              }}
              placeholder={requiresAccountAuth ? t('pairing.usernamePlaceholder') : t('pairing.placeholder')}
              autoCapitalize="off"
              autoCorrect="off"
              autoComplete="username"
              maxLength={128}
              disabled={disabled}
            />
            {requiresAccountAuth && (
              <MobileTextField
                appearance="surface"
                aria-label={t('pairing.passwordLabel')}
                className="pairing-page__field pairing-page__password-field"
                leading={<span className="pairing-page__field-icon" aria-hidden="true">
                  <svg width="21" height="21" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><rect x="4" y="10" width="16" height="11" rx="2.5"/><path d="M8 10V7a4 4 0 0 1 8 0v3"/></svg>
                </span>}
                ref={passwordInputRef}
                id="pairing-password"
                inputClassName="pairing-page__input pairing-page__input--password"
                type={showPassword ? 'text' : 'password'}
                value={password}
                onChange={(event) => onPasswordChange(event.target.value)}
                onAnimationStart={(event) => {
                  if (event.animationName === 'pairingAutofillReconcile') onPasswordChange(event.currentTarget.value);
                }}
                placeholder={t('pairing.passwordPlaceholder')}
                autoComplete="current-password"
                maxLength={1024}
                disabled={disabled}
                trailing={<MobileIconButton
                  appearance="plain"
                  className="pairing-page__password-toggle"
                  aria-label={showPassword ? t('pairing.hidePassword') : t('pairing.showPassword')}
                  onClick={() => onShowPasswordChange(!showPassword)}
                  size="sm"
                  icon={<svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                    {showPassword ? (
                      <><path d="M2 12s3.5-6 10-6 10 6 10 6-3.5 6-10 6S2 12 2 12Z"/><circle cx="12" cy="12" r="2.6"/></>
                    ) : (
                      <><path d="m3 3 18 18"/><path d="M10.6 6.2A10.7 10.7 0 0 1 12 6c6.5 0 10 6 10 6a17.8 17.8 0 0 1-2.2 2.8"/><path d="M6.2 6.2C3.5 8 2 12 2 12s3.5 6 10 6a10 10 0 0 0 4-.8"/></>
                    )}
                  </svg>}
                />}
              />
            )}
          </div>
          <MobileDisclosure
            className="pairing-page__advanced"
            onToggle={() => onAdvancedOpenChange(!advancedOpen)}
            open={advancedOpen}
            title={t('pairing.advancedOptions')}
          >
            <div className="pairing-page__advanced-actions">
              <div className="pairing-page__relay-field">
                <span>{t('pairing.loginServer')}</span>
                <MobileTextField
                  appearance="surface"
                  type="url"
                  value={relayUrl}
                  placeholder={t('pairing.relayUrlPlaceholder')}
                  onChange={(event) => onRelayUrlChange(event.target.value)}
                  disabled={disabled}
                />
              </div>
            </div>
          </MobileDisclosure>
          {error && <MobileBanner className="pairing-page__error" tone="danger">{error}</MobileBanner>}
        </div>
      </div>
      <div className="pairing-page__action">
        <MobileButton
          appearance="primary"
          block
          className="pairing-page__retry"
          type="submit"
          disabled={disabled}
          loading={showSpinner}
        >
          {submitting
            ? t('pairing.connecting')
            : isLocked
              ? t('pairing.retryIn', { seconds: remainingLockSeconds })
              : requiresAccountAuth
                ? t('pairing.loginAction')
                : t('pairing.continue')}
        </MobileButton>
        {!hasPairingDescriptor && (
          <MobileButton
            appearance="plain"
            className="pairing-page__scan-action"
            onClick={onOpenScanner}
            disabled={submitting}
            leading={<svg width="21" height="21" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M3 8V5a2 2 0 0 1 2-2h3M16 3h3a2 2 0 0 1 2 2v3M21 16v3a2 2 0 0 1-2 2h-3M8 21H5a2 2 0 0 1-2-2v-3"/><path d="M8 8h8v8H8z"/></svg>}
          >
            {t('pairing.scanAction')}
          </MobileButton>
        )}
      </div>
    </form>
  );
};

export default PairingForm;
