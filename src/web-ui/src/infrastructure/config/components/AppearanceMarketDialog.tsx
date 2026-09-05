import {
  Button,
  Icon,
  ScrollArea,
  SearchField,
  Select,
  Dialog,
  DialogBody,
  DialogClose,
  DialogHeader,
  DialogHeading,
  DialogTitle,
} from '@openbitfun/ui';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { AlertTriangle, PackageCheck } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { confirmDialog } from '@/infrastructure/confirm-dialog';
import { MarketAccountControls } from '@/features/market-account';
import {
  appearanceMarketAPI,
  type AppearanceMarketBrowseRequest,
  type AppearanceMarketListingDetail,
  type AppearanceMarketListingSummary,
  type AppearanceMarketMode,
  type AppearanceMarketRelease,
  type AppearanceMarketSort,
} from '@/infrastructure/api/service-api/AppearanceMarketAPI';
import {
  marketImageSrcSet,
  marketImageUrl,
  retryOriginalMarketImage,
} from '@/infrastructure/api/service-api/MarketImage';
import {
  getAppearancePackageValidationError,
  useAppearance,
  type AppearanceCatalogEntry,
} from '@/infrastructure/appearance';
import { useMarketAccount } from '@/infrastructure/market-account';
import { notificationService } from '@/shared/notification-system';
import { getVersionInfo } from '@/shared/utils/version';
import {
  AppearanceMarketWorkflows,
  type AppearanceMarketWorkflow,
} from './AppearanceMarketWorkflows';

const SUPPORTED_CAPABILITIES = new Set([
  'components.v1',
  'scenes.v1',
  'renderers.v1',
  'assets.v1',
  'background-media.v1',
]);

/** Placeholder cards keep the grid at its loaded height so the dialog never resizes mid-load. */
const SKELETON_CARD_COUNT = 6;

interface AppearanceMarketDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function latestRelease(detail: AppearanceMarketListingDetail): AppearanceMarketRelease | undefined {
  return detail.releases.find(release => release.releaseNumber === detail.latestRelease);
}

function installedEntry(
  appearances: readonly AppearanceCatalogEntry[],
  detail: AppearanceMarketListingSummary,
): AppearanceCatalogEntry | undefined {
  return appearances.find(item => item.source === 'imported' && item.id === detail.packageId);
}

function hasUnsupportedCapabilities(detail: AppearanceMarketListingSummary): boolean {
  return detail.requiredCapabilities.some(capability => !SUPPORTED_CAPABILITIES.has(capability));
}

function requiresNewerOpenBitFun(minimum: string): boolean {
  const parse = (value: string) => {
    const match = /^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$/.exec(value);
    return match ? {
      numbers: [Number(match[1]), Number(match[2]), Number(match[3])] as const,
      prerelease: match[4],
    } : null;
  };
  const current = parse(getVersionInfo().version);
  const required = parse(minimum);
  if (!current || !required) return true;
  for (let index = 0; index < current.numbers.length; index += 1) {
    if (current.numbers[index] !== required.numbers[index]) {
      return current.numbers[index] < required.numbers[index];
    }
  }
  if (!current.prerelease && required.prerelease) return false;
  if (current.prerelease && !required.prerelease) return true;
  return Boolean(
    current.prerelease
    && required.prerelease
    && current.prerelease < required.prerelease,
  );
}

