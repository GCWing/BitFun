import { ArrowRight, MagnifyingGlass } from '@phosphor-icons/react';
import {
  useCallback,
  useEffect,
  useRef,
  useState,
} from 'react';
import { skinMarketApi, SkinMarketApiError } from './api';
import { formatCompactNumber, formatMarketDate } from './format';
import { GetOpenBitFunCta } from './GetOpenBitFunCta';
import type { Locale, Translate } from './i18n';
import { PosterImage } from './PosterImage';
import type {
  AppearanceListingSummary,
  AppearanceModeFilter,
  AppearanceSort,
} from './types';

const PAGE_SIZE = 12;

interface CatalogFilters {
  query: string;
  mode: AppearanceModeFilter;
  sort: AppearanceSort;
}

interface CatalogPageProps {
  initialSearch: string;
  locale: Locale;
  onNavigate: (path: string) => void;
  onSearchChange: (search: string) => void;
  t: Translate;
}

export function readCatalogFilters(search: string): CatalogFilters {
  const params = new URLSearchParams(search);
  const mode = params.get('mode');
  const sort = params.get('sort');
  return {
    query: params.get('q') ?? '',
    mode: mode === 'light' || mode === 'dark' ? mode : 'all',
    sort: sort === 'downloads' ? 'downloads' : 'newest',
  };
}

export function catalogSearch(filters: CatalogFilters): string {
  const params = new URLSearchParams();
  const query = filters.query.trim();
  if (query) params.set('q', query);
  if (filters.mode !== 'all') params.set('mode', filters.mode);
  if (filters.sort !== 'newest') params.set('sort', filters.sort);
  const result = params.toString();
  return result ? `?${result}` : '';
}

function isAbortError(error: unknown): boolean {
  return error instanceof DOMException && error.name === 'AbortError';
}

