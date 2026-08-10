import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { CheckCircle2, ExternalLink, List, Send, SquarePen } from 'lucide-react';
import {
  Button,
  Checkbox,
  confirmDialog,
  ConfirmDialog,
  Modal,
  Select,
  Switch,
  Textarea,
} from '@/component-library';
import { PrivacyStatementDialog } from '@/app/components/Privacy/PrivacyStatementDialog';
import { usePrivacy } from '@/app/components/Privacy/PrivacyContext';
import {
  feedbackAPI,
  FeedbackApiError,
  feedbackContentLength,
  systemAPI,
  truncateFeedbackContent,
  type FeedbackCategory,
} from '@/infrastructure/api';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { createLogger } from '@/shared/utils/logger';
import { registerCriticalOperationExitGuard } from '@/shared/services/criticalOperationExitGuard';
import { FeedbackInboxView } from './FeedbackInboxView';
import { PrivacyStatementLink } from './PrivacyStatementLink';
import { useFeedbackInboxStore } from './feedbackInboxStore';
import './FeedbackDialog.scss';

const log = createLogger('FeedbackDialog');
const GITCODE_ISSUES_URL = 'https://gitcode.com/OpenHarmonyPCDeveloper/BitFun/issues';
let submissionRetryUntilMs = 0;

function submissionRetrySecondsRemaining(): number {
  return Math.max(0, Math.ceil((submissionRetryUntilMs - Date.now()) / 1_000));
}

interface FeedbackDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

type SubmissionError = FeedbackApiError | 'PRIVACY_SAVE_FAILED' | null;
type PendingReplyExit =
  | { kind: 'close' }
  | { kind: 'view'; view: 'create' | 'inbox' }
  | { kind: 'select'; feedbackId: string | null };

