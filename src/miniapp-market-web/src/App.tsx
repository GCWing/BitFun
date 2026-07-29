import { FormEvent, useCallback, useEffect, useMemo, useState } from 'react';
import { downloadUrl, loginUrl, marketApi, MarketApiError } from './api';
import { formatCompactNumber, formatMarketDate } from './format';
import { useLocale, type Locale, type MessageKey } from './i18n';
import type {
  AdminSubmissionDetail,
  MarketConfig,
  MarketListingDetail,
  MarketListingSummary,
  MarketSubmission,
  Me,
  MiniAppPermissions,
} from './types';

interface RouteState {
  path: string;
  query: URLSearchParams;
}

function currentRoute(): RouteState {
  const base = '/miniapp';
  const path = window.location.pathname.startsWith(base)
    ? window.location.pathname.slice(base.length) || '/'
    : '/';
  return { path, query: new URLSearchParams(window.location.search) };
}

function navigate(path: string) {
  const target = path.startsWith('/miniapp') ? path : `/miniapp${path}`;
  window.history.pushState({}, '', target);
  window.dispatchEvent(new PopStateEvent('popstate'));
  window.scrollTo({ top: 0, behavior: 'smooth' });
}

function App() {
  const { locale, setLocale, t } = useLocale();
  const [route, setRoute] = useState<RouteState>(currentRoute);
  const [config, setConfig] = useState<MarketConfig>();
  const [me, setMe] = useState<Me>();
  const [authResolved, setAuthResolved] = useState(false);

  useEffect(() => {
    const onPopState = () => setRoute(currentRoute());
    window.addEventListener('popstate', onPopState);
    return () => window.removeEventListener('popstate', onPopState);
  }, []);

  const refreshIdentity = useCallback(async () => {
    try {
      setMe(await marketApi.me());
    } catch (error) {
      if (!(error instanceof MarketApiError) || error.code !== 'unauthorized') {
        console.warn('Could not load marketplace identity', error);
      }
      setMe(undefined);
    } finally {
      setAuthResolved(true);
    }
  }, []);

  useEffect(() => {
    void marketApi.config().then(setConfig).catch(() => undefined);
    void refreshIdentity();
  }, [refreshIdentity]);

  const content = (() => {
    if (route.path === '/submit') {
      return (
        <SubmitPage
          me={me}
          authResolved={authResolved}
          query={route.query}
          t={t}
          onSubmitted={() => navigate('/submissions')}
        />
      );
    }
    if (route.path === '/submissions') {
      return <SubmissionsPage me={me} authResolved={authResolved} t={t} />;
    }
    if (route.path === '/admin') {
      return <AdminPage me={me} authResolved={authResolved} t={t} />;
    }
    if (route.path === '/auth/desktop-complete') {
      return <DesktopComplete t={t} />;
    }
    const detailMatch = route.path.match(/^\/apps\/([a-z0-9-]+)$/);
    if (detailMatch) {
      return <DetailPage slug={detailMatch[1]} me={me} locale={locale} t={t} />;
    }
    return <CatalogPage config={config} me={me} locale={locale} t={t} />;
  })();

  return (
    <div className="site-shell">
      <Header
        locale={locale}
        setLocale={setLocale}
        me={me}
        config={config}
        t={t}
        onLogout={async () => {
          await marketApi.logout();
          setMe(undefined);
          navigate('/');
        }}
      />
      {content}
      <footer>
        <span>BitFun MiniApp Market</span>
        <span className="footer-note">{t('footerNote')}</span>
      </footer>
    </div>
  );
}

