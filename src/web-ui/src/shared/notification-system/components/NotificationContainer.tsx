 

import React from 'react';
import { useActiveNotifications } from '../hooks/useNotificationState';
import { NotificationItem } from './NotificationItem';
import { ProgressNotification } from './ProgressNotification';
import { LoadingNotification } from './LoadingNotification';
import './NotificationContainer.scss';

export const NotificationContainer: React.FC = () => {
  const activeNotifications = useActiveNotifications();

  
  
  const visibleNotifications = activeNotifications.filter(
    n => n.variant !== 'silent' && n.variant !== 'progress' && n.variant !== 'loading'
  );

  if (visibleNotifications.length === 0) {
    return null;
  }

  return (
    <div className="notification-container">
      {visibleNotifications.map((notification) => {
        
        if (notification.variant === 'progress') {
          return (
            <ProgressNotification
              key={notification.id}
              notification={notification}
            />
          );
        }

        if (notification.variant === 'loading') {
          return (
            <LoadingNotification
              key={notification.id}
              notification={notification}
            />
          );
        }

        return (
          <NotificationItem
            key={notification.id}
            notification={notification}
          />
        );
      })}
    </div>
  );
};

