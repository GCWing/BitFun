import {
  ArrowLeft,
  ArrowSquareOut,
  CheckCircle,
  Desktop,
  DownloadSimple,
  Prohibit,
  ShieldCheck,
  WarningCircle,
} from '@phosphor-icons/react';
import { useEffect, useMemo, useState } from 'react';
import { downloadUrl, skinMarketApi, SkinMarketApiError } from './api';
import {
  formatCompactNumber,
  formatMarketDate,
  formatPackageSize,
  shortHash,
} from './format';
import { GetOpenBitFunCta } from './GetOpenBitFunCta';
import type { Locale, Translate } from './i18n';
import { PosterImage } from './PosterImage';
import type {
  AppearanceListingDetail,
  AppearanceMarketRelease,
} from './types';

interface DetailPageProps {
  catalogSearch: string;
  isAdmin: boolean;
  locale: Locale;
  onNavigate: (path: string) => void;
  slug: string;
  t: Translate;
}

function isAbortError(error: unknown): boolean {
  return error instanceof DOMException && error.name === 'AbortError';
}

export function DetailPage({ catalogSearch, isAdmin, locale, onNavigate, slug, t }: DetailPageProps) {
  const [detail, setDetail] = useState<AppearanceListingDetail>();
  const [error, setError] = useState<unknown>();
  const [loading, setLoading] = useState(true);
  const [retryKey, setRetryKey] = useState(0);
  const [moderationReason, setModerationReason] = useState('');
  const [moderationBusy, setModerationBusy] = useState<string>();
  const [moderationError, setModerationError] = useState<Error>();
  const catalogPath = `/skin/${catalogSearch}`;

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setError(undefined);
    skinMarketApi
      .detail(slug, controller.signal)
      .then(setDetail)
      .catch((loadError: unknown) => {
        if (!isAbortError(loadError)) {
          setDetail(undefined);
          setError(loadError);
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [retryKey, slug]);

  useEffect(() => {
    if (!detail) return;
    const previous = document.title;
    document.title = `${detail.name} | OpenBitFun Skin Market`;
    return () => { document.title = previous; };
  }, [detail]);

  const navigate = (path: string) => (event: React.MouseEvent<HTMLAnchorElement>) => {
    if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    event.preventDefault();
    onNavigate(path);
  };

  if (loading) return <DetailSkeleton catalogPath={catalogPath} navigate={navigate} t={t} />;

  if (error || !detail) {
    const notFound = error instanceof SkinMarketApiError
      && (error.code === 'not_found' || error.code === 'listing_not_found' || error.code === 'http_404');
    return (
      <main id="main-content" className="shell detail-state">
        <a className="back-link" href={catalogPath} onClick={navigate(catalogPath)}>
          <ArrowLeft size={18} weight="regular" aria-hidden="true" />
          {t('detailBack')}
        </a>
        <div className="state-panel" role={notFound ? undefined : 'alert'}>
          <h1>{t(notFound ? 'notFoundTitle' : 'errorTitle')}</h1>
          <p>{t(notFound ? 'notFoundBody' : 'errorBody')}</p>
          {!notFound && error instanceof SkinMarketApiError && error.requestId ? (
            <code>{t('requestId', { id: error.requestId })}</code>
          ) : null}
          {notFound ? (
            <a className="primary-button" href={catalogPath} onClick={navigate(catalogPath)}>{t('backToCatalog')}</a>
          ) : (
            <button type="button" className="secondary-button" onClick={() => setRetryKey((value) => value + 1)}>{t('retry')}</button>
          )}
        </div>
      </main>
    );
  }

  const author = detail.author?.trim() || detail.owner.login;
  const currentRelease = detail.releases.find((release) => release.releaseNumber === detail.latestRelease)
    ?? detail.releases.find((release) => !release.yanked)
    ?? detail.releases[0];
  const modeLabel = detail.mode === 'light' ? t('lightMode') : t('darkMode');
  const customLicenseUrl = safeExternalUrl(detail.license.customUrl);
  const repositoryUrl = safeExternalUrl(detail.repositoryUrl);
  const trimmedModerationReason = moderationReason.trim();

  const yankRelease = async (release: AppearanceMarketRelease) => {
    if (!trimmedModerationReason || moderationBusy) return;
    if (!window.confirm(t('yankConfirm', { version: release.packageVersion }))) return;
    setModerationBusy(release.releaseId);
    setModerationError(undefined);
    try {
      await skinMarketApi.yankRelease(release.releaseId, trimmedModerationReason);
      setModerationReason('');
      const remainingReleaseCount = detail.releases.filter((item) => (
        !item.yanked && item.releaseId !== release.releaseId
      )).length;
      if (remainingReleaseCount === 0) onNavigate(catalogPath);
      else setRetryKey((value) => value + 1);
    } catch (actionError) {
      setModerationError(toError(actionError));
    } finally {
      setModerationBusy(undefined);
    }
  };

  const unpublishListing = async () => {
    if (!trimmedModerationReason || moderationBusy) return;
    if (!window.confirm(t('unpublishConfirm', { name: detail.name }))) return;
    setModerationBusy(detail.listingId);
    setModerationError(undefined);
    try {
      await skinMarketApi.unpublishListing(detail.listingId, trimmedModerationReason);
      onNavigate(catalogPath);
    } catch (actionError) {
      setModerationError(toError(actionError));
    } finally {
      setModerationBusy(undefined);
    }
  };

  return (
    <main id="main-content" className="detail-page">
      <div className="shell">
        <a className="back-link" href={catalogPath} onClick={navigate(catalogPath)}>
          <ArrowLeft size={18} weight="regular" aria-hidden="true" />
          {t('detailBack')}
        </a>

        <section className="detail-hero" aria-labelledby="appearance-title">
          <div className="detail-hero__poster">
            <PosterImage
              src={detail.previewUrl}
              alt={t('previewAlt', { name: detail.name })}
              name={detail.name}
              eager
              sizes="(max-width: 800px) calc(100vw - 32px), (max-width: 1180px) calc(100vw - 64px), 720px"
              variant="large-v1"
              t={t}
            />
          </div>
          <div className="detail-hero__copy">
            <div className="detail-kicker">
              <span>{modeLabel}</span>
              <span>{t('downloadCount', { count: formatCompactNumber(detail.downloadCount, locale) })}</span>
            </div>
            <h1 id="appearance-title">{detail.name}</h1>
            <p className="detail-description">{detail.description}</p>
            <p className="detail-author">{t('by', { author })}</p>
            {currentRelease && !currentRelease.yanked ? (
              <a className="primary-button download-button" href={downloadUrl(detail.slug, currentRelease.releaseNumber)}>
                <DownloadSimple size={20} weight="bold" aria-hidden="true" />
                {t('detailDownload')}
              </a>
            ) : null}
            <p className="import-hint">
              <Desktop size={19} weight="regular" aria-hidden="true" />
              <span>{t('desktopInstallNote')}</span>
            </p>
            <GetOpenBitFunCta placement="listing" t={t} />
          </div>
        </section>

        <section className="detail-fact-strip" aria-label={t('compatibility')}>
          <div><span>{t('mode')}</span><strong>{modeLabel}</strong></div>
          <div><span>{t('version')}</span><strong>{detail.packageVersion}</strong></div>
          <div><span>{t('compatibility')}</span><strong>{t('minOpenBitFun', { version: detail.minOpenBitFunVersion })}</strong></div>
        </section>

        <div className="detail-content">
          <div className="detail-content__main">
            <section className="content-section" aria-labelledby="changes-heading">
              <h2 id="changes-heading">{t('whatsNew')}</h2>
              <p className="long-copy">{detail.changelog || t('notDeclared')}</p>
            </section>

            <ReleaseHistory
              detail={detail}
              isAdmin={isAdmin}
              locale={locale}
              moderationBusy={moderationBusy}
              moderationReasonReady={Boolean(trimmedModerationReason)}
              onYank={yankRelease}
              t={t}
            />

            {isAdmin ? (
              <section className="content-section moderation-panel" aria-labelledby="moderation-heading">
                <div className="moderation-panel__heading">
                  <WarningCircle size={24} weight="regular" aria-hidden="true" />
                  <div>
                    <h2 id="moderation-heading">{t('moderationTitle')}</h2>
                    <p>{t('moderationIntro')}</p>
                  </div>
                </div>
                <label className="moderation-panel__reason" htmlFor="moderation-reason">
                  <span>{t('moderationReason')}</span>
                  <textarea
                    id="moderation-reason"
                    value={moderationReason}
                    maxLength={1000}
                    rows={3}
                    placeholder={t('moderationReasonPlaceholder')}
                    disabled={Boolean(moderationBusy)}
                    onChange={(event) => setModerationReason(event.target.value)}
                  />
                </label>
                {moderationError ? (
                  <p className="moderation-panel__error" role="alert">
                    <strong>{t('moderationFailed')}</strong>
                    <span>{moderationError.message}</span>
                  </p>
                ) : null}
                <div className="moderation-panel__actions">
                  <button
                    type="button"
                    className="secondary-button danger-button"
                    disabled={!trimmedModerationReason || Boolean(moderationBusy)}
                    onClick={() => void unpublishListing()}
                  >
                    <Prohibit size={18} weight="bold" aria-hidden="true" />
                    {moderationBusy === detail.listingId ? t('unpublishingListing') : t('unpublishListing')}
                  </button>
                </div>
              </section>
            ) : null}
          </div>

          <aside className="detail-aside">
            <section className="aside-section" aria-labelledby="compatibility-heading">
              <ShieldCheck size={24} weight="regular" aria-hidden="true" />
              <h2 id="compatibility-heading">{t('compatibility')}</h2>
              <p>{t('minOpenBitFun', { version: detail.minOpenBitFunVersion })}</p>
              <h3>{t('requiredCapabilities')}</h3>
              {detail.requiredCapabilities.length ? (
                <ul className="capability-list">
                  {detail.requiredCapabilities.map((capability) => <li key={capability}><code>{capability}</code></li>)}
                </ul>
              ) : (
                <p className="verified-line"><CheckCircle size={18} weight="fill" aria-hidden="true" />{t('noExtraCapabilities')}</p>
              )}
            </section>

            <section className="aside-section" aria-labelledby="package-heading">
              <h2 id="package-heading">{t('packageIdentity')}</h2>
              <code className="package-id">{detail.packageId}</code>
              <dl className="aside-facts">
                <div><dt>{t('license')}</dt><dd>{licenseLabel(detail, t)}</dd></div>
                <div><dt>{t('publishedLabel')}</dt><dd>{formatMarketDate(detail.publishedAt, locale)}</dd></div>
              </dl>
              {customLicenseUrl ? (
                <a className="text-link" href={customLicenseUrl} target="_blank" rel="noreferrer">
                  {t('customLicense')}<ArrowSquareOut size={17} weight="regular" aria-hidden="true" />
                </a>
              ) : null}
              {repositoryUrl ? (
                <a className="text-link" href={repositoryUrl} target="_blank" rel="noreferrer">
                  {t('viewRepository')}<ArrowSquareOut size={17} weight="regular" aria-hidden="true" />
                </a>
              ) : null}
            </section>
          </aside>
        </div>
      </div>
    </main>
  );
}

function licenseLabel(detail: AppearanceListingDetail, t: Translate): string {
  return detail.license.spdxExpression || (detail.license.customUrl ? t('customLicense') : t('notDeclared'));
}

export function safeExternalUrl(value?: string): string | undefined {
  if (!value || value.length > 2_048) return undefined;
  try {
    const parsed = new URL(value);
    if (parsed.protocol !== 'https:' || parsed.username || parsed.password || !parsed.hostname) {
      return undefined;
    }
    return parsed.href;
  } catch {
    return undefined;
  }
}

function ReleaseHistory({
  detail,
  isAdmin,
  locale,
  moderationBusy,
  moderationReasonReady,
  onYank,
  t,
}: {
  detail: AppearanceListingDetail;
  isAdmin: boolean;
  locale: Locale;
  moderationBusy?: string;
  moderationReasonReady: boolean;
  onYank: (release: AppearanceMarketRelease) => Promise<void>;
  t: Translate;
}) {
  const releases = useMemo(
    () => [...detail.releases].sort((left, right) => right.releaseNumber - left.releaseNumber),
    [detail.releases],
  );
  const visible = releases.slice(0, 4);
  const older = releases.slice(4);

  return (
    <section className="content-section" aria-labelledby="releases-heading">
      <h2 id="releases-heading">{t('releases')}</h2>
      <div className="release-list">
        {visible.map((release) => (
          <ReleaseItem
            key={release.releaseId}
            detail={detail}
            isAdmin={isAdmin}
            locale={locale}
            moderationBusy={moderationBusy}
            moderationReasonReady={moderationReasonReady}
            onYank={onYank}
            release={release}
            t={t}
          />
        ))}
      </div>
      {older.length ? (
        <details className="older-releases">
          <summary>{t('olderReleases', { count: older.length })}</summary>
          <div className="release-list">
            {older.map((release) => (
              <ReleaseItem
                key={release.releaseId}
                detail={detail}
                isAdmin={isAdmin}
                locale={locale}
                moderationBusy={moderationBusy}
                moderationReasonReady={moderationReasonReady}
                onYank={onYank}
                release={release}
                t={t}
              />
            ))}
          </div>
        </details>
      ) : null}
    </section>
  );
}

function ReleaseItem({
  detail,
  isAdmin,
  locale,
  moderationBusy,
  moderationReasonReady,
  onYank,
  release,
  t,
}: {
  detail: AppearanceListingDetail;
  isAdmin: boolean;
  locale: Locale;
  moderationBusy?: string;
  moderationReasonReady: boolean;
  onYank: (release: AppearanceMarketRelease) => Promise<void>;
  release: AppearanceMarketRelease;
  t: Translate;
}) {
  const current = release.releaseNumber === detail.latestRelease;
  return (
    <article className={`release-item${release.yanked ? ' release-item--yanked' : ''}`}>
      <div className="release-item__heading">
        <div>
          <h3>{release.packageVersion}</h3>
          <p>{t('releaseNumber', { number: release.releaseNumber })}</p>
        </div>
        <span className={release.yanked ? 'release-status release-status--warning' : 'release-status'}>
          {release.yanked ? t('yanked') : current ? t('currentRelease') : formatMarketDate(release.publishedAt, locale)}
        </span>
      </div>
      <dl className="release-facts">
        <div><dt>{t('compatibility')}</dt><dd>{t('minOpenBitFun', { version: release.minOpenBitFunVersion })}</dd></div>
        <div><dt>{t('packageSize')}</dt><dd>{formatPackageSize(release.packageSize, locale)}</dd></div>
        <div><dt>{t('checksum')}</dt><dd><code title={release.packageSha256}>{shortHash(release.packageSha256)}</code></dd></div>
      </dl>
      {!release.yanked ? (
        <div className="release-item__actions">
          <a className="text-link" href={downloadUrl(detail.slug, release.releaseNumber)}>
            <DownloadSimple size={17} weight="regular" aria-hidden="true" />
            {t('downloadVersion', { version: release.packageVersion })}
          </a>
          {isAdmin ? (
            <button
              type="button"
              className="release-yank-button"
              disabled={!moderationReasonReady || Boolean(moderationBusy)}
              onClick={() => void onYank(release)}
            >
              <Prohibit size={16} weight="bold" aria-hidden="true" />
              {moderationBusy === release.releaseId ? t('yankingRelease') : t('yankRelease')}
            </button>
          ) : null}
        </div>
      ) : null}
    </article>
  );
}

function toError(error: unknown): Error {
  return error instanceof Error ? error : new Error(String(error));
}

function DetailSkeleton({ catalogPath, navigate, t }: {
  catalogPath: string;
  navigate: (path: string) => (event: React.MouseEvent<HTMLAnchorElement>) => void;
  t: Translate;
}) {
  return (
    <main id="main-content" className="detail-page" aria-live="polite" aria-busy="true">
      <div className="shell">
        <a className="back-link" href={catalogPath} onClick={navigate(catalogPath)}>
          <ArrowLeft size={18} weight="regular" aria-hidden="true" />
          {t('detailBack')}
        </a>
        <span className="sr-only">{t('loading')}</span>
        <div className="detail-hero detail-hero--skeleton" aria-hidden="true">
          <div className="skeleton skeleton--detail-poster" />
          <div className="skeleton-copy">
            <div className="skeleton skeleton--short" />
            <div className="skeleton skeleton--detail-title" />
            <div className="skeleton skeleton--line" />
            <div className="skeleton skeleton--line skeleton--line-small" />
          </div>
        </div>
      </div>
    </main>
  );
}
