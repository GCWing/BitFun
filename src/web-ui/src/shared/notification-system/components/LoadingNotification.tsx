 

import React from 'react';
import { X, Loader2 } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';
import { Notification } from '../types';
import { notificationService } from '../services/NotificationService';
import './LoadingNotification.scss';

export interface LoadingNotificationProps {
  notification: Notification;
}

export const LoadingNotification: React.FC<LoadingNotificationProps> = ({ notification }) => {
  const { id, title, message, cancellable, onCancel, status } = notification;
  const { t } = useI18n('common');

  
  const handleCancel = () => {
    if (onCancel) {
      onCancel();
    }
    notificationService.dismiss(id);
  };

  
  const getStatusIcon = () => {
    if (status === 'completed') {
      return <span className="loading-notification__status-icon loading-notification__status-icon--success">✓</span>;
    }
    if (status === 'failed') {
      return <span className="loading-notification__status-icon loading-notification__status-icon--error">✕</span>;
    }
    return <Loader2 size={16} className="loading-notification__spinner" />;
  };

  return (
    <div className={`loading-notification loading-notification--${status || 'active'}`}>
      
      <div className="loading-notification__icon">
        {getStatusIcon()}
      </div>

      
      <div className="loading-notification__content">
        <div className="loading-notification__title">{title}</div>
        <div className="loading-notification__message">{message}</div>
      </div>

      
      {cancellable && status === 'active' && (
        <button
          className="loading-notification__cancel"
          onClick={handleCancel}
          aria-label={t('actions.cancel')}
        >
          <X size={14} />
        </button>
      )}
    </div>
  );
};

