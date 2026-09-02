import { Icon } from '@bitfun/ui';
import React from 'react';
import { AlertCircle, AlertTriangle } from 'lucide-react';
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
      return <Icon name="info" size="md" />;
    case 'warning':
    default:
      return <AlertTriangle size={16} />;
  }
}

export const TurnCompletionNoticeItem: React.FC<TurnCompletionNoticeItemProps> = ({ notice }) => {
  const { t } = useI18n('flow-chat');
  const body = notice.bodyKey ? t(notice.bodyKey) : null;

  return (
    <div data-bf-component="turn-completion-notice" data-bf-part="root" data-bf-tone={notice.tone}
      className={[
        'turn-completion-notice',
        `turn-completion-notice--${notice.tone}`,
      ].join(' ')}
      role="note"
      aria-label={t(notice.titleKey)}
    >
      <div data-bf-component="turn-completion-notice" data-bf-part="icon" className="turn-completion-notice__icon" aria-hidden="true">
        {getNoticeIcon(notice.tone)}
      </div>
      <div data-bf-component="turn-completion-notice" data-bf-part="content" className="turn-completion-notice__content">
        <div data-bf-component="turn-completion-notice" data-bf-part="text" className="turn-completion-notice__text">
          <span data-bf-component="turn-completion-notice" data-bf-part="title" className="turn-completion-notice__title">{t(notice.titleKey)}</span>
          {body ? (
            <>
              <span className="turn-completion-notice__separator" aria-hidden="true">·</span>
              <span data-bf-component="turn-completion-notice" data-bf-part="body" className="turn-completion-notice__body">{body}</span>
            </>
          ) : null}
        </div>
        <span data-bf-component="turn-completion-notice" data-bf-part="reason" className="turn-completion-notice__reason">
          <code data-bf-component="turn-completion-notice" data-bf-part="reasonCode" className="turn-completion-notice__reason-code">{notice.reasonCode}</code>
        </span>
      </div>
    </div>
  );
};
