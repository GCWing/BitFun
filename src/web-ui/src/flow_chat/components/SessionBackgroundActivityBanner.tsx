import React from 'react';
import { AlertCircle, CheckCircle2, Loader2, Sparkles } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { SessionBackgroundActivity } from '../types/flow-chat';
import './SessionBackgroundActivityBanner.scss';

export interface SessionBackgroundActivityBannerProps {
  activity: SessionBackgroundActivity;
}

function renderStatusIcon(activity: SessionBackgroundActivity): React.ReactNode {
  switch (activity.status) {
    case 'running':
      return <Loader2 size={15} className="bitfun-session-background-activity__icon-spin" />;
    case 'completed':
      return <CheckCircle2 size={15} />;
    case 'failed':
      return <AlertCircle size={15} />;
    default:
      return <Sparkles size={15} />;
  }
}

export const SessionBackgroundActivityBanner: React.FC<SessionBackgroundActivityBannerProps> = ({
  activity,
}) => {
  const { t } = useTranslation('flow-chat');

  const copy = (() => {
    switch (activity.kind) {
      case 'auto_memory':
        switch (activity.status) {
          case 'running':
            return {
              title: t('chatInput.backgroundActivities.autoMemory.running'),
            };
          case 'completed':
            return {
              title: t('chatInput.backgroundActivities.autoMemory.completed'),
            };
          case 'failed':
            return {
              title: t('chatInput.backgroundActivities.autoMemory.failed'),
            };
        }
    }
  })() || {
    title: t('chatInput.backgroundActivities.autoMemory.running'),
  };

  return (
    <div
      className={`bitfun-session-background-activity bitfun-session-background-activity--${activity.status}`}
      role="status"
      aria-live="polite"
    >
      <div className="bitfun-session-background-activity__icon" aria-hidden="true">
        {renderStatusIcon(activity)}
      </div>
      <div className="bitfun-session-background-activity__body">
        <div className="bitfun-session-background-activity__title">{copy.title}</div>
        {activity.detail ? (
          <div className="bitfun-session-background-activity__detail">{activity.detail}</div>
        ) : null}
      </div>
    </div>
  );
};

export default SessionBackgroundActivityBanner;