export function AppearanceMarketDialog({ isOpen, onClose }: AppearanceMarketDialogProps) {
  const { t } = useTranslation('settings/appearance');
  const account = useMarketAccount();
  const {
    appearances,
    selectedAppearanceId,
    importPackage,
    activate,
    status: appearanceStatus,
  } = useAppearance();
  const [query, setQuery] = useState('');
  const [submittedQuery, setSubmittedQuery] = useState('');
  const [mode, setMode] = useState<AppearanceMarketMode | 'all'>('all');
  const [sort, setSort] = useState<AppearanceMarketSort>('newest');
  const [items, setItems] = useState<AppearanceMarketListingSummary[]>([]);
  const [nextCursor, setNextCursor] = useState<string | undefined>();
  const [detail, setDetail] = useState<AppearanceMarketListingDetail | null>(null);
  const [loading, setLoading] = useState(false);
  const [appending, setAppending] = useState(false);
  const [loadedOnce, setLoadedOnce] = useState(false);
  const [detailLoading, setDetailLoading] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [view, setView] = useState<'browse' | AppearanceMarketWorkflow>('browse');
  const browseSequence = useRef(0);

  const browseRequest = useMemo<AppearanceMarketBrowseRequest>(() => ({
    query: submittedQuery,
    mode,
    sort,
    limit: 20,
  }), [mode, sort, submittedQuery]);

  const loadPage = useCallback(async (cursor?: string, append = false) => {
    const sequence = ++browseSequence.current;
    setLoading(true);
    setAppending(append);
    setError(null);
    try {
      const page = await appearanceMarketAPI.browse({ ...browseRequest, cursor });
      if (sequence !== browseSequence.current) return;
      setItems(previous => append ? [...previous, ...page.items] : page.items);
      setNextCursor(page.nextCursor);
    } catch (loadError) {
      if (sequence !== browseSequence.current) return;
      setError(errorMessage(loadError));
      if (!append) setItems([]);
    } finally {
      if (sequence === browseSequence.current) {
        setLoading(false);
        setAppending(false);
        setLoadedOnce(true);
      }
    }
  }, [browseRequest]);

  useEffect(() => {
    if (!isOpen || view !== 'browse') return;
    setDetail(null);
    void loadPage();
  }, [isOpen, loadPage, view]);

  useEffect(() => {
    if ((view === 'submissions' && !account.me)
      || (view === 'review' && !account.me?.isAdmin)) {
      setView('browse');
    }
  }, [account.me, view]);

  const selectView = (nextView: 'browse' | AppearanceMarketWorkflow) => {
    setDetail(null);
    setError(null);
    setView(nextView);
  };

  const openDetail = async (item: AppearanceMarketListingSummary) => {
    setDetailLoading(true);
    setError(null);
    try {
      setDetail(await appearanceMarketAPI.getListing(item.slug));
    } catch (loadError) {
      setError(errorMessage(loadError));
    } finally {
      setDetailLoading(false);
    }
  };

  const handleInstall = async (release: AppearanceMarketRelease) => {
    if (!detail) return;
    const local = installedEntry(appearances, detail);
    const linkedToOtherListing = local?.marketOrigin
      && local.marketOrigin.listingId !== detail.listingId;
    if (linkedToOtherListing) return;

    if (local && (!local.marketOrigin || local.localOverride)) {
      const confirmed = await confirmDialog({
        title: t('package.market.replaceTitle'),
        message: t('package.market.replaceMessage', { name: local.name }),
        confirmText: t('package.market.replace'),
        type: 'warning',
      });
      if (!confirmed) return;
    }

    setInstalling(true);
    setError(null);
    try {
      const bytes = await appearanceMarketAPI.downloadRelease({
        slug: detail.slug,
        releaseNumber: release.releaseNumber,
        packageId: detail.packageId,
        packageVersion: release.packageVersion,
        packageSha256: release.packageSha256,
        packageSize: release.packageSize,
      });
      await importPackage(bytes, {
        marketOrigin: {
          listingId: detail.listingId,
          slug: detail.slug,
          releaseId: release.releaseId,
          releaseNumber: release.releaseNumber,
          packageId: detail.packageId,
          packageVersion: release.packageVersion,
          packageSha256: release.packageSha256,
        },
      });
      notificationService.success(t(local
        ? 'package.market.updateSuccess'
        : 'package.market.installSuccess', { name: detail.name }));
    } catch (installError) {
      const validationError = getAppearancePackageValidationError(installError);
      const message = validationError
        ? t('package.diagnostics.importSummary', { count: validationError.issues.length })
        : errorMessage(installError);
      setError(message);
      notificationService.error(t('package.market.installFailed', { error: message }), {
        duration: 5000,
      });
    } finally {
      setInstalling(false);
    }
  };

  const handleApply = async () => {
    if (!detail) return;
    setInstalling(true);
    setError(null);
    try {
      await activate(detail.packageId);
    } catch (applyError) {
      const message = errorMessage(applyError);
      setError(message);
      notificationService.error(t('package.market.applyFailed', { error: message }));
    } finally {
      setInstalling(false);
    }
  };

  const renderDetail = () => {
    if (!detail) return null;
    const release = latestRelease(detail);
    const local = installedEntry(appearances, detail);
    const active = selectedAppearanceId === detail.packageId;
    const unsupported = hasUnsupportedCapabilities(detail);
    const incompatibleVersion = requiresNewerOpenBitFun(
      release?.minOpenBitFunVersion ?? detail.minOpenBitFunVersion,
    );
    const linkedToOtherListing = local?.marketOrigin
      && local.marketOrigin.listingId !== detail.listingId;
    const installedReleaseYanked = Boolean(
      local?.marketOrigin?.listingId === detail.listingId
      && detail.releases.find(item => (
        item.releaseNumber === local.marketOrigin?.releaseNumber
      ))?.yanked,
    );
    const updateAvailable = Boolean(
      local?.marketOrigin?.listingId === detail.listingId
      && release
      && release.releaseNumber > local.marketOrigin.releaseNumber,
    );
    const installDisabled = installing
      || appearanceStatus === 'applying'
      || !release
      || release.yanked
      || unsupported
      || incompatibleVersion
      || Boolean(linkedToOtherListing);

    return (
      <ScrollArea
        className="appearance-market__detail"
        data-openbitfun-component="appearance-settings"
        data-openbitfun-part="marketDetail"
      >
        <Button className="appearance-market__back" variant="outline" size="sm" onClick={() => setDetail(null)} leadingIcon={<Icon name="arrow-left" size="sm" />}>

          {t('package.market.back')}
        </Button>
        <div
          className="appearance-market__detail-hero"
          data-openbitfun-component="appearance-settings"
          data-openbitfun-part="marketDetailPreview"
        >
          {detail.previewUrl
            ? (
              <img
                src={marketImageUrl(detail.previewUrl, 'large-v1')}
                srcSet={marketImageSrcSet(detail.previewUrl)}
                sizes="(max-width: 900px) calc(100vw - 64px), 420px"
                alt={detail.name}
                decoding="async"
                onError={(event) => retryOriginalMarketImage(event.currentTarget, detail.previewUrl)}
              />
            )
            : <Icon name="image" size="lg" aria-hidden="true" />}
        </div>
        <div
          className="appearance-market__detail-body"
          data-openbitfun-component="appearance-settings"
          data-openbitfun-part="marketDetailBody"
        >
          <div className="appearance-market__detail-heading">
            <div>
              <h3>{detail.name}</h3>
              <p>{detail.author || detail.owner.login} · v{detail.packageVersion}</p>
            </div>
            <span className="appearance-market__mode">{t(`package.market.mode.${detail.mode}`)}</span>
          </div>
          <p>{detail.description}</p>
          <dl className="appearance-market__facts">
            <div><dt>{t('package.market.minimumVersion')}</dt><dd>{detail.minOpenBitFunVersion}</dd></div>
            <div><dt>{t('package.market.license')}</dt><dd>{detail.license.spdxExpression || t('package.market.customLicense')}</dd></div>
          </dl>
          {detail.requiredCapabilities.length > 0 && (
            <div className="appearance-market__capabilities">
              <strong>{t('package.market.capabilities')}</strong>
              <div>{detail.requiredCapabilities.map(capability => (
                <code key={capability}>{capability}</code>
              ))}</div>
            </div>
          )}
          <section className="appearance-market__changelog">
            <h4>{t('package.market.changelog')}</h4>
            <p>{detail.changelog || t('package.market.noChangelog')}</p>
          </section>

          {(release?.yanked
            || installedReleaseYanked
            || incompatibleVersion
            || unsupported
            || linkedToOtherListing
            || local?.localOverride) && (
            <div
              className="appearance-market__warning"
              data-openbitfun-component="appearance-settings"
              data-openbitfun-part="marketWarning"
            >
              <AlertTriangle size={16} aria-hidden="true" />
              <span>
                {release?.yanked
                  ? t('package.market.yanked')
                  : installedReleaseYanked
                    ? t('package.market.installedReleaseYanked')
                    : incompatibleVersion
                      ? t('package.market.incompatibleVersion', {
                          version: release?.minOpenBitFunVersion ?? detail.minOpenBitFunVersion,
                        })
                      : unsupported
                        ? t('package.market.unsupportedCapabilities')
                        : linkedToOtherListing
                          ? t('package.market.listingConflict')
                          : t('package.market.localOverride')}
              </span>
            </div>
          )}

          <section
            className="appearance-market__releases"
            data-openbitfun-component="appearance-settings"
            data-openbitfun-part="marketReleaseList"
          >
            <h4>{t('package.market.releases')}</h4>
            {detail.releases.map(item => (
              <div
                key={item.releaseId}
                className="appearance-market__release"
                data-openbitfun-component="appearance-settings"
                data-openbitfun-part="marketRelease"
              >
                <span>v{item.packageVersion}</span>
                <small>{t('package.market.releaseNumber', { number: item.releaseNumber })}</small>
                {item.yanked && <small>{t('package.market.yankedBadge')}</small>}
              </div>
            ))}
          </section>

          <div
            className="appearance-market__actions"
            data-openbitfun-component="appearance-settings"
            data-openbitfun-part="marketActions"
          >
            {local?.marketOrigin?.listingId === detail.listingId
              && !updateAvailable
              && !linkedToOtherListing ? (
              <Button
                variant={active ? 'outline' : 'fill'}
                disabled={active
                  || unsupported
                  || incompatibleVersion
                  || installing
                  || appearanceStatus === 'applying'}
                onClick={() => void handleApply()}
              >
                {active ? <Icon name="check-line" size="sm" /> : <PackageCheck size={15} />}
                {active ? t('package.market.applied') : t('package.market.apply')}
              </Button>
            ) : (
              <Button
                variant="fill"
                disabled={installDisabled}
                onClick={() => release && void handleInstall(release)}
              >
                {updateAvailable ? <Icon name="refresh" size="lg" style={{ width: 15, height: 15 }} /> : <Icon name="arrow-down" size="sm" />}
                {installing
                  ? t('package.market.installing')
                  : updateAvailable
                    ? t('package.market.update')
                    : local
                      ? t('package.market.replace')
                      : t('package.market.install')}
              </Button>
            )}
            <span>{t('package.market.noAutoApply')}</span>
          </div>
        </div>
      </ScrollArea>
    );
  };

  // Placeholder cards stand in until the first page lands; a refresh keeps the
  // current cards mounted and merely dims them. Both keep the dialog one size.
  const showSkeletons = !loadedOnce || (loading && !appending && items.length === 0);
  const refreshing = loading && !appending && items.length > 0;
  const showEmpty = loadedOnce && !loading && items.length === 0 && !error;

  return (
    <Dialog
      open={isOpen}
      onOpenChange={(nextOpen) => { if (!nextOpen) onClose(); }}
      size="xl"
      data-testid="appearance-market-dialog"
    >
      <DialogHeader>
        <DialogHeading>
          <DialogTitle>{t('package.market.title')}{<MarketAccountControls />}</DialogTitle>
        </DialogHeading>
        <DialogClose />
      </DialogHeader>
      <DialogBody>
        <div className="appearance-market__modal">
      <div
        className="appearance-market"
        data-openbitfun-component="appearance-settings"
        data-openbitfun-part="marketDialog"
      >
        <nav
          className="appearance-market__nav"
          data-openbitfun-component="appearance-settings"
          data-openbitfun-part="marketNav"
          aria-label={t('package.market.views.label')}
        >
          <button
            type="button"
            data-active={view === 'browse' || undefined}
            aria-current={view === 'browse' ? 'page' : undefined}
            onClick={() => selectView('browse')}
          >
            {t('package.market.views.browse')}
          </button>
          {account.me && (
            <button
              type="button"
              data-active={view === 'submissions' || undefined}
              aria-current={view === 'submissions' ? 'page' : undefined}
              onClick={() => selectView('submissions')}
            >
              {t('package.market.views.submissions')}
            </button>
          )}
          {account.me?.isAdmin && (
            <button
              type="button"
              data-active={view === 'review' || undefined}
              aria-current={view === 'review' ? 'page' : undefined}
              onClick={() => selectView('review')}
            >
              {t('package.market.views.review')}
            </button>
          )}
        </nav>
        {view !== 'browse' ? <AppearanceMarketWorkflows workflow={view} /> : detail ? renderDetail() : (
          <div
            className="appearance-market__browse"
            data-openbitfun-component="appearance-settings"
            data-openbitfun-part="marketBrowse"
          >
            <div
              className="appearance-market__toolbar"
              data-openbitfun-component="appearance-settings"
              data-openbitfun-part="marketToolbar"
            >
              <SearchField
                className="appearance-market__toolbar-control appearance-market__toolbar-search"
                leadingIcon={<Icon name="search" size="lg" aria-hidden />}
                value={query}
                onValueChange={setQuery}
                onSearch={value => setSubmittedQuery(value.trim())}
                placeholder={t('package.market.search')}
                aria-label={t('package.market.search')}
                size="sm"
              />
              <Select
                className="appearance-market__toolbar-control"
                value={mode}
                onValueChange={value => setMode(value as AppearanceMarketMode | 'all')}
                options={[
                  { value: 'all', label: t('package.market.mode.all') },
                  { value: 'dark', label: t('package.market.mode.dark') },
                  { value: 'light', label: t('package.market.mode.light') },
                ]}
                size="sm"
                aria-label={t('package.market.modeFilter')}
              />
              <Select
                className="appearance-market__toolbar-control"
                value={sort}
                onValueChange={value => setSort(value as AppearanceMarketSort)}
                options={[
                  { value: 'newest', label: t('package.market.sort.newest') },
                  { value: 'downloads', label: t('package.market.sort.downloads') },
                ]}
                size="sm"
                aria-label={t('package.market.sortLabel')}
              />
            </div>

            {error && (
              <div
                className="appearance-market__error"
                role="alert"
                data-openbitfun-component="appearance-settings"
                data-openbitfun-part="marketError"
              >
                <AlertTriangle size={16} />
                <span>{error}</span>
                <Button variant="outline" size="sm" onClick={() => void loadPage()}>
                  {t('package.market.retry')}
                </Button>
              </div>
            )}

            <ScrollArea
              className={`appearance-market__results${refreshing ? ' appearance-market__results--dimmed' : ''}`}
              data-openbitfun-component="appearance-settings"
              data-openbitfun-part="marketResults"
              data-openbitfun-state={loading ? 'loading' : undefined}
              aria-busy={loading || undefined}
            >
              {showSkeletons ? (
                <div className="appearance-market__grid" aria-hidden="true">
                  {Array.from({ length: SKELETON_CARD_COUNT }, (_, index) => (
                    <div
                      key={`market-skeleton-${index}`}
                      className="appearance-market__card appearance-market__card--skeleton"
                    >
                      <div className="appearance-market__preview" />
                      <div className="appearance-market__card-body">
                        <span className="appearance-market__skeleton-line appearance-market__skeleton-line--title" />
                        <span className="appearance-market__skeleton-line appearance-market__skeleton-line--meta" />
                        <span className="appearance-market__skeleton-line" />
                        <span className="appearance-market__skeleton-line appearance-market__skeleton-line--short" />
                      </div>
                    </div>
                  ))}
                </div>
              ) : (
                <div
                  className="appearance-market__grid"
                  data-openbitfun-component="appearance-settings"
                  data-openbitfun-part="marketGrid"
                >
                  {items.map(item => {
                    const local = installedEntry(appearances, item);
                    const updateAvailable = Boolean(
                      local?.marketOrigin?.listingId === item.listingId
                      && item.latestRelease > local.marketOrigin.releaseNumber,
                    );
                    return (
                      <button
                        key={item.listingId}
                        type="button"
                        className="appearance-market__card"
                        onClick={() => void openDetail(item)}
                        disabled={detailLoading}
                        data-openbitfun-component="appearance-settings"
                        data-openbitfun-part="marketCard"
                        data-openbitfun-state={detailLoading ? 'disabled' : undefined}
                      >
                        <div
                          className="appearance-market__preview"
                          data-openbitfun-component="appearance-settings"
                          data-openbitfun-part="marketPreview"
                        >
                          {item.previewUrl
                            ? (
                              <img
                                src={marketImageUrl(item.previewUrl, 'compact-v1')}
                                alt=""
                                loading="lazy"
                                decoding="async"
                                onError={(event) => retryOriginalMarketImage(event.currentTarget, item.previewUrl)}
                              />
                            )
                            : <Icon name="image" size="lg" aria-hidden="true" />}
                          <span>{t(`package.market.mode.${item.mode}`)}</span>
                        </div>
                        <div
                          className="appearance-market__card-body"
                          data-openbitfun-component="appearance-settings"
                          data-openbitfun-part="marketCardBody"
                        >
                          <strong>{item.name}</strong>
                          <span>{item.author || item.owner.login} · v{item.packageVersion}</span>
                          <p>{item.description}</p>
                        </div>
                        {local && (
                          <span
                            className="appearance-market__status"
                            data-openbitfun-component="appearance-settings"
                            data-openbitfun-part="marketStatus"
                          >
                            {updateAvailable
                              ? t('package.market.updateAvailable')
                              : local.localOverride
                                ? t('package.market.modified')
                                : t('package.market.installed')}
                          </span>
                        )}
                      </button>
                    );
                  })}
                </div>
              )}

              {showEmpty && (
                <div
                  className="appearance-market__empty"
                  data-openbitfun-component="appearance-settings"
                  data-openbitfun-part="marketEmpty"
                >
                  <Icon name="store" size="lg" aria-hidden="true" />
                  <p>{t('package.market.empty')}</p>
                </div>
              )}

              {nextCursor && !showSkeletons && (
                <div className="appearance-market__load-more">
                  <Button
                    variant="outline"
                    size="sm"
                    loading={appending}
                    disabled={loading}
                    onClick={() => void loadPage(nextCursor, true)}
                  >
                    {t('package.market.loadMore')}
                  </Button>
                </div>
              )}
            </ScrollArea>

            {loading && <span className="sr-only" role="status">{t('package.market.loading')}</span>}
          </div>
        )}
      </div>
            </div>
            </DialogBody>
    </Dialog>
  );
}
