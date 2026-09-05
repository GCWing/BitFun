import React from 'react';
import { MobileBanner } from '@openbitfun/ui/mobile';

interface ChatFeedbackProps {
  actionMessage: string | null;
  errorMessage: string | null;
  infoMessage: string | null;
  onDismissError: () => void;
  onDismissInfo: () => void;
}

export default function ChatFeedback({ actionMessage, errorMessage, infoMessage, onDismissError, onDismissInfo }: ChatFeedbackProps) {
  return (
    <>
      {actionMessage && <MobileBanner className="chat-page__toast" role="alert" aria-live="assertive">{actionMessage}</MobileBanner>}
      {errorMessage && <MobileBanner className="chat-page__toast" onClick={onDismissError} tone="danger">{errorMessage}</MobileBanner>}
      {infoMessage && <MobileBanner className="chat-page__toast chat-page__toast--info" onClick={onDismissInfo} tone="info">{infoMessage}</MobileBanner>}
    </>
  );
}
