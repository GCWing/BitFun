import React from 'react';
import { ArrowLeft, Circle, Inbox, RefreshCw } from 'lucide-react';
import { Button } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import type {
  FeedbackCategory,
  FeedbackRecordSummary,
  FeedbackStatus,
} from '@/infrastructure/api';
import {
  hasActionableUnreadReply,
  useFeedbackInboxStore,
} from './feedbackInboxStore';
import { FeedbackConversationView } from './FeedbackConversationView';

interface FeedbackInboxViewProps {
  wide: boolean;
  selectedId: string | null;
  onSelect: (feedbackId: string | null) => void;
  replySending: boolean;
  resetDraftVersion: number;
  onReplyStateChange: (state: { hasDraft: boolean; sending: boolean }) => void;
}

export const FeedbackInboxView: React.FC<FeedbackInboxViewProps> = ({
  wide,
  selectedId,
  onSelect,
  replySending,
  resetDraftVersion,
  onReplyStateChange,
}) => {
  const { t, formatDate } = useI18n('common');
  const records = useFeedbackInboxStore(state => state.records);
  const loaded = useFeedbackInboxStore(state => state.loaded);
  const loading = useFeedbackInboxStore(state => state.loading);
  const loadingMore = useFeedbackInboxStore(state => state.loadingMore);
  const hasMore = useFeedbackInboxStore(state => state.hasMore);
  const error = useFeedbackInboxStore(state => state.error);
  const refresh = useFeedbackInboxStore(state => state.refresh);
  const loadMore = useFeedbackInboxStore(state => state.loadMore);
  const selected = records.find(record => record.feedbackId === selectedId) ?? null;
  const showList = wide || !selected;
  const showDetail = wide || Boolean(selected);

  return (
    <div className={`bitfun-feedback__inbox-layout${wide ? ' is-wide' : ' is-narrow'}`}>
      {showList ? (
        <section className="bitfun-feedback__inbox-list" aria-label={t('feedback.inbox.title')}>
          <header className="bitfun-feedback__inbox-header">
            <div>
              <strong>{t('feedback.inbox.title')}</strong>
              <span>{t('feedback.inbox.count', { count: records.length })}</span>
            </div>
            <Button
              type="button"
              variant="ghost"
              size="small"
              disabled={loading || loadingMore || replySending}
              onClick={() => void refresh(true)}
            >
              <RefreshCw size={14} aria-hidden="true" />
              {t('feedback.inbox.refresh')}
            </Button>
          </header>
          {error ? (
            <div className="bitfun-feedback__inbox-error" role="alert">
              <span>{inboxErrorText(error.code, t)}</span>
            </div>
          ) : null}
          {loading && records.length === 0 ? (
            <div className="bitfun-feedback__inbox-state" role="status">
              {t('feedback.inbox.loading')}
            </div>
          ) : null}
          {!loading && loaded && records.length === 0 ? (
            <div className="bitfun-feedback__inbox-empty">
              <Inbox size={28} aria-hidden="true" />
              <strong>{t('feedback.inbox.emptyTitle')}</strong>
              <span>{t('feedback.inbox.emptyDescription')}</span>
            </div>
          ) : null}
          {records.length > 0 ? (
            <div className="bitfun-feedback__records" role="list">
              {records.map(record => (
                <button
                  key={record.feedbackId}
                  type="button"
                  role="listitem"
                  className={`bitfun-feedback__record${selectedId === record.feedbackId ? ' is-selected' : ''}`}
                  disabled={replySending}
                  onClick={() => onSelect(record.feedbackId)}
                >
                  <span className="bitfun-feedback__record-topline">
                    <strong>{categoryLabel(record.category, t)}</strong>
                    <span className={`bitfun-feedback__status is-${record.status}`}>
                      {statusLabel(record.status, t)}
                    </span>
                  </span>
                  <span className="bitfun-feedback__record-time">
                    {formatFeedbackDate(record.createdAt, formatDate)}
                  </span>
                  <span className="bitfun-feedback__record-bottomline">
                    <span>{t('feedback.inbox.updated', {
                      time: formatFeedbackDate(record.updatedAt, formatDate),
                    })}</span>
                    {hasActionableUnreadReply(record) ? (
                      <span className="bitfun-feedback__unread">
                        <Circle size={7} fill="currentColor" aria-hidden="true" />
                        {t('feedback.inbox.newReply')}
                      </span>
                    ) : null}
                  </span>
                </button>
              ))}
              {hasMore ? (
                <Button
                  type="button"
                  variant="secondary"
                  disabled={loading || loadingMore || replySending}
                  isLoading={loadingMore}
                  onClick={() => void loadMore()}
                >
                  {t('feedback.inbox.loadMore')}
                </Button>
              ) : null}
            </div>
          ) : null}
        </section>
      ) : null}
      {showDetail ? (
        <FeedbackSummaryDetail
          wide={wide}
          record={selected}
          onBack={() => onSelect(null)}
          replySending={replySending}
          resetDraftVersion={resetDraftVersion}
          onReplyStateChange={onReplyStateChange}
        />
      ) : null}
    </div>
  );
};

