import React from 'react';
import { AlertCircle, AlertTriangle, Info } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';
import type { TurnCompletionNotice } from '../../utils/turnCompletionNotice';
import './TurnCompletionNoticeItem.scss';

interface TurnCompletionNoticeItemProps {
  notice: TurnCompletionNotice;
}

function getNoticeIcon(tone: TurnCompletionNotice['tone']): React.ReactNode {
  switch (tone) {
    case 'error':
      return <AlertCircle size={16} />;
    case 'info':
      return <Info size={16} />;
    case 'warning':
    default:
      return <AlertTriangle size={16} />;
  }
}

export const TurnCompletionNoticeItem: React.FC<TurnCompletionNoticeItemProps> = ({ notice }) => {
  const { t } = useI18n('flow-chat');
  const body = notice.bodyKey ? t(notice.bodyKey) : null;

  return (
    <div
      className={[
        'turn-completion-notice',
        `turn-completion-notice--${notice.tone}`,
      ].join(' ')}
      role="note"
      aria-label={t(notice.titleKey)}
    >
      <div className="turn-completion-notice__icon" aria-hidden="true">
        {getNoticeIcon(notice.tone)}
      </div>
      <div className="turn-completion-notice__content">
        <div className="turn-completion-notice__text">
          <span className="turn-completion-notice__title">{t(notice.titleKey)}</span>
          {body ? (
            <>
              <span className="turn-completion-notice__separator" aria-hidden="true">·</span>
              <span className="turn-completion-notice__body">{body}</span>
            </>
          ) : null}
        </div>
        <span className="turn-completion-notice__reason">
          <code className="turn-completion-notice__reason-code">{notice.reasonCode}</code>
        </span>
      </div>
    </div>
  );
};
