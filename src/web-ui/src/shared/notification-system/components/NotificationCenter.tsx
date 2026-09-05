import React, { useMemo, useState } from 'react';
import { AlertTriangle, Ban, CheckCheck, Loader2, XCircle } from 'lucide-react';
import {
  Card,
  CardBody,
  Dialog,
  DialogBody,
  DialogClose,
  DialogHeader,
  DialogHeaderActions,
  DialogHeading,
  DialogTitle,
  Disclosure,
  Icon,
  IconButton,
  OverflowText,
  ScrollArea,
  SearchField,
} from '@openbitfun/ui';
import { useI18n } from '@/infrastructure/i18n';
import { useNotificationState } from '../hooks/useNotificationState';
import { notificationService } from '../services/NotificationService';
import type { Notification } from '../types';
import './NotificationCenter.scss';

export const NotificationCenter: React.FC = () => {
  const { centerOpen: isOpen, notificationHistory: history, activeNotifications } = useNotificationState();
  const { t, formatDate, formatNumber } = useI18n(['components', 'common', 'errors']);
  const [searchQuery, setSearchQuery] = useState('');
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const defaultTitles = {
    success: t('common:status.success'),
    error: t('common:status.error'),
    warning: t('common:status.warning'),
    info: t('common:status.info'),
  };

  const notifications = useMemo(() => {
    const activeTasks = activeNotifications.filter(isActiveTask);
    const activeTaskIds = new Set(activeTasks.map(notification => notification.id));
    const completedHistory = history.filter(notification => {
      if (activeTaskIds.has(notification.id)) return false;
      if (notification.variant === 'progress' || notification.variant === 'loading') {
        return notification.status === 'completed'
          || notification.status === 'failed'
          || notification.status === 'cancelled';
      }
      return true;
    });
    const query = searchQuery.trim().toLowerCase();

    return [...activeTasks, ...completedHistory]
      .filter(notification => !query
        || notification.title.toLowerCase().includes(query)
        || notification.message.toLowerCase().includes(query)
        || notification.progressText?.toLowerCase().includes(query))
      .sort((left, right) => right.timestamp - left.timestamp);
  }, [activeNotifications, history, searchQuery]);

  const handleClose = () => {
    notificationService.toggleCenter(false);
  };

  const handleNotificationOpenChange = (notification: Notification, open: boolean) => {
    setExpandedIds(previous => {
      const next = new Set(previous);
      if (open) next.add(notification.id);
      else next.delete(notification.id);
      return next;
    });

    if (!isActiveTask(notification)) {
      if (!notification.read) notificationService.markAsRead(notification.id);
      notification.metadata?.onClick?.();
    }
  };

  const formatTime = (timestamp: number) => {
    const now = new Date();
    const date = new Date(timestamp);
    const isToday = date.getFullYear() === now.getFullYear()
      && date.getMonth() === now.getMonth()
      && date.getDate() === now.getDate();

    return formatDate(timestamp, {
      ...(isToday ? {} : { month: '2-digit', day: '2-digit' }),
      ...(date.getFullYear() === now.getFullYear() ? {} : { year: 'numeric' }),
      hour: '2-digit',
      minute: '2-digit',
    });
  };

  const renderNotificationItem = (notification: Notification) => {
    const isProgress = notification.variant === 'progress';
    const isTask = isProgress || notification.variant === 'loading';
    const isActive = isActiveTask(notification);
    const isUnread = !isActive && !notification.read;
    const isExpanded = expandedIds.has(notification.id);
    const status = isTask && notification.status ? notification.status : notification.type;
    const technicalDetails = getTechnicalDetails(notification);
    const usesMessageAsTitle = !isTask
      && notification.messageNode == null
      && Boolean(notification.message.trim())
      && notification.title.trim() === defaultTitles[notification.type];
    const title = usesMessageAsTitle ? notification.message : notification.title;
    const messageText = isProgress && notification.progressText
      ? notification.progressText
      : notification.message;
    const message = isProgress && notification.progressText
      ? notification.progressText
      : (notification.messageNode ?? notification.message);
    const mode = notification.progressMode || (notification.textOnly ? 'text-only' : 'percentage');
    const showProgress = isProgress && mode !== 'text-only';
    const progress = Math.max(0, Math.min(100, notification.progress || 0));
    const progressInfo = !showProgress ? null
      : mode === 'fraction' && notification.current !== undefined && notification.total !== undefined
        ? formatNumber(notification.current) + '/' + formatNumber(notification.total)
        : notification.progress !== undefined
          ? formatNumber(progress / 100, { style: 'percent', maximumFractionDigits: 0 })
          : null;

    return (
      <li
        key={notification.id}
        className="notification-center__item"
        data-notification-id={notification.id}
        data-notification-title={notification.title}
        data-notification-message={notification.message}
        data-notification-diagnostics={technicalDetails ?? undefined}
        data-context-type={isActive ? undefined : 'notification'}
        data-openbitfun-component="notification"
        data-openbitfun-part="centerItem"
        data-openbitfun-state={[isUnread && 'unread', isExpanded && 'expanded'].filter(Boolean).join(' ') || undefined}
      >
        <Card className="notification-center__item-card" appearance="neutral">
          <Disclosure
            className="notification-center__item-disclosure"
            open={isExpanded}
            onOpenChange={open => handleNotificationOpenChange(notification, open)}
            summary={<span title={title}>{title}</span>}
            description={!usesMessageAsTitle && messageText
              ? <OverflowText>{messageText}</OverflowText>
              : undefined}
            leading={
              <span className={'notification-center__item-status notification-center__item-status--' + status} title={notification.title}>
                {getNotificationIcon(notification)}
              </span>
            }
            actions={
              <>
                {isUnread && <span className="notification-center__item-badge" aria-hidden="true" />}
                <time
                  className="notification-center__item-time"
                  dateTime={new Date(notification.timestamp).toISOString()}
                  title={formatDate(notification.timestamp, { dateStyle: 'full', timeStyle: 'short' })}
                >
                  {formatTime(notification.timestamp)}
                </time>
                {!isActive && (
                  <span className="notification-center__item-action">
                    <IconButton
                      icon={<Icon name="xmark" size="sm" />}
                      size="xs"
                      onClick={() => notificationService.deleteFromHistory(notification.id)}
                      title={t('common:actions.delete')}
                      aria-label={t('common:actions.delete')}
                    />
                  </span>
                )}
              </>
            }
          >
            <CardBody className="notification-center__item-details">
              {message && <div className="notification-center__item-detail-message">{message}</div>}
              {technicalDetails && (
                <div className="notification-center__item-technical-details">
                  <div className="notification-center__item-technical-title">
                    {t('errors:boundary.technicalDetails')}
                  </div>
                  <ScrollArea className="notification-center__item-technical-body">
                    <pre>{technicalDetails}</pre>
                  </ScrollArea>
                </div>
              )}
            </CardBody>
          </Disclosure>
          {showProgress && (
            <div className="notification-center__item-progress">
              <div
                className="notification-center__item-progress-bar"
                role="progressbar"
                aria-label={notification.title}
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={progress}
                aria-valuetext={progressInfo ?? undefined}
              >
                <div
                  className={'notification-center__item-progress-fill is-' + status}
                  style={{ transform: 'scaleX(' + progress / 100 + ')' }}
                />
              </div>
              {progressInfo && <span className="notification-center__item-percentage">{progressInfo}</span>}
            </div>
          )}
        </Card>
      </li>
    );
  };

  return (
    <Dialog
      open={isOpen}
      onOpenChange={(nextOpen) => { if (!nextOpen) handleClose(); }}
      size="md"
      aria-label={t('components:notificationCenter.title')}
    >
      <div
        className="notification-center"
        data-testid="notification-center"
        data-openbitfun-component="notification"
        data-openbitfun-part="centerRoot"
      >
        <div className="notification-center__header" data-openbitfun-component="notification" data-openbitfun-part="centerHeader">
          <DialogHeader className="notification-center__dialog-header">
            <DialogHeading>
              <DialogTitle>{t('components:notificationCenter.title')}</DialogTitle>
            </DialogHeading>
            <DialogHeaderActions>
              <IconButton
                icon={<CheckCheck size={16} />}
                onClick={() => notificationService.markAllAsRead()}
                disabled={!history.some(notification => !notification.read)}
                title={t('components:notificationCenter.actions.markAllRead')}
                aria-label={t('components:notificationCenter.actions.markAllRead')}
              />
              <IconButton
                icon={<Icon name="delete" size="md" />}
                onClick={() => notificationService.clearHistory()}
                disabled={history.length === 0}
                title={t('components:notificationCenter.actions.clearAll')}
                aria-label={t('components:notificationCenter.actions.clearAll')}
              />
              <span className="notification-center__close" data-openbitfun-component="notification" data-openbitfun-part="centerClose">
                <DialogClose
                  title={t('common:actions.close')}
                  aria-label={t('common:actions.close')}
                  data-testid="notification-center-close-btn"
                />
              </span>
            </DialogHeaderActions>
          </DialogHeader>
        </div>
        <DialogBody className="notification-center__body" inset="none">
          <div className="notification-center__search">
            <SearchField
              placeholder={t('components:notificationCenter.searchPlaceholder')}
              aria-label={t('components:notificationCenter.searchPlaceholder')}
              leadingIcon={<Icon name="search" size="md" aria-hidden />}
              value={searchQuery}
              onValueChange={setSearchQuery}
              clearLabel={t('components:search.clear')}
              onClear={searchQuery ? () => setSearchQuery('') : undefined}
              size="md"
            />
          </div>
          <ScrollArea className="notification-center__content" data-openbitfun-component="notification" data-openbitfun-part="centerList">
            {notifications.length === 0 ? (
              <div className="notification-center__empty" role="status">
                <div className="notification-center__empty-icon" aria-hidden="true">
                  <Icon name={searchQuery.trim() ? 'search' : 'bell'} size="lg" />
                </div>
                <div className="notification-center__empty-text">
                  {searchQuery.trim()
                    ? t('components:notificationCenter.empty.noMatches')
                    : t('components:notificationCenter.empty.noNotifications')}
                </div>
              </div>
            ) : (
              <ul className="notification-center__list" aria-label={t('components:notificationCenter.title')}>
                {notifications.map(renderNotificationItem)}
              </ul>
            )}
          </ScrollArea>
        </DialogBody>
      </div>
    </Dialog>
  );
};

