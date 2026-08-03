import { useCallback, useEffect, useState } from 'react';
import { AlertTriangle, Image, Inbox, RefreshCw, ShieldCheck } from 'lucide-react';
import { Button, confirmDialog, Textarea } from '@/component-library';
import {
  appearanceMarketAPI,
  type AppearanceAdminSubmissionDetail,
  type AppearanceMarketSubmission,
} from '@/infrastructure/api/service-api/AppearanceMarketAPI';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { notificationService } from '@/shared/notification-system';

export type AppearanceMarketWorkflow = 'submissions' | 'review';

interface AppearanceMarketWorkflowsProps {
  workflow: AppearanceMarketWorkflow;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function canWithdraw(submission: AppearanceMarketSubmission): boolean {
  return submission.status === 'draft' || submission.status === 'submitted';
}

export function AppearanceMarketWorkflows({ workflow }: AppearanceMarketWorkflowsProps) {
  const { t, formatDate } = useI18n('settings/appearance');
  const [submissions, setSubmissions] = useState<AppearanceMarketSubmission[]>([]);
  const [reviewQueue, setReviewQueue] = useState<AppearanceMarketSubmission[]>([]);
  const [reviewDetail, setReviewDetail] = useState<AppearanceAdminSubmissionDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [detailLoading, setDetailLoading] = useState(false);
  const [actingId, setActingId] = useState<string | null>(null);
  const [reason, setReason] = useState('');
  const [error, setError] = useState<string | null>(null);

  const formattedDate = (timestamp: number) => formatDate(timestamp * 1000, {
    dateStyle: 'medium',
    timeStyle: 'short',
  });

  const loadSubmissions = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setSubmissions(await appearanceMarketAPI.listSubmissions());
    } catch (loadError) {
      setError(errorMessage(loadError));
      setSubmissions([]);
    } finally {
      setLoading(false);
    }
  }, []);

  const openReview = useCallback(async (submissionId: string) => {
    setDetailLoading(true);
    setError(null);
    setReason('');
    try {
      setReviewDetail(await appearanceMarketAPI.getReviewSubmission(submissionId));
    } catch (loadError) {
      setError(errorMessage(loadError));
      setReviewDetail(null);
    } finally {
      setDetailLoading(false);
    }
  }, []);

  const loadReviewQueue = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const queue = await appearanceMarketAPI.listReviewSubmissions();
      setReviewQueue(queue);
      if (queue.length === 0) {
        setReviewDetail(null);
      } else if (!reviewDetail
        || !queue.some(item => item.submissionId === reviewDetail.submission.submissionId)) {
        await openReview(queue[0].submissionId);
      }
    } catch (loadError) {
      setError(errorMessage(loadError));
      setReviewQueue([]);
      setReviewDetail(null);
    } finally {
      setLoading(false);
    }
  }, [openReview, reviewDetail]);

  useEffect(() => {
    if (workflow === 'submissions') void loadSubmissions();
    else void loadReviewQueue();
  // Reload only when the selected workflow changes. Review actions refresh explicitly.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [workflow]);

  const withdraw = async (submission: AppearanceMarketSubmission) => {
    const confirmed = await confirmDialog({
      title: t('package.market.submissions.withdrawTitle'),
      message: t('package.market.submissions.withdrawMessage', { name: submission.name || submission.slug }),
      confirmText: t('package.market.submissions.withdraw'),
      type: 'warning',
    });
    if (!confirmed) return;
    setActingId(submission.submissionId);
    setError(null);
    try {
      const updated = await appearanceMarketAPI.withdrawSubmission(submission.submissionId);
      setSubmissions(items => items.map(item => (
        item.submissionId === updated.submissionId ? updated : item
      )));
      notificationService.success(t('package.market.submissions.withdrawn'));
    } catch (withdrawError) {
      setError(errorMessage(withdrawError));
    } finally {
      setActingId(null);
    }
  };

  const decide = async (decision: 'approve' | 'reject') => {
    if (!reviewDetail || (decision === 'reject' && !reason.trim())) return;
    const submissionId = reviewDetail.submission.submissionId;
    setActingId(submissionId);
    setError(null);
    try {
      await appearanceMarketAPI.reviewSubmission(submissionId, decision, reason.trim());
      notificationService.success(t(`package.market.review.${decision}Success`));
      setReviewDetail(null);
      setReason('');
      await loadReviewQueue();
    } catch (reviewError) {
      setError(errorMessage(reviewError));
    } finally {
      setActingId(null);
    }
  };

  const renderError = () => error && (
    <div className="appearance-market__error" role="alert">
      <AlertTriangle size={16} aria-hidden="true" />
      <span>{error}</span>
      <Button
        variant="ghost"
        size="small"
        onClick={() => void (workflow === 'submissions' ? loadSubmissions() : loadReviewQueue())}
      >
        {t('package.market.retry')}
      </Button>
    </div>
  );

  if (workflow === 'submissions') {
    return (
      <section
        className="appearance-market__workflow"
        data-bf-component="appearance-config"
        data-bf-part="marketWorkflow"
        aria-labelledby="appearance-market-submissions-title"
      >
        <header className="appearance-market__workflow-heading">
          <div>
            <h3 id="appearance-market-submissions-title">{t('package.market.submissions.title')}</h3>
            <p>{t('package.market.submissions.hint')}</p>
          </div>
          <Button variant="ghost" size="small" onClick={() => void loadSubmissions()} disabled={loading}>
            <RefreshCw size={14} aria-hidden="true" />
            {t('package.market.submissions.refresh')}
          </Button>
        </header>
        {renderError()}
        {loading ? <p className="appearance-market__loading">{t('package.market.submissions.loading')}</p>
          : submissions.length === 0 ? (
            <div className="appearance-market__empty">
              <Inbox size={28} aria-hidden="true" />
              <p>{t('package.market.submissions.empty')}</p>
            </div>
          ) : (
            <div
              className="appearance-market__submission-list"
              data-bf-component="appearance-config"
              data-bf-part="marketSubmissionList"
            >
              {submissions.map(submission => (
                <article
                  key={submission.submissionId}
                  className="appearance-market__submission"
                  data-bf-component="appearance-config"
                  data-bf-part="marketSubmission"
                >
                  <div className="appearance-market__submission-preview">
                    {submission.previewUrl
                      ? <img src={submission.previewUrl} alt="" />
                      : <Image size={22} aria-hidden="true" />}
                  </div>
                  <div className="appearance-market__submission-body">
                    <div className="appearance-market__submission-title">
                      <strong>{submission.name || submission.slug}</strong>
                      <span className={`appearance-market__submission-status appearance-market__submission-status--${submission.status}`}>
                        {t(`package.market.submissions.status.${submission.status}`)}
                      </span>
                    </div>
                    <p>{submission.description || submission.slug}</p>
                    <small>
                      {submission.packageVersion ? `v${submission.packageVersion} · ` : ''}
                      {t('package.market.submissions.updated', { date: formattedDate(submission.updatedAt) })}
                    </small>
                    {submission.rejectionReason && (
                      <p className="appearance-market__submission-rejection">
                        {t('package.market.submissions.rejection', { reason: submission.rejectionReason })}
                      </p>
                    )}
                  </div>
                  {canWithdraw(submission) && (
                    <Button
                      variant="ghost"
                      size="small"
                      isLoading={actingId === submission.submissionId}
                      onClick={() => void withdraw(submission)}
                    >
                      {t('package.market.submissions.withdraw')}
                    </Button>
                  )}
                </article>
              ))}
            </div>
          )}
      </section>
    );
  }

  return (
    <section
      className="appearance-market__workflow"
      data-bf-component="appearance-config"
      data-bf-part="marketWorkflow"
      aria-labelledby="appearance-market-review-title"
    >
      <header className="appearance-market__workflow-heading">
        <div>
          <h3 id="appearance-market-review-title">{t('package.market.review.title')}</h3>
          <p>{t('package.market.review.hint')}</p>
        </div>
        <Button variant="ghost" size="small" onClick={() => void loadReviewQueue()} disabled={loading}>
          <RefreshCw size={14} aria-hidden="true" />
          {t('package.market.review.refresh')}
        </Button>
      </header>
      {renderError()}
      {loading && reviewQueue.length === 0 ? (
        <p className="appearance-market__loading">{t('package.market.review.loading')}</p>
      ) : reviewQueue.length === 0 ? (
        <div className="appearance-market__empty">
          <ShieldCheck size={28} aria-hidden="true" />
          <p>{t('package.market.review.empty')}</p>
        </div>
      ) : (
        <div
          className="appearance-market__review-layout"
          data-bf-component="appearance-config"
          data-bf-part="marketReviewLayout"
        >
          <div
            className="appearance-market__review-queue"
            data-bf-component="appearance-config"
            data-bf-part="marketReviewQueue"
          >
            {reviewQueue.map(submission => (
              <button
                key={submission.submissionId}
                type="button"
                className="appearance-market__review-item"
                data-active={reviewDetail?.submission.submissionId === submission.submissionId || undefined}
                onClick={() => void openReview(submission.submissionId)}
              >
                <strong>{submission.name || submission.slug}</strong>
                <span>{submission.packageVersion ? `v${submission.packageVersion}` : submission.slug}</span>
                <small>{formattedDate(submission.updatedAt)}</small>
              </button>
            ))}
          </div>
          <div
            className="appearance-market__review-detail"
            data-bf-component="appearance-config"
            data-bf-part="marketReviewDetail"
          >
            {detailLoading || !reviewDetail ? (
              <p className="appearance-market__loading">{t('package.market.review.detailLoading')}</p>
            ) : (
              <>
                <div className="appearance-market__review-heading">
                  <div>
                    <h4>{reviewDetail.submission.name || reviewDetail.submission.slug}</h4>
                    <p>{reviewDetail.submission.description}</p>
                  </div>
                  {reviewDetail.submission.previewUrl && (
                    <img src={reviewDetail.submission.previewUrl} alt="" />
                  )}
                </div>
                <dl className="appearance-market__facts">
                  <div><dt>{t('package.market.review.package')}</dt><dd>{reviewDetail.submission.packageId}</dd></div>
                  <div><dt>{t('package.market.review.version')}</dt><dd>{reviewDetail.submission.packageVersion}</dd></div>
                  <div><dt>{t('package.market.minimumVersion')}</dt><dd>{reviewDetail.submission.minBitfunVersion}</dd></div>
                  <div><dt>{t('package.market.license')}</dt><dd>{reviewDetail.submission.license.spdxExpression || t('package.market.customLicense')}</dd></div>
                </dl>
                {reviewDetail.submission.requiredCapabilities.length > 0 && (
                  <div className="appearance-market__capabilities">
                    <strong>{t('package.market.capabilities')}</strong>
                    <div>{reviewDetail.submission.requiredCapabilities.map(capability => <code key={capability}>{capability}</code>)}</div>
                  </div>
                )}
                <section className="appearance-market__changelog">
                  <h4>{t('package.market.changelog')}</h4>
                  <p>{reviewDetail.submission.changelog || t('package.market.noChangelog')}</p>
                </section>
                <dl className="appearance-market__review-hashes">
                  <div><dt>{t('package.market.review.packageHash')}</dt><dd>{reviewDetail.packageSha256 || t('package.market.review.unavailable')}</dd></div>
                  <div><dt>{t('package.market.review.previewHash')}</dt><dd>{reviewDetail.previewSha256 || t('package.market.review.unavailable')}</dd></div>
                  <div><dt>{t('package.market.review.bundleHash')}</dt><dd>{reviewDetail.reviewBundleHash || t('package.market.review.unavailable')}</dd></div>
                </dl>
                {reviewDetail.manifest !== undefined && (
                  <details className="appearance-market__review-manifest">
                    <summary>{t('package.market.review.manifest')}</summary>
                    <pre>{JSON.stringify(reviewDetail.manifest, null, 2)}</pre>
                  </details>
                )}
                <div
                  className="appearance-market__review-actions"
                  data-bf-component="appearance-config"
                  data-bf-part="marketReviewActions"
                >
                  <Textarea
                    label={t('package.market.review.reason')}
                    placeholder={t('package.market.review.reasonPlaceholder')}
                    value={reason}
                    onChange={event => setReason(event.target.value)}
                    rows={3}
                    maxLength={1000}
                  />
                  <div>
                    <Button
                      variant="success"
                      size="small"
                      isLoading={actingId === reviewDetail.submission.submissionId}
                      onClick={() => void decide('approve')}
                    >
                      {t('package.market.review.approve')}
                    </Button>
                    <Button
                      variant="danger"
                      size="small"
                      disabled={!reason.trim()}
                      isLoading={actingId === reviewDetail.submission.submissionId}
                      onClick={() => void decide('reject')}
                    >
                      {t('package.market.review.reject')}
                    </Button>
                  </div>
                </div>
              </>
            )}
          </div>
        </div>
      )}
    </section>
  );
}
