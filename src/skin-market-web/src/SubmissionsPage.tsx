import { ArrowClockwise, GithubLogo, Image, Package, XCircle } from '@phosphor-icons/react';
import { useCallback, useEffect, useState } from 'react';
import { sharedMarketLoginUrl } from './account';
import { skinMarketApi } from './api';
import { formatMarketDate } from './format';
import type { Locale, Translate } from './i18n';
import type { AppearanceSubmission, AppearanceSubmissionStatus, SharedMarketAccount } from './types';

interface SubmissionsPageProps {
  account?: SharedMarketAccount;
  accountResolved: boolean;
  locale: Locale;
  t: Translate;
}

function statusLabel(status: AppearanceSubmissionStatus, t: Translate): string {
  switch (status) {
    case 'draft': return t('submissionStatusDraft');
    case 'submitted': return t('submissionStatusSubmitted');
    case 'approved': return t('submissionStatusApproved');
    case 'rejected': return t('submissionStatusRejected');
    case 'withdrawn': return t('submissionStatusWithdrawn');
  }
}

export function SubmissionsPage({ account, accountResolved, locale, t }: SubmissionsPageProps) {
  const [items, setItems] = useState<AppearanceSubmission[]>([]);
  const [loading, setLoading] = useState(false);
  const [actingId, setActingId] = useState<string>();
  const [error, setError] = useState<Error>();

  const load = useCallback(async (signal?: AbortSignal) => {
    if (!account) return;
    setLoading(true);
    setError(undefined);
    try {
      setItems((await skinMarketApi.submissions(signal)).items);
    } catch (loadError) {
      if (loadError instanceof DOMException && loadError.name === 'AbortError') return;
      setError(loadError instanceof Error ? loadError : new Error(String(loadError)));
    } finally {
      if (!signal?.aborted) setLoading(false);
    }
  }, [account]);

  useEffect(() => {
    const controller = new AbortController();
    void load(controller.signal);
    return () => controller.abort();
  }, [load]);

  const withdraw = async (submission: AppearanceSubmission) => {
    if (!window.confirm(t('submissionWithdrawConfirm', { name: submission.name || submission.slug }))) return;
    setActingId(submission.submissionId);
    setError(undefined);
    try {
      const updated = await skinMarketApi.withdrawSubmission(submission.submissionId);
      setItems(current => current.map(item => item.submissionId === updated.submissionId ? updated : item));
    } catch (withdrawError) {
      setError(withdrawError instanceof Error ? withdrawError : new Error(String(withdrawError)));
    } finally {
      setActingId(undefined);
    }
  };

  if (!accountResolved) {
    return <main id="main-content" className="shell workflow-page"><div className="workflow-loading">{t('submissionsLoading')}</div></main>;
  }

  if (!account) {
    return (
      <main id="main-content" className="shell workflow-page">
        <section className="workflow-gate">
          <GithubLogo size={30} weight="regular" aria-hidden="true" />
          <h1>{t('submissionsSignInTitle')}</h1>
          <p>{t('submissionsSignInBody')}</p>
          <a className="primary-button" href={sharedMarketLoginUrl('/skin/submissions')}>
            <GithubLogo size={18} weight="bold" aria-hidden="true" />
            {t('signInGitHub')}
          </a>
        </section>
      </main>
    );
  }

  return (
    <main id="main-content" className="shell workflow-page">
      <header className="workflow-heading">
        <div>
          <p className="eyebrow">{t('market')}</p>
          <h1>{t('submissionsTitle')}</h1>
          <p>{t('submissionsIntro')}</p>
        </div>
        <button type="button" className="secondary-button" onClick={() => void load()} disabled={loading}>
          <ArrowClockwise size={18} weight="bold" aria-hidden="true" />
          {t('refresh')}
        </button>
      </header>
      {error && (
        <div className="workflow-error" role="alert">
          <XCircle size={20} weight="fill" aria-hidden="true" />
          <span>{t('submissionsError')}</span>
          <small>{error.message}</small>
        </div>
      )}
      {loading && items.length === 0 ? <div className="workflow-loading">{t('submissionsLoading')}</div>
        : items.length === 0 ? (
          <div className="workflow-empty">
            <Package size={34} weight="regular" aria-hidden="true" />
            <h2>{t('submissionsEmptyTitle')}</h2>
            <p>{t('submissionsEmptyBody')}</p>
          </div>
        ) : (
          <div className="submission-list">
            {items.map(submission => (
              <article className="submission-card" key={submission.submissionId}>
                <div className="submission-card__preview">
                  {submission.previewUrl
                    ? <img src={submission.previewUrl} alt="" />
                    : <Image size={26} weight="regular" aria-hidden="true" />}
                </div>
                <div className="submission-card__body">
                  <div className="submission-card__title">
                    <h2>{submission.name || submission.slug}</h2>
                    <span className={`status-pill status-pill--${submission.status}`}>
                      {statusLabel(submission.status, t)}
                    </span>
                  </div>
                  <p>{submission.description || submission.slug}</p>
                  <div className="submission-card__meta">
                    {submission.packageVersion && <span>v{submission.packageVersion}</span>}
                    <span>{t('submissionUpdated', { date: formatMarketDate(submission.updatedAt, locale) })}</span>
                  </div>
                  {submission.rejectionReason && (
                    <p className="submission-card__feedback">{t('submissionFeedback', { reason: submission.rejectionReason })}</p>
                  )}
                </div>
                {(submission.status === 'draft' || submission.status === 'submitted') && (
                  <button
                    type="button"
                    className="text-button text-button--danger"
                    disabled={actingId === submission.submissionId}
                    onClick={() => void withdraw(submission)}
                  >
                    {actingId === submission.submissionId ? t('withdrawing') : t('withdraw')}
                  </button>
                )}
              </article>
            ))}
          </div>
        )}
    </main>
  );
}