export const FeedbackDialog: React.FC<FeedbackDialogProps> = ({ isOpen, onClose }) => {
  const { t } = useI18n('common');
  const { status, accept } = usePrivacy();
  const containerRef = useRef<HTMLDivElement>(null);
  const refreshInbox = useFeedbackInboxStore(state => state.refresh);
  const [activeView, setActiveView] = useState<'create' | 'inbox'>('create');
  const [selectedFeedbackId, setSelectedFeedbackId] = useState<string | null>(null);
  const [wideLayout, setWideLayout] = useState(false);
  const [category, setCategory] = useState<FeedbackCategory | ''>('');
  const [content, setContent] = useState('');
  const [includeCorrelation, setIncludeCorrelation] = useState(false);
  const [privacyChecked, setPrivacyChecked] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<SubmissionError>(null);
  const [gitCodeError, setGitCodeError] = useState(false);
  const [showDiscardConfirm, setShowDiscardConfirm] = useState(false);
  const [showPrivacy, setShowPrivacy] = useState(false);
  const [wasTruncated, setWasTruncated] = useState(false);
  const [completed, setCompleted] = useState(false);
  const [retryWaitSeconds, setRetryWaitSeconds] = useState(
    submissionRetrySecondsRemaining,
  );
  const [replyState, setReplyState] = useState({ hasDraft: false, sending: false });
  const [replyResetVersion, setReplyResetVersion] = useState(0);
  const [pendingReplyExit, setPendingReplyExit] = useState<PendingReplyExit | null>(null);

  const contentLength = feedbackContentLength(content);
  const correlationAvailable = feedbackAPI.correlationAvailable();
  const hasDraft = Boolean(
    category || content || includeCorrelation || privacyChecked,
  );
  const canSubmit = Boolean(
    category
      && content.trim()
      && privacyChecked
      && status?.policy
      && retryWaitSeconds === 0,
  );
  const categoryOptions = useMemo(() => [
    { value: 'runtime_error', label: t('feedback.categories.runtimeError') },
    { value: 'feature_request', label: t('feedback.categories.featureRequest') },
    { value: 'usage_question', label: t('feedback.categories.usageQuestion') },
    { value: 'other', label: t('feedback.categories.other') },
  ], [t]);

  useEffect(() => {
    if (!submitting && !replyState.sending) return;
    return registerCriticalOperationExitGuard(() => confirmDialog({
      title: t('feedback.exit.title'),
      message: t('feedback.exit.message'),
      confirmText: t('feedback.exit.quit'),
      cancelText: t('feedback.exit.wait'),
      confirmDanger: true,
      showCancel: true,
    }));
  }, [replyState.sending, submitting, t]);

  useEffect(() => {
    if (retryWaitSeconds <= 0) return;
    const timer = window.setInterval(() => {
      const next = submissionRetrySecondsRemaining();
      setRetryWaitSeconds(next);
      if (next === 0) {
        submissionRetryUntilMs = 0;
        setSubmitError(error => (
          error instanceof FeedbackApiError
            && (error.code === 'RATE_LIMITED' || error.code === 'FEEDBACK_QUOTA_EXCEEDED')
            ? null
            : error
        ));
      }
    }, 1_000);
    return () => window.clearInterval(timer);
  }, [retryWaitSeconds]);

  useEffect(() => {
    if (!isOpen || !containerRef.current) return;
    const element = containerRef.current;
    const update = (width: number) => setWideLayout(width >= 840);
    update(element.getBoundingClientRect().width);
    if (typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(entries => {
      const entry = entries[0];
      if (entry) update(entry.contentRect.width);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [isOpen]);

  const reset = useCallback(() => {
    setCategory('');
    setContent('');
    setIncludeCorrelation(false);
    setPrivacyChecked(false);
    setSubmitting(false);
    setSubmitError(null);
    setGitCodeError(false);
    setShowDiscardConfirm(false);
    setShowPrivacy(false);
    setWasTruncated(false);
    setCompleted(false);
    setActiveView('create');
    setSelectedFeedbackId(null);
    setReplyState({ hasDraft: false, sending: false });
    setReplyResetVersion(current => current + 1);
    setPendingReplyExit(null);
  }, []);

  const closeImmediately = useCallback(() => {
    reset();
    onClose();
  }, [onClose, reset]);

  const requestClose = useCallback(() => {
    if (submitting || replyState.sending) return;
    if (activeView === 'inbox' && replyState.hasDraft) {
      setPendingReplyExit({ kind: 'close' });
      return;
    }
    if (hasDraft && !completed) {
      setShowDiscardConfirm(true);
      return;
    }
    closeImmediately();
  }, [activeView, closeImmediately, completed, hasDraft, replyState, submitting]);

  const handleContentChange = (value: string) => {
    const truncated = truncateFeedbackContent(value);
    setWasTruncated(truncated !== value);
    setContent(truncated);
    setSubmitError(null);
  };

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!canSubmit || !category || submitting || !status?.policy) return;
    setSubmitting(true);
    setSubmitError(null);
    try {
      const submitPreparedFeedback = await feedbackAPI.prepareSubmission({
        category,
        content: content.trim(),
        includeCorrelation,
      });
      if (status.effectiveMode !== 'full') {
        try {
          await accept({
            policyUpdatedAt: status.policy.updatedAt,
            consentVersion: status.policy.consentVersion,
            documentSha256: status.policy.documentSha256,
            locale: status.policy.locale,
          });
        } catch (error) {
          log.warn('Privacy consent could not be saved before feedback submission', error);
          setSubmitError('PRIVACY_SAVE_FAILED');
          return;
        }
      }
      await submitPreparedFeedback();
      await refreshInbox(true);
      setCompleted(true);
    } catch (error) {
      const feedbackError = asFeedbackError(error);
      setSubmitError(feedbackError);
      if (feedbackError.retryAfterSeconds && feedbackError.retryAfterSeconds > 0) {
        submissionRetryUntilMs = Date.now()
          + Math.ceil(feedbackError.retryAfterSeconds) * 1_000;
        setRetryWaitSeconds(submissionRetrySecondsRemaining());
      }
    } finally {
      setSubmitting(false);
    }
  };

  const changeView = (view: 'create' | 'inbox') => {
    if (submitting || replyState.sending) return;
    if (activeView === 'inbox' && view !== activeView && replyState.hasDraft) {
      setPendingReplyExit({ kind: 'view', view });
      return;
    }
    setActiveView(view);
    if (view === 'inbox') void refreshInbox(true);
  };

  const selectFeedback = (feedbackId: string | null) => {
    if (replyState.sending || feedbackId === selectedFeedbackId) return;
    if (replyState.hasDraft) {
      setPendingReplyExit({ kind: 'select', feedbackId });
      return;
    }
    setSelectedFeedbackId(feedbackId);
  };

  const updateReplyState = useCallback((next: { hasDraft: boolean; sending: boolean }) => {
    setReplyState(current => current.hasDraft === next.hasDraft && current.sending === next.sending
      ? current
      : next);
  }, []);

  const discardReplyAndContinue = () => {
    const action = pendingReplyExit;
    setPendingReplyExit(null);
    setReplyState({ hasDraft: false, sending: false });
    setReplyResetVersion(current => current + 1);
    if (!action) return;
    if (action.kind === 'select') {
      setSelectedFeedbackId(action.feedbackId);
      return;
    }
    if (action.kind === 'view') {
      setActiveView(action.view);
      if (action.view === 'inbox') void refreshInbox(true);
      return;
    }
    if (hasDraft && !completed) {
      setShowDiscardConfirm(true);
    } else {
      closeImmediately();
    }
  };

  const openGitCode = () => {
    setGitCodeError(false);
    void systemAPI.openExternal(GITCODE_ISSUES_URL).catch(error => {
      log.warn('GitCode feedback page could not be opened', error);
      setGitCodeError(true);
    });
  };

  const displayError = (error: SubmissionError): string | null => {
    if (!error) return null;
    if (error === 'PRIVACY_SAVE_FAILED') return t('feedback.errors.privacySave');
    if (
      error.code === 'FEEDBACK_QUOTA_EXCEEDED'
      && (!error.retryAfterSeconds || error.retryAfterSeconds <= 0)
    ) {
      return t('feedback.errors.quotaExceeded');
    }
    if (error.code === 'RATE_LIMITED' || error.code === 'FEEDBACK_QUOTA_EXCEEDED') {
      return t('feedback.errors.rateLimited', {
        seconds: retryWaitSeconds || error.retryAfterSeconds,
      });
    }
    if (error.code === 'CAPABILITY_SAVE_FAILED') {
      return t('feedback.errors.capabilitySave');
    }
    if (error.code === 'FEEDBACK_NOT_CONFIGURED') return t('feedback.errors.notConfigured');
    if (error.code === 'REQUEST_TIMEOUT') return t('feedback.errors.timeout');
    if (error.code === 'NETWORK_ERROR' || error.code === 'SERVICE_UNAVAILABLE') {
      return t('feedback.errors.network');
    }
    return t('feedback.errors.generic', { code: error.code });
  };
  const submitErrorMessage = retryWaitSeconds > 0
    ? t('feedback.errors.rateLimited', { seconds: retryWaitSeconds })
    : displayError(submitError);

  return (
    <>
      <Modal
        isOpen={isOpen}
        onClose={requestClose}
        title={t('header.feedback')}
        size="xlarge"
        overlayClassName="bitfun-feedback__overlay"
        contentClassName="bitfun-feedback__modal-content"
        showCloseButton={!submitting && !replyState.sending}
        closeOnOverlayClick={!submitting && !replyState.sending}
        testId="feedback-dialog"
      >
        <div data-bf-component="feedback-dialog" data-bf-part="root">
        {completed ? (
          <div className="bitfun-feedback__complete" role="status">
            <CheckCircle2 size={34} aria-hidden="true" />
            <strong>{t('feedback.complete.title')}</strong>
            <span>{t('feedback.complete.description')}</span>
            <Button onClick={closeImmediately}>{t('shared:statuses.done')}</Button>
          </div>
        ) : (
          <div ref={containerRef} className="bitfun-feedback__center">
            <div className="bitfun-feedback__view-switch" role="tablist" data-bf-component="feedback-dialog" data-bf-part="viewSwitch">
              <button
                type="button"
                role="tab"
                aria-selected={activeView === 'create'}
                className={activeView === 'create' ? 'is-active' : ''}
                disabled={submitting || replyState.sending}
                onClick={() => changeView('create')}
              >
                <SquarePen size={15} aria-hidden="true" />
                {t('feedback.views.create')}
              </button>
              <button
                type="button"
                role="tab"
                aria-selected={activeView === 'inbox'}
                className={activeView === 'inbox' ? 'is-active' : ''}
                disabled={submitting || replyState.sending}
                onClick={() => changeView('inbox')}
              >
                <List size={15} aria-hidden="true" />
                {t('feedback.views.inbox')}
              </button>
            </div>
            {activeView === 'create' ? (
              <form className="bitfun-feedback__form" onSubmit={handleSubmit} data-bf-component="feedback-dialog" data-bf-part="form">
            <div className="bitfun-feedback__field">
              <label>{t('feedback.category')}<span aria-hidden="true">*</span></label>
              <Select
                value={category}
                placeholder={t('feedback.categoryPlaceholder')}
                options={categoryOptions}
                disabled={submitting}
                onChange={value => {
                  setCategory(value as FeedbackCategory);
                  setSubmitError(null);
                }}
                autoClose
                triggerTestId="feedback-category"
              />
            </div>
            <div className="bitfun-feedback__content-field">
              <Textarea
                label={t('feedback.content')}
                required
                value={content}
                disabled={submitting}
                onChange={event => handleContentChange(event.target.value)}
                placeholder={t('feedback.contentPlaceholder')}
                error={content.length > 0 && !content.trim()}
                errorMessage={t('feedback.errors.contentRequired')}
                data-testid="feedback-content"
              />
              <div className="bitfun-feedback__content-meta" aria-live="polite">
                <span>{wasTruncated ? t('feedback.contentTruncated') : ''}</span>
                <span>{contentLength}/2000</span>
              </div>
            </div>
            <div className="bitfun-feedback__correlation">
              <Switch
                checked={includeCorrelation}
                onChange={event => setIncludeCorrelation(event.target.checked)}
                disabled={submitting || !correlationAvailable}
                label={t('feedback.correlation.label')}
                description={correlationAvailable
                  ? t('feedback.correlation.description')
                  : t('feedback.correlation.unavailable')}
              />
            </div>
            <div className="bitfun-feedback__privacy">
              <Checkbox
                checked={privacyChecked}
                disabled={submitting}
                aria-label={t('feedback.privacyConsent')}
                onChange={event => {
                  setPrivacyChecked(event.target.checked);
                  setSubmitError(null);
                }}
              />
              <span>
                {t('feedback.privacyConsentPrefix')}
                <PrivacyStatementLink
                  disabled={submitting}
                  onClick={() => setShowPrivacy(true)}
                />
                {t('feedback.privacyConsentSuffix')}
              </span>
            </div>
            {submitErrorMessage ? (
              <div className="bitfun-feedback__error" role="alert">
                {submitErrorMessage}
              </div>
            ) : null}
            {gitCodeError ? (
              <div className="bitfun-feedback__error" role="alert">
                {t('feedback.errors.gitcode')}
              </div>
            ) : null}
            <div className="bitfun-feedback__actions" data-bf-component="feedback-dialog" data-bf-part="actions">
              <Button type="button" variant="ghost" onClick={openGitCode}>
                <ExternalLink size={15} aria-hidden="true" />
                {t('feedback.actions.gitcode')}
              </Button>
              <span className="bitfun-feedback__action-spacer" />
              <Button
                type="button"
                variant="secondary"
                disabled={submitting}
                onClick={requestClose}
              >
                {t('feedback.actions.cancel')}
              </Button>
              <Button
                type="submit"
                disabled={!canSubmit}
                isLoading={submitting}
                data-testid="feedback-submit"
              >
                <Send size={15} aria-hidden="true" />
                {submitError instanceof FeedbackApiError && submitError.retryable
                  ? t('feedback.actions.retry')
                  : t('feedback.actions.submit')}
              </Button>
            </div>
              </form>
            ) : (
              <FeedbackInboxView
                wide={wideLayout}
                selectedId={selectedFeedbackId}
                onSelect={selectFeedback}
                replySending={replyState.sending}
                resetDraftVersion={replyResetVersion}
                onReplyStateChange={updateReplyState}
              />
            )}
          </div>
        )}
        </div>
      </Modal>
      <ConfirmDialog
        isOpen={showDiscardConfirm}
        onClose={() => setShowDiscardConfirm(false)}
        onConfirm={closeImmediately}
        title={t('feedback.discard.title')}
        message={t('feedback.discard.message')}
        confirmText={t('feedback.discard.confirm')}
        cancelText={t('feedback.discard.continue')}
        confirmDanger
      />
      <ConfirmDialog
        isOpen={pendingReplyExit !== null}
        onClose={() => setPendingReplyExit(null)}
        onConfirm={discardReplyAndContinue}
        title={t('feedback.reply.discardTitle')}
        message={t('feedback.reply.discardMessage')}
        confirmText={t('feedback.reply.discardConfirm')}
        cancelText={t('feedback.reply.continueEditing')}
        confirmDanger
      />
      <PrivacyStatementDialog
        isOpen={showPrivacy}
        onClose={() => setShowPrivacy(false)}
        variant="readonly"
      />
    </>
  );
};

function asFeedbackError(error: unknown): FeedbackApiError {
  return error instanceof FeedbackApiError
    ? error
    : new FeedbackApiError(
      'UNKNOWN_ERROR',
      'Feedback request could not be completed',
      true,
    );
}

export default FeedbackDialog;
