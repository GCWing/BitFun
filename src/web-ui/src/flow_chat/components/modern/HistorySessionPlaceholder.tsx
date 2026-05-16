import React from 'react';
import { AlertCircle, LoaderCircle, RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { SessionHistoryState } from '../../types/flow-chat';

interface HistorySessionPlaceholderProps {
  state: Extract<SessionHistoryState, 'metadata-only' | 'hydrating' | 'failed'>;
  onRetry?: () => void;
}

export const HistorySessionPlaceholder: React.FC<HistorySessionPlaceholderProps> = ({
  state,
  onRetry,
}) => {
  const { t } = useTranslation('flow-chat');
  const failed = state === 'failed';

  return (
    <div className="history-session-placeholder" role={failed ? 'alert' : 'status'}>
      <div
        className={`history-session-placeholder__icon${failed ? ' history-session-placeholder__icon--failed' : ''}`}
        aria-hidden="true"
      >
        {failed ? <AlertCircle size={24} /> : <LoaderCircle size={24} />}
      </div>
      <div className="history-session-placeholder__text">
        <h2 className="history-session-placeholder__title">
          {failed ? t('historyState.failedTitle') : t('historyState.loadingTitle')}
        </h2>
        <p className="history-session-placeholder__description">
          {failed ? t('historyState.failedDescription') : t('historyState.loadingDescription')}
        </p>
      </div>
      {failed && (
        <button
          type="button"
          className="history-session-placeholder__retry"
          onClick={onRetry}
        >
          <RefreshCw size={14} aria-hidden="true" />
          <span>{t('historyState.retry')}</span>
        </button>
      )}
    </div>
  );
};

HistorySessionPlaceholder.displayName = 'HistorySessionPlaceholder';
