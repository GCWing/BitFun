 

import React, { useState, useMemo } from 'react';
import { CheckCheck, Loader2 } from 'lucide-react';
import {
  Icon,
  IconButton,
  ScrollArea,
  SearchField,
  Dialog,
  DialogBody,
  DialogHeader,
} from '@bitfun/ui';
import { useI18n } from '@/infrastructure/i18n';
import { useNotificationHistory, useCenterOpen, useAllProgressNotifications, useAllLoadingNotifications } from '../hooks/useNotificationState';
import { notificationService } from '../services/NotificationService';
import { NotificationFilter, NotificationRecord, Notification } from '../types';
import './NotificationCenter.scss';
export const NotificationCenter: React.FC = () => {
  const isOpen = useCenterOpen();
  const history = useNotificationHistory();
  const allProgressNotifications = useAllProgressNotifications();
  const allLoadingNotifications = useAllLoadingNotifications();
  const { t, formatDate } = useI18n(['components', 'common', 'errors']);
  const [filter, setFilter] = useState<NotificationFilter>('all');
  const [searchQuery, setSearchQuery] = useState('');
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const activeTaskNotifications = useMemo(() => {
    return [...allProgressNotifications, ...allLoadingNotifications];
  }, [allProgressNotifications, allLoadingNotifications]);

  
  const handleClose = React.useCallback(() => {
    notificationService.toggleCenter(false);
  }, []);

  
  const handleMarkAllRead = () => {
    notificationService.markAllAsRead();
  };

  
  const handleClearAll = () => {
    notificationService.clearHistory();
  };

  
  const handleDeleteNotification = (e: React.MouseEvent, notificationId: string) => {
    e.stopPropagation(); 
    notificationService.deleteFromHistory(notificationId);
  };

  
  const handleNotificationClick = (notification: NotificationRecord) => {
    
    setExpandedIds(prev => {
      const newSet = new Set(prev);
      if (newSet.has(notification.id)) {
        newSet.delete(notification.id);
      } else {
        newSet.add(notification.id);
      }
      return newSet;
    });

    
    if (!notification.read) {
      notificationService.markAsRead(notification.id);
    }
    
    
    if (notification.metadata?.onClick) {
      notification.metadata.onClick();
    }
  };

  
  const filteredHistory = useMemo(() => {
    let filtered = history;

    
    
    filtered = filtered.filter(n => {
      if (n.variant === 'progress' || n.variant === 'loading') {
        
        return n.status === 'completed' || n.status === 'failed' || n.status === 'cancelled';
      }
      return true; 
    });

    
    if (filter !== 'all') {
      filtered = filtered.filter(n => n.type === filter);
    }

    
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      filtered = filtered.filter(n =>
        n.title.toLowerCase().includes(query) ||
        n.message.toLowerCase().includes(query)
      );
    }

    return filtered;
  }, [history, filter, searchQuery]);

  
  const groupedHistory = useMemo(() => {
    const now = Date.now();
    const today = new Date(now).setHours(0, 0, 0, 0);
    const yesterday = today - 86400000;

    const groups = {
      today: [] as NotificationRecord[],
      yesterday: [] as NotificationRecord[],
      earlier: [] as NotificationRecord[]
    };

    filteredHistory.forEach(notification => {
      const notificationDate = new Date(notification.timestamp).setHours(0, 0, 0, 0);
      
      if (notificationDate === today) {
        groups.today.push(notification);
      } else if (notificationDate === yesterday) {
        groups.yesterday.push(notification);
      } else {
        groups.earlier.push(notification);
      }
    });

    return groups;
  }, [filteredHistory]);

  
  const formatTime = (timestamp: number) => {
    return formatDate(timestamp, { hour: '2-digit', minute: '2-digit' });
  };

  
  const getIcon = (type: string, status?: string) => {
    
    if (status === 'completed') {
      return '✓';
    }
    if (status === 'failed') {
      return '✕';
    }
    if (status === 'cancelled') {
      return '⊘';
    }
    
    
    switch (type) {
      case 'success':
        return '✓';
      case 'error':
        return '✕';
      case 'warning':
        return '⚠';
      case 'info':
      default:
        return 'ℹ';
    }
  };

  const getTechnicalDetails = (notification: NotificationRecord | Notification): string | null => {
    const metadata = notification.metadata;
    const aiError = metadata?.aiError;
    const diagnostics = normalizeMetadataString(aiError?.diagnostics ?? metadata?.diagnostics);
    const rawError = normalizeMetadataString(aiError?.rawError ?? metadata?.rawError);

    if (diagnostics && rawError && !diagnostics.includes(rawError)) {
      return `${diagnostics}\nraw_error=${rawError}`;
    }

    return diagnostics || (rawError ? `raw_error=${rawError}` : null);
  };

  
  const renderActiveTaskItem = (notification: Notification) => {
    const isProgress = notification.variant === 'progress';
    const isLoading = notification.variant === 'loading';
    
    
    const getProgressInfo = () => {
      if (isLoading) {
        return null;  
      }
      
      if (isProgress) {
        const mode = notification.progressMode || (notification.textOnly ? 'text-only' : 'percentage');
        if (mode === 'text-only') return null;
        
        if (mode === 'fraction' && notification.current !== undefined && notification.total !== undefined) {
          return `${notification.current}/${notification.total}`;
        }
        
        if (mode === 'percentage' && notification.progress !== undefined) {
          return `${Math.round(notification.progress)}%`;
        }
      }
      
      return null;
    };
    
    const progressInfo = getProgressInfo();
    
    return (
      <div
        key={notification.id}
        className="notification-center__active-task-item"
        data-bf-component="notification"
        data-bf-part="centerItem"
      >
        <div className="notification-center__active-task-icon">
          <Loader2 size={16} className="notification-center__spinner" />
        </div>
        <div className="notification-center__active-task-content">
          <div className="notification-center__active-task-header">
            <div className="notification-center__active-task-title">{notification.title}</div>
            {progressInfo && (
              <div className="notification-center__active-task-progress-text">{progressInfo}</div>
            )}
          </div>
          <div className="notification-center__active-task-message">
            {isProgress && notification.progressText ? notification.progressText : (notification.messageNode ?? notification.message)}
          </div>
          
          {isProgress && (() => {
            const mode = notification.progressMode || (notification.textOnly ? 'text-only' : 'percentage');
            if (mode === 'text-only') return null;
            
            return (
              <div className="notification-center__active-task-progress-bar">
                <div
                  className="notification-center__active-task-progress-fill"
                  style={{ transform: `scaleX(${Math.max(0, Math.min(100, notification.progress || 0)) / 100})` }}
                />
              </div>
            );
          })()}
        </div>
      </div>
    );
  };

  
  const renderNotificationItem = (notification: NotificationRecord) => {
    const isProgress = notification.variant === 'progress';
    const isLoading = notification.variant === 'loading';
    const iconClass = (isProgress || isLoading) && notification.status 
      ? `notification-center__item-icon--${notification.status}` 
      : `notification-center__item-icon--${notification.type}`;

    
    const now = Date.now();
    const today = new Date(now).setHours(0, 0, 0, 0);
    const yesterday = today - 86400000;
    const notificationDate = new Date(notification.timestamp).setHours(0, 0, 0, 0);
    
    const timeDisplay = notificationDate < yesterday
      ? formatDate(notification.timestamp, {
          month: '2-digit',
          day: '2-digit',
          hour: '2-digit',
          minute: '2-digit'
        })
      : formatTime(notification.timestamp);

    const isExpanded = expandedIds.has(notification.id);
    const technicalDetails = getTechnicalDetails(notification);

    return (
      <div
        key={notification.id}
        className={`notification-center__item ${!notification.read ? 'is-unread' : ''} ${isProgress ? 'is-progress' : ''} ${isLoading ? 'is-loading' : ''} ${isExpanded ? 'is-expanded' : ''}`}
        onClick={() => handleNotificationClick(notification)}
        data-notification-id={notification.id}
        data-notification-title={notification.title}
        data-notification-message={notification.message}
        data-notification-diagnostics={technicalDetails ?? undefined}
        data-context-type="notification"
        data-bf-component="notification"
        data-bf-part="centerItem"
        data-bf-state={`${!notification.read ? 'unread ' : ''}${isExpanded ? 'expanded' : ''}`.trim() || undefined}
      >
        <div className={`notification-center__item-icon ${iconClass}`}>
          {getIcon(notification.type, notification.status)}
        </div>
        <div className="notification-center__item-content">
          <div className="notification-center__item-header">
            <div className="notification-center__item-title">{notification.title}</div>
            
            {isProgress && (() => {
              const mode = notification.progressMode || (notification.textOnly ? 'text-only' : 'percentage');
              if (mode === 'text-only') return null;
              
              if (mode === 'fraction' && notification.current !== undefined && notification.total !== undefined) {
                return <div className="notification-center__item-percentage">{notification.current}/{notification.total}</div>;
              }
              
              if (mode === 'percentage' && notification.progress !== undefined) {
                return <div className="notification-center__item-percentage">{Math.round(notification.progress)}%</div>;
              }
              
              return null;
            })()}
          </div>
          <div className="notification-center__item-message">
            {(isProgress && notification.progressText) ? notification.progressText : (notification.messageNode ?? notification.message)}
          </div>
          
          {isProgress && (() => {
            const mode = notification.progressMode || (notification.textOnly ? 'text-only' : 'percentage');
            if (mode === 'text-only') return null;
            
            return (
              <div className="notification-center__item-progress-bar">
                <div
                  className={`notification-center__item-progress-fill ${notification.status ? `is-${notification.status}` : ''}`}
                  style={{ transform: `scaleX(${Math.max(0, Math.min(100, notification.progress || 0)) / 100})` }}
                />
              </div>
            );
          })()}
          <div className="notification-center__item-time">{timeDisplay}</div>
          {isExpanded && technicalDetails && (
            <div
              className="notification-center__item-technical-details"
              onClick={(event) => event.stopPropagation()}
            >
              <div className="notification-center__item-technical-title">
                {t('errors:boundary.technicalDetails')}
              </div>
              <ScrollArea className="notification-center__item-technical-body">
                <pre>{technicalDetails}</pre>
              </ScrollArea>
            </div>
          )}
        </div>
        {!notification.read && <div className="notification-center__item-badge" />}
        <div className="notification-center__item-actions">
          <button
            className="notification-center__item-expand"
            onClick={(e) => {
              e.stopPropagation();
              handleNotificationClick(notification);
            }}
            title={isExpanded ? t('common:actions.collapse') : t('common:actions.expand')}
          >
            {isExpanded ? <Icon name="chevron-up" size="sm" /> : <Icon name="chevron-down" size="sm" />}
          </button>
          <button
            className="notification-center__item-delete"
            onClick={(e) => handleDeleteNotification(e, notification.id)}
            title={t('common:actions.delete')}
          >
            <Icon name="xmark" size="md" />
          </button>
        </div>
      </div>
    );
  };

  return (
    <Dialog
      open={isOpen}
      onOpenChange={(nextOpen) => { if (!nextOpen) handleClose(); }}
      size="lg"
    >
      <DialogHeader>
      </DialogHeader>
      <DialogBody inset="none">
      <div className="notification-center" data-testid="notification-center" data-bf-component="notification" data-bf-part="centerRoot">
        
        <div className="notification-center__header" data-bf-component="notification" data-bf-part="centerHeader">
          <h2 className="notification-center__title">{t('components:notificationCenter.title')}</h2>
          <div className="notification-center__header-actions">
            <button
              type="button"
              className="notification-center__header-button"
              onClick={handleMarkAllRead}
              title={t('components:notificationCenter.actions.markAllRead')}
              aria-label={t('components:notificationCenter.actions.markAllRead')}
            >
              <CheckCheck size={16} />
            </button>
            <button
              type="button"
              className="notification-center__header-button"
              onClick={handleClearAll}
              title={t('components:notificationCenter.actions.clearAll')}
              aria-label={t('components:notificationCenter.actions.clearAll')}
            >
              <Icon name="delete" size="md" />
            </button>
            <IconButton
              className="notification-center__close"
              icon={<Icon name="xmark" size="lg" />}
              onClick={handleClose}
              size="md"
              title={t('common:actions.close')}
              aria-label={t('common:actions.close')}
              data-testid="notification-center-close-btn"
              data-bf-component="notification"
              data-bf-part="centerClose"
            />
          </div>
        </div>

        
        <div className="notification-center__search">
          <SearchField
            placeholder={t('components:notificationCenter.searchPlaceholder')}
            aria-label={t('components:notificationCenter.searchPlaceholder')}
            leadingIcon={<Icon name="search" size="md" aria-hidden />}
            value={searchQuery}
            onValueChange={(val) => setSearchQuery(val)}
            clearLabel={t('components:search.clear')}
            onClear={searchQuery ? () => setSearchQuery('') : undefined}
            size="md"
          />
        </div>

        
        <div className="notification-center__filters">
          <button
            className={`notification-center__filter ${filter === 'all' ? 'is-active' : ''}`}
            onClick={() => setFilter('all')}
          >
            {t('components:notificationCenter.filters.all', { count: history.length })}
          </button>
          <button
            className={`notification-center__filter ${filter === 'error' ? 'is-active' : ''}`}
            onClick={() => setFilter('error')}
          >
            {t('common:status.error')}
          </button>
          <button
            className={`notification-center__filter ${filter === 'warning' ? 'is-active' : ''}`}
            onClick={() => setFilter('warning')}
          >
            {t('common:status.warning')}
          </button>
          <button
            className={`notification-center__filter ${filter === 'info' ? 'is-active' : ''}`}
            onClick={() => setFilter('info')}
          >
            {t('common:status.info')}
          </button>
        </div>

        
        <ScrollArea className="notification-center__content" data-bf-component="notification" data-bf-part="centerList">
          
          {activeTaskNotifications.length > 0 && (
            <div className="notification-center__active-section" data-testid="notification-center-active-section">
              <div className="notification-center__active-section-title">
                {t('components:notificationCenter.activeTasks.title', { count: activeTaskNotifications.length })}
              </div>
              <div className="notification-center__active-section-list">
                {activeTaskNotifications.map(renderActiveTaskItem)}
              </div>
            </div>
          )}

          {filteredHistory.length === 0 && activeTaskNotifications.length === 0 ? (
            <div className="notification-center__empty">
              <div className="notification-center__empty-icon" />
              <div className="notification-center__empty-text">
                {searchQuery ? t('components:notificationCenter.empty.noMatches') : t('components:notificationCenter.empty.noNotifications')}
              </div>
            </div>
          ) : (
            <>
              
              {groupedHistory.today.length > 0 && (
                <div className="notification-center__group">
                  <div className="notification-center__group-title">{t('common:time.today')}</div>
                  {groupedHistory.today.map(renderNotificationItem)}
                </div>
              )}

              
              {groupedHistory.yesterday.length > 0 && (
                <div className="notification-center__group">
                  <div className="notification-center__group-title">{t('common:time.yesterday')}</div>
                  {groupedHistory.yesterday.map(renderNotificationItem)}
                </div>
              )}

              
              {groupedHistory.earlier.length > 0 && (
                <div className="notification-center__group">
                  <div className="notification-center__group-title">{t('components:notificationCenter.groups.earlier')}</div>
                  {groupedHistory.earlier.map(renderNotificationItem)}
                </div>
              )}
            </>
          )}
        </ScrollArea>
      </div>
          </DialogBody>
    </Dialog>
  );
};

function normalizeMetadataString(value: unknown): string | null {
  return typeof value === 'string' && value.trim() ? value.trim() : null;
}
