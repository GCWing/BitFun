 

import React from 'react';
import { X, CheckCircle, XCircle, AlertTriangle, Info } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';
import { Notification } from '../types';
import { notificationService } from '../services/NotificationService';
import './NotificationItem.scss';

export interface NotificationItemProps {
  notification: Notification;
  isExiting?: boolean;
}

export const NotificationItem: React.FC<NotificationItemProps> = ({ notification, isExiting = false }) => {
  const { id, type, title, message, messageNode, closable, actions } = notification;
  const isAssertive = type === 'error' || type === 'warning';
  const { t } = useI18n('common');

  
  const getIcon = () => {
    switch (type) {
      case 'success':
        return <CheckCircle size={14} />;
      case 'error':
        return <XCircle size={14} />;
      case 'warning':
        return <AlertTriangle size={14} />;
      case 'info':
      default:
        return <Info size={14} />;
    }
  };

  
  const handleClose = () => {
    notificationService.dismiss(id);
  };

  
  const handleAction = (onClick: () => void) => {
    onClick();
    
    if (closable) {
      notificationService.dismiss(id);
    }
  };

  return (
    <div
      className={`notification-item notification-item--${type}${isExiting ? ' notification-item--exiting' : ''}`}
      data-bf-component="notification"
      data-bf-part="item"
      role={isAssertive ? 'alert' : 'status'}
      aria-live={isAssertive ? 'assertive' : 'polite'}
      aria-atomic="true"
    >
      
      <div className="notification-item__icon" data-bf-component="notification" data-bf-part="itemIcon">
        {getIcon()}
      </div>

      
      <div className="notification-item__content" data-bf-component="notification" data-bf-part="itemContent">
        <div className="notification-item__title" data-bf-component="notification" data-bf-part="itemTitle">{title}</div>
        <div className="notification-item__message" data-bf-component="notification" data-bf-part="itemMessage">{messageNode ?? message}</div>

        
        {actions && actions.length > 0 && (
          <div className="notification-item__actions" data-bf-component="notification" data-bf-part="itemActions">
            {actions.map((action, index) => (
              <button
                key={index}
                className={`notification-item__action notification-item__action--${action.variant || 'secondary'}`}
                onClick={() => handleAction(action.onClick)}
              >
                {action.label}
              </button>
            ))}
          </div>
        )}
      </div>

      
      {closable && (
        <button
          className="notification-item__close"
          data-bf-component="notification"
          data-bf-part="itemClose"
          onClick={handleClose}
          aria-label={t('actions.close')}
        >
          <X size={14} />
        </button>
      )}
    </div>
  );
};