function Header({
  locale,
  setLocale,
  me,
  config,
  t,
  onLogout,
}: {
  locale: Locale;
  setLocale: (locale: Locale) => void;
  me?: Me;
  config?: MarketConfig;
  t: (key: MessageKey) => string;
  onLogout: () => Promise<void>;
}) {
  return (
    <header className="topbar">
      <button className="brand" onClick={() => navigate('/')} aria-label={t('market')}>
        <span className="brand-mark">B</span>
        <span>{t('market')}</span>
      </button>
      <nav aria-label={t('navigationLabel')}>
        <button onClick={() => navigate('/')}>{t('discover')}</button>
        <button onClick={() => navigate('/submit')}>{t('submit')}</button>
        {me && <button onClick={() => navigate('/submissions')}>{t('submissions')}</button>}
        {me?.isAdmin && <button onClick={() => navigate('/admin')}>{t('admin')}</button>}
      </nav>
      <div className="topbar-actions">
        <label className="locale-picker">
          <span className="sr-only">{t('language')}</span>
          <select value={locale} onChange={(event) => setLocale(event.target.value as Locale)}>
            <option value="en-US">EN</option>
            <option value="zh-CN">简</option>
            <option value="zh-TW">繁</option>
          </select>
        </label>
        {me ? (
          <div className="profile">
            <img src={me.user.avatarUrl} alt="" />
            <span>{me.user.login}</span>
            <button className="text-action" onClick={() => void onLogout()}>
              {t('signOut')}
            </button>
          </div>
        ) : (
          <a
            className={`button button-small ${config?.githubAuthConfigured === false ? 'disabled' : ''}`}
            href={loginUrl(window.location.pathname)}
            aria-disabled={config?.githubAuthConfigured === false}
            onClick={(event) => {
              if (config?.githubAuthConfigured === false) event.preventDefault();
            }}
          >
            {t('signIn')}
          </a>
        )}
      </div>
    </header>
  );
}

function CatalogPage({
  config,
  me,
  locale,
  t,
}: {
  config?: MarketConfig;
  me?: Me;
  locale: Locale;
  t: (key: MessageKey) => string;
}) {
  const [items, setItems] = useState<MarketListingSummary[]>([]);
  const [query, setQuery] = useState('');
  const [category, setCategory] = useState('');
  const [sort, setSort] = useState('newest');
  const [cursor, setCursor] = useState<string>();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<unknown>();

  const load = useCallback(
    async (next?: string, append = false) => {
      setLoading(true);
      setError(undefined);
      const params = new URLSearchParams({ sort, limit: '20' });
      if (query.trim()) params.set('q', query.trim());
      if (category) params.set('category', category);
      if (next) params.set('cursor', next);
      try {
        const page = await marketApi.list(params);
        setItems((previous) => (append ? [...previous, ...page.items] : page.items));
        setCursor(page.nextCursor);
      } catch (caught) {
        setError(caught);
        if (!append) setItems([]);
      } finally {
        setLoading(false);
      }
    },
    [category, query, sort],
  );

  useEffect(() => {
    const timer = window.setTimeout(() => void load(), 220);
    return () => window.clearTimeout(timer);
  }, [load, me]);

  return (
    <main>
      <section className="hero">
        <div className="eyebrow">
          <span className="pulse" />
          {t('heroEyebrow')}
        </div>
        <h1>{t('headline')}</h1>
        <p>{t('intro')}</p>
        <div className="trust-row">
          <span>{t('trustSource')}</span>
          <span>{t('trustHash')}</span>
          <span>{t('trustPermissions')}</span>
        </div>
      </section>

      <section className="catalog" aria-label={t('discover')}>
        <div className="catalog-toolbar">
          <label className="search-field">
            <span>⌕</span>
            <input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder={t('search')}
            />
          </label>
          <select value={category} onChange={(event) => setCategory(event.target.value)}>
            <option value="">{t('allCategories')}</option>
            {(config?.categories || []).map((item) => (
              <option key={item} value={item}>
                {categoryLabel(item, t)}
              </option>
            ))}
          </select>
          <select value={sort} onChange={(event) => setSort(event.target.value)}>
            <option value="newest">{t('newest')}</option>
            <option value="downloads">{t('downloads')}</option>
            <option value="rating">{t('rating')}</option>
          </select>
        </div>

        {error != null && <Notice tone="error">{errorMessage(error, t)}</Notice>}
        <div className="app-grid" aria-busy={loading}>
          {items.map((item) => (
            <AppCard key={item.listingId} app={item} locale={locale} t={t} />
          ))}
          {!loading && items.length === 0 && (
            <div className="empty-state">
              <span>◇</span>
              <p>{t('empty')}</p>
            </div>
          )}
          {loading && items.length === 0 &&
            Array.from({ length: 6 }, (_, index) => (
              <div className="app-card skeleton" key={index} aria-hidden="true" />
            ))}
        </div>
        {cursor && (
          <button className="button button-ghost load-more" onClick={() => void load(cursor, true)}>
            {t('loadMore')}
          </button>
        )}
      </section>
    </main>
  );
}

