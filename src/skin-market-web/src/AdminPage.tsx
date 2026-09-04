import { ArrowClockwise, GithubLogo, ShieldCheck, WarningCircle } from '@phosphor-icons/react';
import { useCallback, useEffect, useState } from 'react';
import { sharedMarketLoginUrl } from './account';
import { skinMarketApi } from './api';
import { formatMarketDate } from './format';
import type { Locale, Translate } from './i18n';
import { marketImageUrl, retryOriginalMarketImage } from './marketImages';
import type {
  AppearanceAdminSubmissionDetail,
  AppearanceSubmission,
  SharedMarketAccount,
} from './types';

interface AdminPageProps {
  account?: SharedMarketAccount;
  accountResolved: boolean;
  locale: Locale;
  t: Translate;
}

export function AdminPage({ account, accountResolved, locale, t }: AdminPageProps) {
  const [queue, setQueue] = useState<AppearanceSubmission[]>([]);
  const [detail, setDetail] = useState<AppearanceAdminSubmissionDetail>();
  const [loading, setLoading] = useState(false);
  const [detailLoading, setDetailLoading] = useState(false);
  const [acting, setActing] = useState(false);
  const [reason, setReason] = useState('');
  const [error, setError] = useState<Error>();

  const openSubmission = useCallback(async (submissionId: string, signal?: AbortSignal) => {
    setDetailLoading(true);
    setReason('');
    setError(undefined);
    try {
      setDetail(await skinMarketApi.reviewSubmission(submissionId, signal));
    } catch (loadError) {
      if (loadError instanceof DOMException && loadError.name === 'AbortError') return;
      setError(loadError instanceof Error ? loadError : new Error(String(loadError)));
      setDetail(undefined);
    } finally {
      if (!signal?.aborted) setDetailLoading(false);
    }
  }, []);

  const load = useCallback(async (signal?: AbortSignal) => {
    if (!account?.isAdmin) return;
    setLoading(true);
    setError(undefined);
    try {
      const items = (await skinMarketApi.reviewSubmissions('submitted', signal)).items;
      setQueue(items);
      if (items.length === 0) {
        setDetail(undefined);
      } else if (!detail || !items.some(item => item.submissionId === detail.submission.submissionId)) {
        await openSubmission(items[0].submissionId, signal);
      }
    } catch (loadError) {
      if (loadError instanceof DOMException && loadError.name === 'AbortError') return;
      setError(loadError instanceof Error ? loadError : new Error(String(loadError)));
      setQueue([]);
      setDetail(undefined);
    } finally {
      if (!signal?.aborted) setLoading(false);
    }
  }, [account?.isAdmin, detail, openSubmission]);

  useEffect(() => {
    const controller = new AbortController();
    void load(controller.signal);
    return () => controller.abort();
  // Refreshes after decisions are explicit; do not re-fetch when detail selection changes.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [account?.isAdmin]);

  const decide = async (decision: 'approve' | 'reject') => {
    if (!detail || (decision === 'reject' && !reason.trim())) return;
    setActing(true);
    setError(undefined);
    try {
      await skinMarketApi.decideSubmission(detail.submission.submissionId, decision, reason.trim());
      setDetail(undefined);
      setReason('');
      await load();
    } catch (decisionError) {
      setError(decisionError instanceof Error ? decisionError : new Error(String(decisionError)));
    } finally {
      setActing(false);
    }
  };

  if (!accountResolved) {
    return <main id="main-content" className="shell workflow-page"><div className="workflow-loading">{t('reviewLoading')}</div></main>;
  }

  if (!account) {
    return (
      <main id="main-content" className="shell workflow-page">
        <section className="workflow-gate">
          <GithubLogo size={30} weight="regular" aria-hidden="true" />
          <h1>{t('reviewSignInTitle')}</h1>
          <p>{t('reviewSignInBody')}</p>
          <a className="primary-button" href={sharedMarketLoginUrl('/skin/admin')}>
            <GithubLogo size={18} weight="bold" aria-hidden="true" />
            {t('signInGitHub')}
          </a>
        </section>
      </main>
    );
  }

  if (!account.isAdmin) {
    return (
      <main id="main-content" className="shell workflow-page">
        <section className="workflow-gate">
          <WarningCircle size={32} weight="regular" aria-hidden="true" />
          <h1>{t('reviewForbiddenTitle')}</h1>
          <p>{t('reviewForbiddenBody')}</p>
        </section>
      </main>
    );
  }

  return (
    <main id="main-content" className="shell workflow-page workflow-page--wide">
      <header className="workflow-heading">
        <div>
          <p className="eyebrow">{t('market')}</p>
          <h1>{t('reviewTitle')}</h1>
          <p>{t('reviewIntro')}</p>
        </div>
        <button type="button" className="secondary-button" onClick={() => void load()} disabled={loading}>
          <ArrowClockwise size={18} weight="bold" aria-hidden="true" />
          {t('refreshQueue')}
        </button>
      </header>
      {error && <div className="workflow-error" role="alert"><WarningCircle size={20} weight="fill" /><span>{t('reviewError')}</span><small>{error.message}</small></div>}
      {loading && queue.length === 0 ? <div className="workflow-loading">{t('reviewLoading')}</div>
        : queue.length === 0 ? (
          <div className="workflow-empty">
            <ShieldCheck size={36} weight="regular" aria-hidden="true" />
            <h2>{t('reviewEmptyTitle')}</h2>
            <p>{t('reviewEmptyBody')}</p>
          </div>
        ) : (
          <div className="review-workspace">
            <aside className="review-queue" aria-label={t('reviewQueueLabel')}>
              {queue.map(submission => (
                <button
                  type="button"
                  key={submission.submissionId}
                  aria-current={detail?.submission.submissionId === submission.submissionId || undefined}
                  onClick={() => void openSubmission(submission.submissionId)}
                >
                  <strong>{submission.name || submission.slug}</strong>
                  <span>{submission.packageVersion ? `v${submission.packageVersion}` : submission.slug}</span>
                  <small>{formatMarketDate(submission.updatedAt, locale)}</small>
                </button>
              ))}
            </aside>
            <section className="review-detail">
              {detailLoading || !detail ? <div className="workflow-loading">{t('reviewDetailLoading')}</div> : (
                <>
                  <header className="review-detail__heading">
                    <div><h2>{detail.submission.name || detail.submission.slug}</h2><p>{detail.submission.description}</p></div>
                    {detail.submission.previewUrl && (
                      <img
                        src={marketImageUrl(detail.submission.previewUrl, 'compact-v1')}
                        alt=""
                        loading="lazy"
                        decoding="async"
                        onError={(event) => retryOriginalMarketImage(event.currentTarget, detail.submission.previewUrl!)}
                      />
                    )}
                  </header>
                  <dl className="review-facts">
                    <div>
                      <dt>{t('reviewSubmitter')}</dt>
                      <dd>
                        {detail.submitter ? (
                          <a
                            className="review-submitter"
                            href={`https://github.com/${detail.submitter.login}`}
                            target="_blank"
                            rel="noreferrer noopener"
                          >
                            <img src={detail.submitter.avatarUrl} alt="" loading="lazy" decoding="async" />
                            <span>@{detail.submitter.login}</span>
                          </a>
                        ) : t('reviewSubmitterUnknown')}
                      </dd>
                    </div>
                    <div><dt>{t('packageIdentity')}</dt><dd>{detail.submission.packageId || t('notDeclared')}</dd></div>
                    <div><dt>{t('version')}</dt><dd>{detail.submission.packageVersion || t('notDeclared')}</dd></div>
                    <div><dt>{t('compatibility')}</dt><dd>{detail.submission.minOpenBitFunVersion}</dd></div>
                    <div><dt>{t('license')}</dt><dd>{detail.submission.license.spdxExpression || t('customLicense')}</dd></div>
                  </dl>
                  <section className="review-section"><h3>{t('requiredCapabilities')}</h3><div className="capability-list">{detail.submission.requiredCapabilities.length > 0 ? detail.submission.requiredCapabilities.map(value => <code key={value}>{value}</code>) : <span>{t('noExtraCapabilities')}</span>}</div></section>
                  <section className="review-section"><h3>{t('whatsNew')}</h3><p>{detail.submission.changelog || t('notDeclared')}</p></section>
                  <dl className="hash-list">
                    <div><dt>{t('reviewPackageHash')}</dt><dd>{detail.packageSha256 || t('notDeclared')}</dd></div>
                    <div><dt>{t('reviewPreviewHash')}</dt><dd>{detail.previewSha256 || t('notDeclared')}</dd></div>
                    <div><dt>{t('reviewBundleHash')}</dt><dd>{detail.reviewBundleHash || t('notDeclared')}</dd></div>
                  </dl>
                  {detail.manifest !== undefined && <details className="manifest-panel"><summary>{t('reviewManifest')}</summary><pre>{JSON.stringify(detail.manifest, null, 2)}</pre></details>}
                  <div className="review-actions">
                    <label htmlFor="review-reason">{t('reviewReason')}</label>
                    <textarea id="review-reason" rows={4} maxLength={1000} value={reason} onChange={event => setReason(event.target.value)} placeholder={t('reviewReasonPlaceholder')} />
                    <div>
                      <button type="button" className="secondary-button danger-button" disabled={acting || !reason.trim()} onClick={() => void decide('reject')}>{t('reviewReject')}</button>
                      <button type="button" className="primary-button" disabled={acting} onClick={() => void decide('approve')}>{acting ? t('reviewActing') : t('reviewApprove')}</button>
                    </div>
                  </div>
                </>
              )}
            </section>
          </div>
        )}
    </main>
  );
}
