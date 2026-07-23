 

import { 
  NotificationState, 
  Notification, 
  NotificationRecord,
  NotificationConfig 
} from '../types';
const DEFAULT_CONFIG: NotificationConfig = {
  maxActiveNotifications: 3,
  defaultDuration: 3000,
  enableSound: false,
  enableAnimation: true,
  position: 'bottom-left'
};


type Listener = (state: NotificationState) => void;

class NotificationStore {
  private state: NotificationState;
  private listeners = new Set<Listener>();

  constructor() {
    
    this.state = {
      activeNotifications: [],
      notificationHistory: [],
      unreadCount: 0,
      centerOpen: false,
      config: DEFAULT_CONFIG
    };
  }

   
  getState(): NotificationState {
    return { ...this.state };
  }

   
  subscribe(listener: Listener): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

   
  private notify(): void {
    this.listeners.forEach(listener => listener(this.state));
  }

   
  private setState(updates: Partial<NotificationState>): void {
    this.state = {
      ...this.state,
      ...updates
    };
    this.notify();
  }

   
  addNotification(notification: Notification): void {
    const activeNotifications = [...this.state.activeNotifications];
    
    
    if (activeNotifications.length >= this.state.config.maxActiveNotifications) {
      activeNotifications.shift();
    }
    
    activeNotifications.push(notification);

    
    
    const shouldAddToHistory = notification.variant !== 'progress' && notification.variant !== 'loading';
    
    if (shouldAddToHistory) {
      this.addToHistory(notification);
      
      
      if (notification.variant === 'silent') {
        this.setState({
          notificationHistory: this.state.notificationHistory,
          unreadCount: this.state.unreadCount + 1
        });
        return;
      }
    }

    
    const shouldIncreaseUnread = shouldAddToHistory;
    const unreadIncrement = shouldIncreaseUnread ? 1 : 0;
    
    this.setState({
      activeNotifications,
      unreadCount: this.state.unreadCount + unreadIncrement
    });
  }

   
  updateNotification(id: string, updates: Partial<Notification>): void {
    
    const activeNotifications = this.state.activeNotifications.map(n =>
      n.id === id ? { ...n, ...updates } : n
    );

    
    const updatedNotification = activeNotifications.find(n => n.id === id);
    
    
    let notificationHistory = [...this.state.notificationHistory];
    const unreadCount = this.state.unreadCount;
    
    if (updatedNotification && 
        (updatedNotification.variant === 'progress' || updatedNotification.variant === 'loading')) {
      const isFinished = updates.status === 'completed' || 
                        updates.status === 'failed' || 
                        updates.status === 'cancelled';
      
      if (isFinished) {
        
        const existingIndex = notificationHistory.findIndex(n => n.id === id);
        
        if (existingIndex === -1) {
          
          const record: NotificationRecord = {
            ...updatedNotification,
            showInCenter: true
          };
          notificationHistory = [record, ...notificationHistory];
          
          
          if (notificationHistory.length > 100) {
            notificationHistory.splice(100);
          }
          
          
        } else {
          
          notificationHistory = notificationHistory.map(n =>
            n.id === id ? { ...n, ...updates } : n
          );
        }
      }
    } else {
      
      notificationHistory = notificationHistory.map(n =>
        n.id === id ? { ...n, ...updates } : n
      );
    }

    this.setState({
      activeNotifications,
      notificationHistory,
      unreadCount
    });
  }

   
  removeNotification(id: string): void {
    
    const notificationToRemove = this.state.activeNotifications.find(n => n.id === id);
    const activeNotifications = this.state.activeNotifications.filter(n => n.id !== id);
    
    let notificationHistory = [...this.state.notificationHistory];
    
    
    if (notificationToRemove && 
        (notificationToRemove.variant === 'progress' || notificationToRemove.variant === 'loading')) {
      
      const existingIndex = notificationHistory.findIndex(n => n.id === id);
      if (existingIndex !== -1) {
        notificationHistory = notificationHistory.map(n =>
          n.id === id ? { ...n, status: 'dismissed' as const, dismissedAt: Date.now() } : n
        );
      }
      
    } else {
      
      notificationHistory = notificationHistory.map(n =>
        n.id === id ? { ...n, status: 'dismissed' as const, dismissedAt: Date.now() } : n
      );
    }

    this.setState({
      activeNotifications,
      notificationHistory
    });
  }

   
  clearActiveNotifications(): void {
    this.setState({
      activeNotifications: []
    });
  }

   
  private addToHistory(notification: Notification): void {
    const record: NotificationRecord = {
      ...notification,
      showInCenter: true
    };

    const notificationHistory = [record, ...this.state.notificationHistory];

    
    if (notificationHistory.length > 100) {
      notificationHistory.splice(100);
    }

    this.state.notificationHistory = notificationHistory;
  }

   
  markAsRead(id: string): void {
    const notificationHistory = this.state.notificationHistory.map(n =>
      n.id === id && !n.read ? { ...n, read: true } : n
    );

    const unreadCount = notificationHistory.filter(n => !n.read).length;

    this.setState({
      notificationHistory,
      unreadCount
    });
  }

   
  markAllAsRead(): void {
    const notificationHistory = this.state.notificationHistory.map(n => ({
      ...n,
      read: true
    }));

    this.setState({
      notificationHistory,
      unreadCount: 0
    });
  }

   
  removeFromHistory(id: string): void {
    const notificationHistory = this.state.notificationHistory.filter(n => n.id !== id);
    const unreadCount = notificationHistory.filter(n => !n.read).length;

    this.setState({
      notificationHistory,
      unreadCount
    });
  }

   
  clearHistory(): void {
    this.setState({
      notificationHistory: [],
      unreadCount: 0
    });
  }

   
  toggleCenter(open?: boolean): void {
    const newState = open !== undefined ? open : !this.state.centerOpen;
    this.setState({
      centerOpen: newState
    });
  }

   
  updateConfig(config: Partial<NotificationConfig>): void {
    this.setState({
      config: {
        ...this.state.config,
        ...config
      }
    });
  }
}


export const notificationStore = new NotificationStore();
