 

import { useState, useEffect } from 'react';
import { notificationStore } from '../store/NotificationStore';
import { NotificationState } from '../types';

 
export function useNotificationState(): NotificationState {
  const [state, setState] = useState<NotificationState>(
    notificationStore.getState()
  );

  useEffect(() => {
    
    const unsubscribe = notificationStore.subscribe((newState) => {
      setState(newState);
    });

    return unsubscribe;
  }, []);

  return state;
}

 
export function useActiveNotifications() {
  const state = useNotificationState();
  return state.activeNotifications;
}

 
export function useNotificationHistory() {
  const state = useNotificationState();
  return state.notificationHistory;
}

 
export function useUnreadCount() {
  const state = useNotificationState();
  return state.unreadCount;
}

 
export function useCenterOpen() {
  const state = useNotificationState();
  return state.centerOpen;
}

 
export function useLatestProgressNotification() {
  const activeNotifications = useActiveNotifications();
  
  
  const progressNotifications = activeNotifications.filter(
    n => n.variant === 'progress' && n.status === 'active'
  );
  
  
  if (progressNotifications.length === 0) {
    return null;
  }
  
  return progressNotifications.reduce((latest, current) => {
    return current.timestamp > latest.timestamp ? current : latest;
  });
}

 
export function useLatestLoadingNotification() {
  const activeNotifications = useActiveNotifications();
  
  
  const loadingNotifications = activeNotifications.filter(
    n => n.variant === 'loading' && n.status === 'active'
  );
  
  
  if (loadingNotifications.length === 0) {
    return null;
  }
  
  return loadingNotifications.reduce((latest, current) => {
    return current.timestamp > latest.timestamp ? current : latest;
  });
}

 
export function useAllProgressNotifications() {
  const activeNotifications = useActiveNotifications();
  
  
  return activeNotifications.filter(
    n => n.variant === 'progress' && n.status === 'active'
  );
}

 
export function useAllLoadingNotifications() {
  const activeNotifications = useActiveNotifications();
  
  
  return activeNotifications.filter(
    n => n.variant === 'loading' && n.status === 'active'
  );
}

 
export function useLatestTaskNotification() {
  const activeNotifications = useActiveNotifications();
  
  
  const taskNotifications = activeNotifications.filter(
    n => (n.variant === 'progress' || n.variant === 'loading') && n.status === 'active'
  );
  
  
  if (taskNotifications.length === 0) {
    return null;
  }
  
  return taskNotifications.reduce((latest, current) => {
    return current.timestamp > latest.timestamp ? current : latest;
  });
}