interface FeedbackSummaryDetailProps {
  wide: boolean;
  record: FeedbackRecordSummary | null;
  onBack: () => void;
  replySending: boolean;
  resetDraftVersion: number;
  onReplyStateChange: (state: { hasDraft: boolean; sending: boolean }) => void;
}

const FeedbackSummaryDetail: React.FC<FeedbackSummaryDetailProps> = ({
  wide,
  record,
  onBack,
  replySending,
  resetDraftVersion,
  onReplyStateChange,
}) => {
  const { t, formatDate } = useI18n('common');
  if (!record) {
    return (
      <section className="bitfun-feedback__detail is-empty">
        <Inbox size={28} aria-hidden="true" />
        <span>{t('feedback.inbox.selectRecord')}</span>
      </section>
    );
  }

  return (
    <section className="bitfun-feedback__detail" aria-label={t('feedback.inbox.detailTitle')}>
      <header className="bitfun-feedback__detail-header">
        {!wide ? (
          <button
            type="button"
            className="bitfun-feedback__back"
            disabled={replySending}
            onClick={onBack}
            aria-label={t('feedback.inbox.back')}
          >
            <ArrowLeft size={17} aria-hidden="true" />
          </button>
        ) : null}
        <div>
          <strong>{categoryLabel(record.category, t)}</strong>
          <span>{formatFeedbackDate(record.createdAt, formatDate)}</span>
        </div>
        <span className={`bitfun-feedback__status is-${record.status}`}>
          {statusLabel(record.status, t)}
        </span>
      </header>
      <div className="bitfun-feedback__detail-body">
        {record.canOpen ? (
          <FeedbackConversationView
            key={record.feedbackId}
            record={record}
            resetDraftVersion={resetDraftVersion}
            onInteractionStateChange={onReplyStateChange}
          />
        ) : (
          <div className="bitfun-feedback__inaccessible" role="status">
            <strong>{t('feedback.inbox.inaccessibleTitle')}</strong>
            <span>{t('feedback.inbox.inaccessibleDescription')}</span>
          </div>
        )}
      </div>
    </section>
  );
};

function categoryLabel(category: FeedbackCategory, t: (key: string) => string): string {
  const keys: Record<FeedbackCategory, string> = {
    runtime_error: 'feedback.categories.runtimeError',
    feature_request: 'feedback.categories.featureRequest',
    usage_question: 'feedback.categories.usageQuestion',
    other: 'feedback.categories.other',
  };
  return t(keys[category]);
}

function statusLabel(status: FeedbackStatus, t: (key: string) => string): string {
  const keys: Record<FeedbackStatus, string> = {
    submitted: 'feedback.inbox.status.submitted',
    in_progress: 'feedback.inbox.status.inProgress',
    waiting_user: 'feedback.inbox.status.waitingUser',
    resolved: 'feedback.inbox.status.resolved',
  };
  return t(keys[status]);
}

function inboxErrorText(code: string, t: (key: string) => string): string {
  if (code === 'FEEDBACK_ACCESS_UNAVAILABLE' || code === 'FEEDBACK_ACCESS_EXPIRED') {
    return t('feedback.inbox.accessUnavailable');
  }
  return t('feedback.inbox.refreshFailed');
}

function formatFeedbackDate(
  value: string,
  formatDate: (date: Date | number, options?: Intl.DateTimeFormatOptions) => string,
): string {
  const timestamp = Date.parse(value);
  if (Number.isNaN(timestamp)) return value;
  return formatDate(timestamp, {
    year: 'numeric',
    month: 'short',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
}