function AppCard({
  app,
  locale,
  t,
}: {
  app: MarketListingSummary;
  locale: Locale;
  t: (key: MessageKey) => string;
}) {
  const localized = localizedListing(app, locale);
  return (
    <article className="app-card" onClick={() => navigate(`/apps/${app.slug}`)}>
      <div className="card-visual">
        {app.screenshotUrls[0] ? (
          <img src={app.screenshotUrls[0]} alt="" loading="lazy" />
        ) : (
          <span className="app-icon-large">{app.icon || '✦'}</span>
        )}
        <span className="category-chip">{categoryLabel(app.category, t)}</span>
      </div>
      <div className="card-body">
        <div className="card-heading">
          <span className="app-icon">{app.icon || '✦'}</span>
          <div>
            <h2>{localized.name}</h2>
            <p className="owner">
              {t('by')} @{app.owner.login}
            </p>
          </div>
        </div>
        <p className="card-description">{localized.description}</p>
        <div className="card-meta">
          <span>★ {app.ratingAverage.toFixed(1)} · {app.ratingCount}</span>
          <span>↓ {formatCompactNumber(app.downloadCount, locale)}</span>
          <span>v{app.latestRelease}</span>
        </div>
      </div>
    </article>
  );
}

function DetailPage({
  slug,
  me,
  locale,
  t,
}: {
  slug: string;
  me?: Me;
  locale: Locale;
  t: (key: MessageKey) => string;
}) {
  const [app, setApp] = useState<MarketListingDetail>();
  const [error, setError] = useState<unknown>();
  const [ratingBusy, setRatingBusy] = useState(false);
  const [moderationReason, setModerationReason] = useState('');

  const load = useCallback(async () => {
    try {
      setApp(await marketApi.detail(slug));
      setError(undefined);
    } catch (caught) {
      setError(caught);
    }
  }, [slug]);

  useEffect(() => {
    void load();
  }, [load, me]);

  if (error) {
    return (
      <main className="narrow-page">
        <button className="back-link" onClick={() => navigate('/')}>← {t('back')}</button>
        <Notice tone="error">{errorMessage(error, t)}</Notice>
      </main>
    );
  }
  if (!app) return <main className="narrow-page loading-page">{t('loading')}</main>;

  const owner = me?.user.githubId === app.owner.githubId;
  const localized = localizedListing(app, locale);
  return (
    <main className="detail-page">
      <button className="back-link" onClick={() => navigate('/')}>← {t('back')}</button>
      <section className="detail-hero">
        <div className="detail-copy">
          <span className="category-chip">{categoryLabel(app.category, t)}</span>
          <div className="detail-title-row">
            <span className="detail-icon">{app.icon || '✦'}</span>
            <div>
              <h1>{localized.name}</h1>
              <p>@{app.owner.login} · {t('version')} {app.latestRelease}</p>
            </div>
          </div>
          <p className="detail-description">{localized.description}</p>
          <div className="detail-actions">
            <a className="button" href={downloadUrl(app.slug, app.latestRelease)}>
              ↓ {t('install')}
            </a>
            <button
              className={`button button-ghost ${app.isFavorited ? 'active' : ''}`}
              onClick={async () => {
                if (!me) {
                  window.location.href = loginUrl(window.location.pathname);
                  return;
                }
                const result = await marketApi.favorite(app.slug, !app.isFavorited);
                setApp({ ...app, isFavorited: result.isFavorited, favoriteCount: result.count });
              }}
            >
              {app.isFavorited ? '♥' : '♡'} {app.isFavorited ? t('favorited') : t('favorite')}
            </button>
            {owner && (
              <button
                className="button button-ghost"
                onClick={() =>
                  navigate(
                    `/submit?listingId=${encodeURIComponent(app.listingId)}&slug=${encodeURIComponent(app.slug)}&release=${app.latestRelease + 1}`,
                  )
                }
              >
                {t('submitUpdate')}
              </button>
            )}
          </div>
          <div className="rating-control" aria-label={t('ratingLabel')}>
            {[1, 2, 3, 4, 5].map((value) => (
              <button
                key={value}
                disabled={ratingBusy}
                className={value <= (app.myRating || 0) ? 'selected' : ''}
                onClick={async () => {
                  if (!me) {
                    window.location.href = loginUrl(window.location.pathname);
                    return;
                  }
                  setRatingBusy(true);
                  try {
                    const result =
                      app.myRating === value
                        ? await marketApi.deleteRating(app.slug)
                        : await marketApi.rate(app.slug, value);
                    setApp({
                      ...app,
                      myRating: result.myRating,
                      ratingAverage: result.average,
                      ratingCount: result.count,
                    });
                  } finally {
                    setRatingBusy(false);
                  }
                }}
                aria-label={`${value} ${t('stars')}`}
              >
                ★
              </button>
            ))}
            <span>{app.ratingAverage.toFixed(1)} ({app.ratingCount})</span>
          </div>
        </div>
        <div className="detail-gallery">
          {app.screenshotUrls[0] ? (
            <img src={app.screenshotUrls[0]} alt={`${localized.name} screenshot`} />
          ) : (
            <span>{app.icon || '✦'}</span>
          )}
        </div>
      </section>

      <section className="detail-columns">
        <div>
          <h2>{t('permissions')}</h2>
          <PermissionList permissions={app.permissions} t={t} />
          <h2>{t('changelog')}</h2>
          <p className="prose">{app.changelog}</p>
        </div>
        <aside className="facts-panel">
          <Fact label={t('requires')} value={`v${app.minBitfunVersion}+`} />
          <Fact
            label={t('downloadsLabel')}
            value={formatCompactNumber(app.downloadCount, locale)}
          />
          <Fact
            label={t('favoritesLabel')}
            value={formatCompactNumber(app.favoriteCount, locale)}
          />
          <Fact
            label={t('licenseLabel')}
            value={app.license.spdxExpression || app.license.customUrl || t('customLicense')}
          />
          {app.repositoryUrl && (
            <a href={app.repositoryUrl} target="_blank" rel="noreferrer">
              {t('viewSourceRepository')}
            </a>
          )}
        </aside>
      </section>

      <section className="release-section">
        <h2>{t('releases')}</h2>
        {app.releases.map((release) => (
          <div className={`release-row ${release.yanked ? 'yanked' : ''}`} key={release.releaseId}>
            <strong>v{release.releaseNumber}</strong>
            <span>{formatMarketDate(release.publishedAt, locale)}</span>
            <span className="hash" title={release.packageSha256}>
              {release.packageSha256.slice(0, 12)}
            </span>
            <span>{release.yanked ? t('yankedLabel') : release.changelog}</span>
            {me?.isAdmin && !release.yanked && (
              <button
                className="text-action danger-action"
                disabled={!moderationReason.trim()}
                onClick={async () => {
                  await marketApi.yankRelease(release.releaseId, moderationReason.trim());
                  setModerationReason('');
                  await load();
                }}
              >
                {t('yank')}
              </button>
            )}
          </div>
        ))}
        {me?.isAdmin && (
          <div className="moderation-panel">
            <label>
              <span>{t('moderationReason')}</span>
              <input
                value={moderationReason}
                onChange={(event) => setModerationReason(event.target.value)}
              />
            </label>
            <button
              className="button button-danger"
              disabled={!moderationReason.trim()}
              onClick={async () => {
                await marketApi.unpublishListing(app.listingId, moderationReason.trim());
                navigate('/');
              }}
            >
              {t('unpublish')}
            </button>
          </div>
        )}
      </section>
    </main>
  );
}

