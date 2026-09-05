import React from 'react';
import { useI18n } from '../i18n';
import { MobileBanner, MobileIconButton } from '@openbitfun/ui/mobile';

interface ChatFeedbackProps {
  actionMessage: string | null;
  errorMessage: string | null;
  infoMessage: string | null;
  onDismissError: () => void;
  onDismissInfo: () => void;
}

export default function ChatFeedback({ actionMessage, errorMessage, infoMessage, onDismissError, onDismissInfo }: ChatFeedbackProps) {
  const { t } = useI18n();
  const closeAction = (onClose: () => void) => <MobileIconButton appearance="plain" onClick={onClose} aria-label={t('common.close')} icon={<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18" /></svg>} />;
  return (
    <>
      {actionMessage && <MobileBanner className="chat-page__toast" role="status" aria-live="polite" tone="neutral">{actionMessage}</MobileBanner>}
      {errorMessage && <MobileBanner className="chat-page__toast" action={closeAction(onDismissError)} role="alert" tone="danger">{errorMessage}</MobileBanner>}
      {infoMessage && <MobileBanner className="chat-page__toast" action={closeAction(onDismissInfo)} role="status" tone="info">{infoMessage}</MobileBanner>}
    </>
  );
}