function isActiveTask(notification: Notification): boolean {
  return (notification.variant === 'progress' || notification.variant === 'loading')
    && notification.status === 'active';
}

function getNotificationIcon(notification: Notification) {
  if (isActiveTask(notification)) return <Loader2 size={14} className="notification-center__spinner" />;
  if (notification.status === 'completed') return <Icon name="check-circle" size="sm" />;
  if (notification.status === 'failed') return <XCircle size={14} />;
  if (notification.status === 'cancelled') return <Ban size={14} />;

  switch (notification.type) {
    case 'success': return <Icon name="check-circle" size="sm" />;
    case 'error': return <XCircle size={14} />;
    case 'warning': return <AlertTriangle size={14} />;
    default: return <Icon name="info" size="sm" />;
  }
}

function getTechnicalDetails(notification: Notification): string | null {
  const metadata = notification.metadata;
  const aiError = metadata?.aiError;
  const diagnostics = normalizeMetadataString(aiError?.diagnostics ?? metadata?.diagnostics);
  const rawError = normalizeMetadataString(aiError?.rawError ?? metadata?.rawError);

  if (diagnostics && rawError && !diagnostics.includes(rawError)) {
    return diagnostics + '\nraw_error=' + rawError;
  }

  return diagnostics || (rawError ? 'raw_error=' + rawError : null);
}

function normalizeMetadataString(value: unknown): string | null {
  return typeof value === 'string' && value.trim() ? value.trim() : null;
}
