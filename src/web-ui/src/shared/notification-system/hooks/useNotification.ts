 

import { useCallback } from 'react';
import { notificationService } from '../services/NotificationService';
import {
  ToastOptions,
  ProgressOptions,
  PersistentOptions,
  SilentOptions,
  ProgressController
} from '../types';

export interface UseNotificationReturn {
   
  success: (message: string, options?: ToastOptions) => string;
   
  error: (message: string, options?: ToastOptions) => string;
   
  warning: (message: string, options?: ToastOptions) => string;
   
  info: (message: string, options?: ToastOptions) => string;
   
  progress: (options: ProgressOptions) => ProgressController;
   
  persistent: (options: PersistentOptions) => string;
   
  silent: (options: SilentOptions) => string;
   
  dismiss: (id: string) => void;
   
  dismissAll: () => void;
}

 
export function useNotification(): UseNotificationReturn {
  const success = useCallback((message: string, options?: ToastOptions) => {
    return notificationService.success(message, options);
  }, []);

  const error = useCallback((message: string, options?: ToastOptions) => {
    return notificationService.error(message, options);
  }, []);

  const warning = useCallback((message: string, options?: ToastOptions) => {
    return notificationService.warning(message, options);
  }, []);

  const info = useCallback((message: string, options?: ToastOptions) => {
    return notificationService.info(message, options);
  }, []);

  const progress = useCallback((options: ProgressOptions) => {
    return notificationService.progress(options);
  }, []);

  const persistent = useCallback((options: PersistentOptions) => {
    return notificationService.persistent(options);
  }, []);

  const silent = useCallback((options: SilentOptions) => {
    return notificationService.silent(options);
  }, []);

  const dismiss = useCallback((id: string) => {
    notificationService.dismiss(id);
  }, []);

  const dismissAll = useCallback(() => {
    notificationService.dismissAll();
  }, []);

  return {
    success,
    error,
    warning,
    info,
    progress,
    persistent,
    silent,
    dismiss,
    dismissAll
  };
}