function SubmitPage({
  me,
  authResolved,
  query,
  t,
  onSubmitted,
}: {
  me?: Me;
  authResolved: boolean;
  query: URLSearchParams;
  t: (key: MessageKey) => string;
  onSubmitted: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<unknown>();
  const [done, setDone] = useState(false);
  const listingId = query.get('listingId') || undefined;
  const initialSlug = query.get('slug') || '';
  const initialRelease = Number(query.get('release') || 1);

  if (!authResolved) return <main className="form-page loading-page">{t('loading')}</main>;
  if (!me) {
    return (
      <main className="form-page">
        <section className="auth-gate">
          <span className="gate-icon">↗</span>
          <h1>{t('signInRequired')}</h1>
          <a className="button" href={loginUrl('/miniapp/submit')}>{t('signIn')}</a>
        </section>
      </main>
    );
  }

  return (
    <main className="form-page">
      <div className="page-intro">
        <span className="eyebrow">{t('publisherWorkspace')}</span>
        <h1>{t('submitTitle')}</h1>
        <p>{t('submitIntro')}</p>
      </div>
      {error != null && <Notice tone="error">{errorMessage(error, t)}</Notice>}
      {done && <Notice tone="success">{t('submitted')}</Notice>}
      <form
        className="submission-form"
        onSubmit={async (event) => {
          event.preventDefault();
          const form = new FormData(event.currentTarget);
          const packageFile = form.get('package');
          const screenshots = form.getAll('screenshots').filter((value) => value instanceof File);
          if (!(packageFile instanceof File) || packageFile.size === 0 || screenshots.length === 0) {
            setError(new LocalizedUiError('choosePackageAndScreenshot'));
            return;
          }
          setBusy(true);
          setError(undefined);
          try {
            const licenseKind = String(form.get('licenseKind'));
            let submission = await marketApi.createSubmission({
              listingId,
              slug: String(form.get('slug')),
              releaseNumber: Number(form.get('releaseNumber')),
              name: String(form.get('name')),
              description: String(form.get('description')),
              icon: String(form.get('icon')) || '✦',
              category: String(form.get('category')),
              tags: String(form.get('tags'))
                .split(',')
                .map((tag) => tag.trim())
                .filter(Boolean),
              minBitfunVersion: String(form.get('minBitfunVersion')),
              changelog: String(form.get('changelog')),
              license:
                licenseKind === 'spdx'
                  ? { spdxExpression: String(form.get('licenseValue')) }
                  : { customUrl: String(form.get('licenseValue')) },
              repositoryUrl: String(form.get('repositoryUrl')) || undefined,
            });
            submission = await marketApi.uploadPackage(submission.submissionId, packageFile);
            for (const [index, screenshot] of screenshots.slice(0, 5).entries()) {
              submission = await marketApi.uploadScreenshot(
                submission.submissionId,
                index,
                screenshot as File,
              );
            }
            await marketApi.submit(submission.submissionId);
            setDone(true);
            window.setTimeout(onSubmitted, 900);
          } catch (caught) {
            setError(caught);
          } finally {
            setBusy(false);
          }
        }}
      >
        <fieldset>
          <legend>{t('listingSection')}</legend>
          <div className="form-grid">
            <Field label={t('slugLabel')}>
              <input name="slug" required pattern="[a-z0-9][a-z0-9-]{2,62}" defaultValue={initialSlug} readOnly={Boolean(listingId)} />
            </Field>
            <Field label={t('releaseNumberLabel')}>
              <input name="releaseNumber" type="number" min="1" required defaultValue={initialRelease} readOnly={Boolean(listingId)} />
            </Field>
            <Field label={t('nameLabel')}>
              <input name="name" required maxLength={80} />
            </Field>
            <Field label={t('iconLabel')}>
              <input name="icon" defaultValue="✦" maxLength={8} />
            </Field>
            <Field label={t('categoryLabel')}>
              <select name="category" defaultValue="utilities">
                {['developer', 'productivity', 'data', 'creative', 'education', 'utilities', 'entertainment', 'other'].map((value) => (
                  <option key={value} value={value}>{categoryLabel(value, t)}</option>
                ))}
              </select>
            </Field>
            <Field label={t('tagsLabel')}>
              <input name="tags" placeholder={t('tagsPlaceholder')} />
            </Field>
          </div>
          <Field label={t('descriptionLabel')}>
            <textarea name="description" required maxLength={500} rows={3} />
          </Field>
        </fieldset>

        <fieldset>
          <legend>{t('releaseSection')}</legend>
          <div className="form-grid">
            <Field label={t('minBitfunVersionLabel')}>
              <input name="minBitfunVersion" required defaultValue="0.2.14" />
            </Field>
            <Field label={t('publicRepositoryOptional')}>
              <input name="repositoryUrl" type="url" placeholder="https://github.com/…" />
            </Field>
            <Field label={t('licenseTypeLabel')}>
              <select name="licenseKind">
                <option value="spdx">{t('spdxExpression')}</option>
                <option value="custom">{t('customLicenseUrl')}</option>
              </select>
            </Field>
            <Field label={t('licenseLabel')}>
              <input name="licenseValue" required defaultValue="MIT" />
            </Field>
          </div>
          <Field label={t('changelog')}>
            <textarea name="changelog" required rows={4} />
          </Field>
        </fieldset>

        <fieldset>
          <legend>{t('reviewBundle')}</legend>
          <div className="upload-grid">
            <Field label={t('package')}>
              <input name="package" type="file" accept=".bfminiapp,application/zip" required />
            </Field>
            <Field label={`${t('screenshots')} (1–5)`}>
              <input name="screenshots" type="file" accept="image/png,image/jpeg,image/webp" multiple required />
            </Field>
          </div>
          <div className="safety-note">
            <strong>{t('beforeUpload')}</strong>
            <span>{t('packageSafety')}</span>
          </div>
        </fieldset>

        <button className="button submit-button" disabled={busy}>
          {busy ? t('uploading') : t('publishForReview')}
        </button>
      </form>
    </main>
  );
}

function SubmissionsPage({
  me,
  authResolved,
  t,
}: {
  me?: Me;
  authResolved: boolean;
  t: (key: MessageKey) => string;
}) {
  const [items, setItems] = useState<MarketSubmission[]>([]);
  const [error, setError] = useState<unknown>();
  useEffect(() => {
    if (!me) return;
    void marketApi
      .submissions()
      .then((page) => setItems(page.items))
      .catch((caught) => setError(caught));
  }, [me]);
  const withdraw = async (submissionId: string) => {
    try {
      await marketApi.withdrawSubmission(submissionId);
      setItems((current) =>
        current.map((item) =>
          item.submissionId === submissionId ? { ...item, status: 'withdrawn' } : item,
        ),
      );
    } catch (caught) {
      setError(caught);
    }
  };
  if (!authResolved) return <main className="narrow-page loading-page">{t('loading')}</main>;
  if (!me) return <AuthGate t={t} returnTo="/miniapp/submissions" />;
  return (
    <main className="narrow-page">
      <div className="page-intro">
        <span className="eyebrow">{t('publisherHistory')}</span>
        <h1>{t('mySubmissions')}</h1>
      </div>
      {error != null && <Notice tone="error">{errorMessage(error, t)}</Notice>}
      <div className="submission-list">
        {items.map((item) => (
          <SubmissionRow
            key={item.submissionId}
            item={item}
            t={t}
            action={
              item.status === 'draft' || item.status === 'submitted' ? (
                <button className="text-action danger-action" onClick={() => void withdraw(item.submissionId)}>
                  {t('withdraw')}
                </button>
              ) : undefined
            }
          />
        ))}
        {error == null && items.length === 0 && <div className="empty-state"><span>◇</span><p>{t('noSubmissions')}</p></div>}
      </div>
    </main>
  );
}

function AdminPage({
  me,
  authResolved,
  t,
}: {
  me?: Me;
  authResolved: boolean;
  t: (key: MessageKey) => string;
}) {
  const [items, setItems] = useState<MarketSubmission[]>([]);
  const [selected, setSelected] = useState<AdminSubmissionDetail>();
  const [sourceName, setSourceName] = useState('meta.json');
  const [sourceMode, setSourceMode] = useState<'current' | 'diff'>('current');
  const [reason, setReason] = useState('');
  const [error, setError] = useState<unknown>();

  const load = useCallback(async () => {
    try {
      setItems((await marketApi.adminSubmissions()).items);
    } catch (caught) {
      setError(caught);
    }
  }, []);
  useEffect(() => {
    if (me?.isAdmin) void load();
  }, [load, me]);
  if (!authResolved) return <main className="narrow-page loading-page">{t('loading')}</main>;
  if (!me) return <AuthGate t={t} returnTo="/miniapp/admin" />;
  if (!me.isAdmin) {
    return (
      <main className="narrow-page">
        <Notice tone="error">{t('administratorRequired')}</Notice>
      </main>
    );
  }

  return (
    <main className="admin-page">
      <div className="page-intro">
        <span className="eyebrow">{t('adminEyebrow')}</span>
        <h1>{t('reviewQueue')}</h1>
      </div>
      {error != null && <Notice tone="error">{errorMessage(error, t)}</Notice>}
      <div className="review-layout">
        <div className="review-list">
          {items.map((item) => (
            <button
              key={item.submissionId}
              className={selected?.submission.submissionId === item.submissionId ? 'selected' : ''}
              onClick={async () => {
                try {
                  const detail = await marketApi.adminDetail(item.submissionId);
                  setSelected(detail);
                  setSourceName(Object.keys(detail.sourceFiles)[0] || 'meta.json');
                  setSourceMode('current');
                } catch (caught) {
                  setError(caught);
                }
              }}
            >
              <span className="app-icon">{item.icon}</span>
              <span><strong>{item.name}</strong><small>{item.slug} · v{item.releaseNumber}</small></span>
              <StatusBadge status={item.status} t={t} />
            </button>
          ))}
          {items.length === 0 && <p className="muted">{t('queueClear')}</p>}
        </div>
        <div className="review-detail">
          {!selected ? (
            <div className="review-placeholder">{t('reviewPlaceholder')}</div>
          ) : (
            <>
              <div className="review-summary">
                <span className="detail-icon">{selected.submission.icon}</span>
                <div>
                  <h2>{selected.submission.name}</h2>
                  <p>{selected.submission.description}</p>
                </div>
              </div>
              <PermissionList permissions={selected.submission.permissions} t={t} />
              <div className="review-evidence-grid">
                <Fact label={t('releaseLabel')} value={`v${selected.submission.releaseNumber}`} />
                <Fact
                  label={t('minimumBitfunLabel')}
                  value={selected.submission.minBitfunVersion}
                />
                <Fact
                  label={t('licenseLabel')}
                  value={
                    selected.submission.license.spdxExpression
                    || selected.submission.license.customUrl
                    || t('notDeclared')
                  }
                />
                <Fact label={t('changelog')} value={selected.submission.changelog} />
              </div>
              {selected.submission.repositoryUrl && (
                <a
                  className="review-repository"
                  href={selected.submission.repositoryUrl}
                  target="_blank"
                  rel="noreferrer"
                >
                  {t('publicRepository')}
                </a>
              )}
              <div className="review-screenshots">
                {selected.submission.screenshotUrls.map((url, index) => (
                  <figure key={url}>
                    <img src={url} alt={`${t('submissionScreenshot')} ${index + 1}`} />
                    <figcaption>
                      <span>#{index + 1}</span>
                      <code>{selected.screenshotHashes[index]}</code>
                    </figcaption>
                  </figure>
                ))}
              </div>
              <div className="hash-block">
                <span>{t('packageSha256')}</span>
                <code>{selected.submission.packageSha256}</code>
              </div>
              <div className="source-browser">
                <div className="source-mode">
                  <button
                    className={sourceMode === 'current' ? 'active' : ''}
                    onClick={() => setSourceMode('current')}
                  >
                    {t('currentSource')}
                  </button>
                  <button
                    className={sourceMode === 'diff' ? 'active' : ''}
                    onClick={() => setSourceMode('diff')}
                  >
                    {t('sourceDiff')}
                  </button>
                </div>
                <div className="source-tabs">
                  {Object.keys(selected.sourceDiffs).map((name) => (
                    <button className={sourceName === name ? 'active' : ''} onClick={() => setSourceName(name)} key={name}>{name}</button>
                  ))}
                </div>
                <pre className={sourceMode === 'diff' ? 'diff-view' : ''}>
                  <code>
                    {sourceMode === 'diff'
                      ? selected.sourceDiffs[sourceName] || t('noSourceChanges')
                      : selected.sourceFiles[sourceName] || ''}
                  </code>
                </pre>
              </div>
              <div className="review-actions">
                <button
                  className="button"
                  onClick={async () => {
                    try {
                      await marketApi.review(selected.submission.submissionId, 'approve');
                      setSelected(undefined);
                      await load();
                    } catch (caught) {
                      setError(caught);
                    }
                  }}
                >
                  {t('approve')}
                </button>
                <input value={reason} onChange={(event) => setReason(event.target.value)} placeholder={t('rejectionReason')} />
                <button
                  className="button button-danger"
                  disabled={!reason.trim()}
                  onClick={async () => {
                    try {
                      await marketApi.review(selected.submission.submissionId, 'reject', reason);
                      setSelected(undefined);
                      setReason('');
                      await load();
                    } catch (caught) {
                      setError(caught);
                    }
                  }}
                >
                  {t('reject')}
                </button>
              </div>
            </>
          )}
        </div>
      </div>
    </main>
  );
}

function DesktopComplete({ t }: { t: (key: MessageKey) => string }) {
  return (
    <main className="form-page">
      <section className="auth-gate">
        <span className="gate-icon success">✓</span>
        <h1>{t('authComplete')}</h1>
        <p>{t('authCompleteBody')}</p>
      </section>
    </main>
  );
}

function PermissionList({
  permissions,
  t,
}: {
  permissions: MiniAppPermissions;
  t: (key: MessageKey) => string;
}) {
  const rows = useMemo(() => {
    const values: string[] = [t('permissionPrivateStorage')];
    permissions.fs?.read?.forEach((scope) =>
      values.push(`${t('permissionReadFiles')}: ${scope}`),
    );
    permissions.fs?.write?.forEach((scope) =>
      values.push(`${t('permissionWriteFiles')}: ${scope}`),
    );
    permissions.shell?.allow?.forEach((command) =>
      values.push(`${t('permissionRunCommand')}: ${command}`),
    );
    permissions.net?.allow?.forEach((domain) =>
      values.push(`${t('permissionNetwork')}: ${domain}`),
    );
    if (permissions.ai?.enabled) values.push(t('permissionAi'));
    if (permissions.agent?.enabled) values.push(t('permissionAgent'));
    if (permissions.notifications?.system) values.push(t('permissionNotifications'));
    return values;
  }, [permissions, t]);
  return (
    <ul className="permission-list">
      {rows.map((row) => <li key={row}><span>✓</span>{row}</li>)}
      <li className="node-denied"><span>×</span>{t('permissionNodeUnavailable')}</li>
    </ul>
  );
}

function Fact({ label, value }: { label: string; value: string }) {
  return <div className="fact"><span>{label}</span><strong>{value}</strong></div>;
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return <label className="field"><span>{label}</span>{children}</label>;
}

function Notice({ tone, children }: { tone: 'error' | 'success'; children: React.ReactNode }) {
  return <div className={`notice ${tone}`} role={tone === 'error' ? 'alert' : 'status'}>{children}</div>;
}

function AuthGate({ t, returnTo }: { t: (key: MessageKey) => string; returnTo: string }) {
  return (
    <main className="form-page">
      <section className="auth-gate">
        <span className="gate-icon">↗</span>
        <h1>{t('signInRequired')}</h1>
        <a className="button" href={loginUrl(returnTo)}>{t('signIn')}</a>
      </section>
    </main>
  );
}

function SubmissionRow({
  item,
  action,
  t,
}: {
  item: MarketSubmission;
  action?: React.ReactNode;
  t: (key: MessageKey) => string;
}) {
  return (
    <article className="submission-row">
      <span className="app-icon">{item.icon}</span>
      <div>
        <h2>{item.name}</h2>
        <p>{item.slug} · {t('releaseLabel')} {item.releaseNumber}</p>
        {item.rejectionReason && <p className="rejection">{item.rejectionReason}</p>}
      </div>
      <span className="hash">{item.packageSha256?.slice(0, 12) || t('packagePending')}</span>
      <StatusBadge status={item.status} t={t} />
      {action}
    </article>
  );
}

function StatusBadge({
  status,
  t,
}: {
  status: MarketSubmission['status'];
  t: (key: MessageKey) => string;
}) {
  const labels: Record<MarketSubmission['status'], MessageKey> = {
    draft: 'statusDraft',
    submitted: 'statusSubmitted',
    approved: 'statusApproved',
    rejected: 'statusRejected',
    withdrawn: 'statusWithdrawn',
  };
  return <span className={`status-badge ${status}`}>{t(labels[status])}</span>;
}

function categoryLabel(value: string, t: (key: MessageKey) => string) {
  const labels: Record<string, MessageKey> = {
    developer: 'categoryDeveloper',
    productivity: 'categoryProductivity',
    data: 'categoryData',
    creative: 'categoryCreative',
    education: 'categoryEducation',
    utilities: 'categoryUtilities',
    entertainment: 'categoryEntertainment',
    other: 'categoryOther',
  };
  const key = labels[value];
  return key ? t(key) : value;
}

function localizedListing(
  listing: MarketListingSummary,
  locale: Locale,
): { name: string; description: string; tags: string[] } {
  const fallbacks =
    locale === 'zh-TW'
      ? ['zh-TW', 'zh-CN', 'en-US']
      : locale === 'zh-CN'
        ? ['zh-CN', 'en-US']
        : ['en-US', 'zh-CN'];
  const values = fallbacks
    .map((candidate) => listing.i18n?.locales?.[candidate])
    .filter((value) => value != null);
  return {
    name: values.find((value) => value?.name)?.name || listing.name,
    description:
      values.find((value) => value?.description)?.description || listing.description,
    tags: values.find((value) => value?.tags?.length)?.tags || listing.tags,
  };
}

class LocalizedUiError {
  constructor(readonly key: MessageKey) {}
}

function errorMessage(error: unknown, t: (key: MessageKey) => string) {
  if (error instanceof LocalizedUiError) return t(error.key);
  if (error instanceof MarketApiError) {
    if (error.code === 'market_not_public') return t('marketNotPublic');
    if (error.code === 'oauth_not_configured') return t('oauthNotConfigured');
    if (error.code === 'unauthorized') return t('signInRequired');
  }
  if (error instanceof Error) return error.message;
  return String(error);
}

export default App;