export function CatalogPage({
  initialSearch,
  locale,
  onNavigate,
  onSearchChange,
  t,
}: CatalogPageProps) {
  const initialFilters = useRef(readCatalogFilters(initialSearch));
  const [queryInput, setQueryInput] = useState(initialFilters.current.query);
  const [query, setQuery] = useState(initialFilters.current.query);
  const [mode, setMode] = useState<AppearanceModeFilter>(initialFilters.current.mode);
  const [sort, setSort] = useState<AppearanceSort>(initialFilters.current.sort);
  const [items, setItems] = useState<AppearanceListingSummary[]>([]);
  const [nextCursor, setNextCursor] = useState<string>();
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<unknown>();
  const [retryKey, setRetryKey] = useState(0);
  const loadMoreController = useRef<AbortController>();

  useEffect(() => {
    const timeout = window.setTimeout(() => setQuery(queryInput), 240);
    return () => window.clearTimeout(timeout);
  }, [queryInput]);

  useEffect(() => {
    const next = readCatalogFilters(initialSearch);
    setQueryInput(next.query);
    setQuery(next.query);
    setMode(next.mode);
    setSort(next.sort);
  }, [initialSearch]);

  useEffect(() => {
    const filters = { query, mode, sort };
    const search = catalogSearch(filters);
    onSearchChange(search);
    const target = `/skin/${search}`;
    window.history.replaceState({}, '', target);
  }, [mode, onSearchChange, query, sort]);

  useEffect(() => {
    const controller = new AbortController();
    loadMoreController.current?.abort();
    setLoading(true);
    setError(undefined);
    skinMarketApi
      .list({ query, mode, sort, limit: PAGE_SIZE }, controller.signal)
      .then((page) => {
        setItems(page.items);
        setNextCursor(page.nextCursor);
      })
      .catch((loadError: unknown) => {
        if (!isAbortError(loadError)) {
          setItems([]);
          setNextCursor(undefined);
          setError(loadError);
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [mode, query, retryKey, sort]);

  useEffect(() => () => loadMoreController.current?.abort(), []);

  const loadMore = useCallback(async () => {
    if (!nextCursor || loadingMore) return;
    const controller = new AbortController();
    loadMoreController.current = controller;
    setLoadingMore(true);
    try {
      const page = await skinMarketApi.list(
        { query, mode, sort, cursor: nextCursor, limit: PAGE_SIZE },
        controller.signal,
      );
      setError(undefined);
      setItems((current) => {
        const known = new Set(current.map((item) => item.listingId));
        return [...current, ...page.items.filter((item) => !known.has(item.listingId))];
      });
      setNextCursor(page.nextCursor);
    } catch (loadError) {
      if (!isAbortError(loadError)) setError(loadError);
    } finally {
      if (!controller.signal.aborted) setLoadingMore(false);
    }
  }, [loadingMore, mode, nextCursor, query, sort]);

  return (
    <main id="main-content">
      <section className="masthead shell" aria-labelledby="market-heading">
        <div className="masthead__copy">
          <p className="eyebrow">{t('market')}</p>
          <h1 id="market-heading">{t('headline')}</h1>
          <p>{t('intro')}</p>
        </div>
        <GetOpenBitFunCta placement="catalog" t={t} />
      </section>

      <section className="catalog shell" aria-labelledby="catalog-heading">
        <div className="catalog__heading">
          <h2 id="catalog-heading">{t('catalogTitle')}</h2>
          <p>{t('catalogIntro')}</p>
        </div>

        <div className="catalog-controls">
          <label className="search-field">
            <span className="sr-only">{t('searchLabel')}</span>
            <MagnifyingGlass size={20} weight="regular" aria-hidden="true" />
            <input
              type="search"
              value={queryInput}
              onChange={(event) => setQueryInput(event.currentTarget.value)}
              placeholder={t('searchPlaceholder')}
              autoComplete="off"
            />
          </label>

          <div className="mode-filter" role="group" aria-label={t('modeFilterLabel')}>
            {(['all', 'light', 'dark'] as const).map((value) => (
              <button
                key={value}
                type="button"
                aria-pressed={mode === value}
                onClick={() => setMode(value)}
              >
                {t(value === 'all' ? 'allModes' : value === 'light' ? 'lightMode' : 'darkMode')}
              </button>
            ))}
          </div>

          <label className="sort-field">
            <span>{t('sortLabel')}</span>
            <select value={sort} onChange={(event) => setSort(event.currentTarget.value as AppearanceSort)}>
              <option value="newest">{t('newest')}</option>
              <option value="downloads">{t('downloads')}</option>
            </select>
          </label>
        </div>

        {!loading && !error && items.length > 0 ? (
          <p className="result-count" aria-live="polite">
            {t('resultCount', { count: formatCompactNumber(items.length, locale) })}
          </p>
        ) : null}

        {loading ? <CatalogSkeleton t={t} /> : null}
        {!loading && error && items.length === 0 ? (
          <ErrorState error={error} onRetry={() => setRetryKey((value) => value + 1)} t={t} />
        ) : null}
        {!loading && !error && items.length === 0 ? <EmptyState t={t} /> : null}
        {!loading && items.length > 0 ? (
          <div className="catalog-list">
            {items.map((item, index) => (
              <AppearanceRow
                key={item.listingId}
                item={item}
                locale={locale}
                eager={index === 0}
                onNavigate={onNavigate}
                t={t}
              />
            ))}
          </div>
        ) : null}

        {error && items.length > 0 ? (
          <div className="inline-error" role="status">
            <span>{t('errorBody')}</span>
            <button type="button" onClick={() => void loadMore()}>{t('retry')}</button>
          </div>
        ) : null}

        {nextCursor ? (
          <div className="load-more">
            <button type="button" className="secondary-button" onClick={() => void loadMore()} disabled={loadingMore}>
              {loadingMore ? t('loading') : t('loadMore')}
            </button>
          </div>
        ) : null}
      </section>
    </main>
  );
}

interface AppearanceRowProps {
  eager: boolean;
  item: AppearanceListingSummary;
  locale: Locale;
  onNavigate: (path: string) => void;
  t: Translate;
}

function AppearanceRow({ eager, item, locale, onNavigate, t }: AppearanceRowProps) {
  const path = `/skin/appearances/${encodeURIComponent(item.slug)}`;
  const author = item.author?.trim() || item.owner.login;
  const modeLabel = item.mode === 'light' ? t('lightMode') : t('darkMode');
  const follow = (event: React.MouseEvent<HTMLAnchorElement>) => {
    if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    event.preventDefault();
    onNavigate(path);
  };

  return (
    <article className="appearance-row">
      <a className="appearance-row__poster" href={path} onClick={follow} aria-label={t('openDetail', { name: item.name })}>
        <PosterImage
          src={item.previewUrl}
          alt={t('previewAlt', { name: item.name })}
          name={item.name}
          eager={eager}
          sizes="(max-width: 800px) calc(100vw - 32px), (max-width: 1180px) calc(100vw - 64px), 720px"
          t={t}
        />
      </a>
      <div className="appearance-row__body">
        <div className="appearance-row__meta">
          <span>{modeLabel}</span>
          <span>{t('downloadCount', { count: formatCompactNumber(item.downloadCount, locale) })}</span>
        </div>
        <h3><a href={path} onClick={follow}>{item.name}</a></h3>
        <p className="appearance-row__description">{item.description}</p>
        <p className="appearance-row__author">{t('by', { author })}</p>
        <dl className="appearance-row__facts">
          <div><dt>{t('version')}</dt><dd>{item.packageVersion}</dd></div>
          <div><dt>{t('compatibility')}</dt><dd>{t('minOpenBitFun', { version: item.minOpenBitFunVersion })}</dd></div>
        </dl>
        <a className="text-link" href={path} onClick={follow}>
          {t('openDetail', { name: item.name })}
          <ArrowRight size={18} weight="regular" aria-hidden="true" />
        </a>
        <time dateTime={new Date(item.publishedAt * 1000).toISOString()} className="appearance-row__date">
          {t('published', { date: formatMarketDate(item.publishedAt, locale) })}
        </time>
      </div>
    </article>
  );
}

function CatalogSkeleton({ t }: { t: Translate }) {
  return (
    <div className="catalog-skeleton" aria-live="polite" aria-busy="true">
      <span className="sr-only">{t('loading')}</span>
      {[0, 1, 2].map((item) => (
        <div className="skeleton-row" key={item} aria-hidden="true">
          <div className="skeleton skeleton--poster" />
          <div className="skeleton-copy">
            <div className="skeleton skeleton--short" />
            <div className="skeleton skeleton--title" />
            <div className="skeleton skeleton--line" />
            <div className="skeleton skeleton--line skeleton--line-small" />
          </div>
        </div>
      ))}
    </div>
  );
}

function ErrorState({ error, onRetry, t }: { error: unknown; onRetry: () => void; t: Translate }) {
  const requestId = error instanceof SkinMarketApiError ? error.requestId : undefined;
  return (
    <div className="state-panel" role="alert">
      <h3>{t('errorTitle')}</h3>
      <p>{t('errorBody')}</p>
      {requestId ? <code>{t('requestId', { id: requestId })}</code> : null}
      <button type="button" className="secondary-button" onClick={onRetry}>{t('retry')}</button>
    </div>
  );
}

function EmptyState({ t }: { t: Translate }) {
  return (
    <div className="state-panel">
      <h3>{t('emptyTitle')}</h3>
      <p>{t('emptyBody')}</p>
    </div>
  );
}
