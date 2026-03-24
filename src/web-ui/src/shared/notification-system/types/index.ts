/**
 * Notification system types.
 *
 * Shared by the notification store, service, and UI components.
 */
import type { ReactNode } from 'react';

export type NotificationType = 'success' | 'error' | 'warning' | 'info';


export type NotificationVariant = 'toast' | 'progress' | 'persistent' | 'silent' | 'loading';


export type ProgressMode = 'percentage' | 'fraction' | 'text-only';


export type NotificationStatus = 'active' | 'dismissed' | 'completed' | 'failed' | 'cancelled';


export type ActionVariant = 'primary' | 'secondary' | 'danger';

/**
 * Action button rendered alongside a notification.
 */
export interface NotificationAction {
   
  label: string;
   
  onClick: () => void;
   
  variant?: ActionVariant;
}

/**
 * Canonical notification record stored in the notification store.
 */
export interface Notification {
   
  id: string;
   
  type: NotificationType;
   
  variant: NotificationVariant;
   
  title: string;
   
  message: string;

  /** When set, toast/history render this instead of plain `message` (keep `message` for search/plain fallback). */
  messageNode?: ReactNode;
   
  timestamp: number;
  
  
   
  progress?: number;
   
  progressText?: string;
   
  progressMode?: ProgressMode;
   
  current?: number;
   
  total?: number;
   
  textOnly?: boolean;
   
  cancellable?: boolean;
   
  onCancel?: () => void;
  
  
   
  actions?: NotificationAction[];
  
  
   
  metadata?: Record<string, any>;
  
  
   
  duration?: number;
   
  closable?: boolean;
  
  
   
  read?: boolean;
   
  status?: NotificationStatus;
}


export interface NotificationRecord extends Notification {
   
  dismissedAt?: number;
   
  showInCenter?: boolean;
}


export interface ToastOptions {
   
  title?: string;
   
  duration?: number;
   
  closable?: boolean;
   
  actions?: NotificationAction[];

  messageNode?: ReactNode;
   
  metadata?: Record<string, any>;
}


export interface ProgressOptions {
   
  title: string;
   
  message: string;
   
  initialProgress?: number;
   
  progressMode?: ProgressMode;
   
  initialCurrent?: number;
   
  total?: number;
   
  textOnly?: boolean;
   
  cancellable?: boolean;
   
  onCancel?: () => void;
   
  metadata?: Record<string, any>;
}


export interface PersistentOptions {
   
  type: NotificationType;
   
  title: string;
   
  message: string;
   
  actions?: NotificationAction[];
   
  closable?: boolean;
   
  metadata?: Record<string, any>;
}


export interface SilentOptions {
   
  title: string;
   
  message: string;
   
  type?: NotificationType;
   
  metadata?: Record<string, any>;
}


export interface LoadingOptions {
   
  title: string;
   
  message: string;
   
  cancellable?: boolean;
   
  onCancel?: () => void;
   
  metadata?: Record<string, any>;
}


export interface ProgressController {
   
  id: string;
   
  update(progress: number, text?: string): void;
   
  updateFraction(current: number, total?: number, text?: string): void;
   
  complete(message?: string): void;
   
  fail(message?: string): void;
   
  cancel(): void;
}


export interface LoadingController {
   
  id: string;
   
  updateMessage(message: string): void;
   
  complete(message?: string): void;
   
  fail(message?: string): void;
   
  cancel(): void;
}


export interface NotificationConfig {
   
  maxActiveNotifications: number;
   
  defaultDuration: number;
   
  enableSound: boolean;
   
  enableAnimation: boolean;
   
  position: 'top-right' | 'bottom-right' | 'top-left' | 'bottom-left';
}


export interface NotificationState {
   
  activeNotifications: Notification[];
   
  notificationHistory: NotificationRecord[];
   
  unreadCount: number;
   
  centerOpen: boolean;
   
  config: NotificationConfig;
}


export type NotificationFilter = 'all' | 'success' | 'error' | 'warning' | 'info';
