 

import React, { useState, useMemo } from 'react';
import { X, CheckCheck, Trash2, XCircle, ChevronDown, ChevronUp, Loader2 } from 'lucide-react';
import { Search, Modal } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { useNotificationHistory, useCenterOpen, useAllProgressNotifications, useAllLoadingNotifications } from '../hooks/useNotificationState';
import { notificationService } from '../services/NotificationService';
import { NotificationFilter, NotificationRecord, Notification } from '../types';
import { createLogger } from '@/shared/utils/logger';
import './NotificationCenter.scss';

const log = createLogger('NotificationCenter');

export const NotificationCenter: React.FC = () => {
  const isOpen = useCenterOpen();
  const history = useNotificationHistory();
  const allProgressNotifications = useAllProgressNotifications();
  const allLoadingNotifications = useAllLoadingNotifications();
  const { t, formatDate } = useI18n(['components', 'common']);
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
            {isProgress && notification.progressText ? notification.progressText : notification.message}
          </div>
          
          {isProgress && (() => {
            const mode = notification.progressMode || (notification.textOnly ? 'text-only' : 'percentage');
            if (mode === 'text-only') return null;
            
            return (
              <div className="notification-center__active-task-progress-bar">
                <div
                  className="notification-center__active-task-progress-fill"
                  style={{ width: `${notification.progress || 0}%` }}
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

    return (
      <div
        key={notification.id}
        className={`notification-center__item ${!notification.read ? 'is-unread' : ''} ${isProgress ? 'is-progress' : ''} ${isLoading ? 'is-loading' : ''} ${isExpanded ? 'is-expanded' : ''}`}
        onClick={() => handleNotificationClick(notification)}
        data-notification-id={notification.id}
        data-notification-title={notification.title}
        data-notification-message={notification.message}
        data-context-type="notification"
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
            {(isProgress && notification.progressText) ? notification.progressText : notification.message}
          </div>
          
          {isProgress && (() => {
            const mode = notification.progressMode || (notification.textOnly ? 'text-only' : 'percentage');
            if (mode === 'text-only') return null;
            
            return (
              <div className="notification-center__item-progress-bar">
                <div
                  className={`notification-center__item-progress-fill ${notification.status ? `is-${notification.status}` : ''}`}
                  style={{ width: `${notification.progress || 0}%` }}
                />
              </div>
            );
          })()}
          <div className="notification-center__item-time">{timeDisplay}</div>
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
            {isExpanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
          </button>
          <button
            className="notification-center__item-delete"
            onClick={(e) => handleDeleteNotification(e, notification.id)}
            title={t('common:actions.delete')}
          >
            <XCircle size={16} />
          </button>
        </div>
      </div>
    );
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={handleClose}
      showCloseButton={false}
      size="large"
    >
      <div className="notification-center">
        
        <div className="notification-center__header">
          <h2 className="notification-center__title">{t('components:notificationCenter.title')}</h2>
          <div className="notification-center__header-actions">
            <button
              className="notification-center__header-button"
              onClick={handleMarkAllRead}
              title={t('components:notificationCenter.actions.markAllRead')}
            >
              <CheckCheck size={16} />
            </button>
            <button
              className="notification-center__header-button"
              onClick={handleClearAll}
              title={t('components:notificationCenter.actions.clearAll')}
            >
              <Trash2 size={16} />
            </button>
            <button
              className="notification-center__header-button"
              onClick={handleClose}
              title={t('common:actions.close')}
            >
              <X size={16} />
            </button>
          </div>
        </div>

        
        <div className="notification-center__search">
          <Search
            placeholder={t('components:notificationCenter.searchPlaceholder')}
            value={searchQuery}
            onChange={(val) => setSearchQuery(val)}
            clearable
            size="medium"
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

        
        <div className="notification-center__content">
          
          {activeTaskNotifications.length > 0 && (
            <div className="notification-center__active-section">
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
        </div>
      </div>
    </Modal>
  );
};
