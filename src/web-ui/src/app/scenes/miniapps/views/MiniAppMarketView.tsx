import React, { useCallback, useEffect, useMemo, useState } from 'react';
import {
  AlertTriangle,
  ArrowUpRight,
  Check,
  Download,
  ExternalLink,
  Heart,
  Loader2,
  LogOut,
  PackageCheck,
  RefreshCw,
  ShieldCheck,
  Star,
} from 'lucide-react';
import {
  Avatar,
  Badge,
  Button,
  ConfirmDialog,
  Search,
  Select,
  type SelectOption,
} from '@/component-library';
import {
  GalleryDetailModal,
  GalleryEmpty,
  GalleryGrid,
  GalleryLayout,
  GalleryPageHeader,
  GallerySkeleton,
  GalleryZone,
} from '@/app/components';
import { useSceneManager } from '@/app/hooks/useSceneManager';
import type { SceneTabId } from '@/app/components/SceneBar/types';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { useI18n } from '@/infrastructure/i18n';
import { isRemoteWorkspace } from '@/shared/types';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import type { MiniAppPermissions } from '@/infrastructure/api/service-api/MiniAppAPI';
import {
  miniAppMarketAPI,
  type MarketInstalledStatus,
  type MarketListingDetail,
  type MarketListingSummary,
  type MarketMe,
  type MarketSort,
} from '@/infrastructure/api/service-api/MiniAppMarketAPI';
import { createLogger } from '@/shared/utils/logger';
import { useNotification } from '@/shared/notification-system';
import { getMiniAppIconGradient, renderMiniAppIcon } from '../utils/miniAppIcons';
import { useMiniAppStore } from '../miniAppStore';
import { pickLocalizedString } from '../utils/pickLocalizedString';
import './MiniAppMarketView.scss';

const log = createLogger('MiniAppMarketView');
const CATEGORIES = [
  'all',
  'developer',
  'productivity',
  'data',
  'creative',
  'education',
  'utilities',
  'entertainment',
  'other',
] as const;

const MiniAppMarketView: React.FC = () => {
  const { t, formatNumber, currentLanguage } = useI18n('scenes/miniapp');
  const notification = useNotification();
  const { workspace } = useCurrentWorkspace();
  const { openScene, activateScene, openTabs } = useSceneManager();
  const upsertApp = useMiniAppStore((state) => state.upsertApp);
  const [query, setQuery] = useState('');
  const [category, setCategory] = useState<(typeof CATEGORIES)[number]>('all');
  const [sort, setSort] = useState<MarketSort>('newest');
  const [items, setItems] = useState<MarketListingSummary[]>([]);
  const [nextCursor, setNextCursor] = useState<string>();
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<string>();
  const [detail, setDetail] = useState<MarketListingDetail>();
  const [detailLoading, setDetailLoading] = useState(false);
  const [installed, setInstalled] = useState<MarketInstalledStatus | null>(null);
  const [me, setMe] = useState<MarketMe | null>(null);
  const [authBusy, setAuthBusy] = useState(false);
  const [actionBusy, setActionBusy] = useState(false);
  const [installPrompt, setInstallPrompt] = useState(false);

  const openTabIds = useMemo(() => new Set(openTabs.map((tab) => tab.id)), [openTabs]);

  /** Focuses the installed MiniApp's scene tab, opening it when absent. */
  const openInstalledApp = useCallback((appId: string) => {
    setDetail(undefined);
    setInstalled(null);
    const tabId: SceneTabId = `miniapp:${appId}`;
    if (openTabIds.has(tabId)) {
      activateScene(tabId);
    } else {
      openScene(tabId);
    }
  }, [activateScene, openScene, openTabIds]);

  const loadCatalog = useCallback(async (append = false) => {
    if (append) {
      setLoadingMore(true);
    } else {
      setLoading(true);
    }
    setError(undefined);
    try {
      const page = await miniAppMarketAPI.browse({
        query,
        category,
        sort,
        cursor: append ? nextCursor : undefined,
        limit: 20,
      });
      setItems((current) => append ? [...current, ...page.items] : page.items);
      setNextCursor(page.nextCursor);
    } catch (loadError) {
      log.error('Failed to load MiniApp marketplace catalog', loadError);
      setError(String(loadError));
    } finally {
      setLoading(false);
      setLoadingMore(false);
    }
  }, [category, nextCursor, query, sort]);

  useEffect(() => {
    const timeout = window.setTimeout(() => {
      void loadCatalog(false);
    }, 180);
    return () => window.clearTimeout(timeout);
  }, [category, query, sort]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    void miniAppMarketAPI.me().then(setMe).catch(() => setMe(null));
  }, []);

  const openDetail = async (summary: MarketListingSummary) => {
    setDetailLoading(true);
    setDetail(undefined);
    setInstalled(null);
    try {
      const loaded = await miniAppMarketAPI.getListing(summary.slug);
      setDetail(loaded);
      setInstalled(await miniAppMarketAPI.installedStatus(loaded.listingId));
    } catch (loadError) {
      notification.error(t('market.messages.detailFailed', { error: String(loadError) }));
    } finally {
      setDetailLoading(false);
    }
  };

  const signIn = async () => {
    setAuthBusy(true);
    try {
      const transaction = await miniAppMarketAPI.authStart();
      await systemAPI.openExternal(transaction.authorizationUrl);
      const deadline = transaction.expiresAt * 1000;
      while (Date.now() < deadline) {
        await new Promise((resolve) =>
          window.setTimeout(resolve, transaction.pollIntervalSeconds * 1000),
        );
        const status = await miniAppMarketAPI.authPoll(transaction);
        if (status === 'authorized') {
          const profile = await miniAppMarketAPI.me();
          setMe(profile);
          if (detail) {
            setDetail(await miniAppMarketAPI.getListing(detail.slug));
          }
          notification.success(t('market.messages.signedIn'));
          return;
        }
        if (status === 'expired') break;
      }
      notification.error(t('market.messages.authExpired'));
    } catch (authError) {
      notification.error(t('market.messages.authFailed', { error: String(authError) }));
    } finally {
      setAuthBusy(false);
    }
  };

  const signOut = async () => {
    setAuthBusy(true);
    try {
      await miniAppMarketAPI.logout();
      setMe(null);
      if (detail) {
        setDetail(await miniAppMarketAPI.getListing(detail.slug));
      }
      notification.success(t('market.messages.signedOut'));
    } catch (authError) {
      notification.error(t('market.messages.actionFailed', { error: String(authError) }));
    } finally {
      setAuthBusy(false);
    }
  };

  const toggleFavorite = async () => {
    if (!detail) return;
    if (!me) {
      await signIn();
      return;
    }
    setActionBusy(true);
    try {
      const result = await miniAppMarketAPI.setFavorite(detail.slug, !detail.isFavorited);
      setDetail({
        ...detail,
        isFavorited: result.isFavorited,
        favoriteCount: result.count,
      });
    } catch (actionError) {
      notification.error(t('market.messages.actionFailed', { error: String(actionError) }));
    } finally {
      setActionBusy(false);
    }
  };

  const rate = async (value: number) => {
    if (!detail) return;
    if (!me) {
      await signIn();
      return;
    }
    setActionBusy(true);
    try {
      const result = await miniAppMarketAPI.setRating(detail.slug, value);
      setDetail({
        ...detail,
        ratingAverage: result.average,
        ratingCount: result.count,
        myRating: result.myRating,
      });
    } catch (actionError) {
      notification.error(t('market.messages.actionFailed', { error: String(actionError) }));
    } finally {
      setActionBusy(false);
    }
  };

  const install = async () => {
    if (!detail) return;
    setInstallPrompt(false);
    setActionBusy(true);
    try {
      const result = await miniAppMarketAPI.install(detail.slug, detail.latestRelease, {
        existingAppId: installed?.appId,
        confirmPermissions: true,
        confirmOverwrite: Boolean(installed?.localOverride),
      });
      upsertApp(result.app);
      notification.success(
        t(result.updated ? 'market.messages.updated' : 'market.messages.installed'),
      );
      openInstalledApp(result.app.id);
    } catch (installError) {
      notification.error(t('market.messages.installFailed', { error: String(installError) }));
    } finally {
      setActionBusy(false);
    }
  };

  const permissionLines = detail ? summarizePermissions(detail.permissions, t) : [];
  const installedPermissionLines = installed
    ? summarizePermissions(installed.permissions, t)
    : [];
  const addedPermissionLines = permissionLines.filter(
    (line) => !installedPermissionLines.includes(line),
  );
  const removedPermissionLines = installedPermissionLines.filter(
    (line) => !permissionLines.includes(line),
  );
  const installedReleaseYanked = Boolean(
    detail
      && installed
      && detail.releases.find(
        (release) => release.releaseId === installed.origin.releaseId,
      )?.yanked,
  );
  const workspaceUnsupported = Boolean(
    detail
      && workspace
      && isRemoteWorkspace(workspace)
      && requiresWorkspace(detail.permissions),
  );
  const canUpdate = Boolean(installed && detail && detail.latestRelease > installed.origin.releaseNumber);
  // The install button only renders for the actionable states: fresh install or update.
  const installLabel = canUpdate ? t('market.detail.update') : t('market.detail.install');
  const detailName = detail
    ? pickLocalizedString(detail, currentLanguage, 'name')
    : '';
  const detailDescription = detail
    ? pickLocalizedString(detail, currentLanguage, 'description')
    : '';
  const sortOptions = useMemo<SelectOption[]>(() => [
    { value: 'newest', label: t('market.sort.newest') },
    { value: 'downloads', label: t('market.sort.downloads') },
    { value: 'rating', label: t('market.sort.rating') },
  ], [t]);

  return (
    <GalleryLayout className="miniapp-market-native">
      <GalleryPageHeader
        title={t('market.title')}
        subtitle={t('market.subtitle')}
        actions={(
          <div className="miniapp-market-native__header-actions">
            <Search
              value={query}
              onChange={setQuery}
              placeholder={t('market.search')}
              size="small"
            />
            {me ? (
              <div className="miniapp-market-native__identity">
                <Avatar size={22} src={me.user.avatarUrl} alt={me.user.login} />
                <span>@{me.user.login}</span>
                <Button
                  size="small"
                  variant="ghost"
                  iconOnly
                  onClick={() => void signOut()}
                  disabled={authBusy}
                  title={t('market.signOut')}
                  aria-label={t('market.signOut')}
                >
                  {authBusy ? <Loader2 size={14} className="gallery-spinning" /> : <LogOut size={14} />}
                </Button>
              </div>
            ) : (
              <Button size="small" variant="secondary" onClick={() => void signIn()} disabled={authBusy}>
                {authBusy ? <Loader2 size={14} className="gallery-spinning" /> : null}
                {t('market.signIn')}
              </Button>
            )}
          </div>
        )}
      />

      <div className="gallery-zones">
        <GalleryZone
          title={t('market.catalog')}
          tools={(
            <Select
              className="miniapp-market-native__sort"
              size="small"
              options={sortOptions}
              value={sort}
              onChange={(value) => setSort(value as MarketSort)}
              triggerAriaLabel={t('market.sortLabel')}
            />
          )}
        >
          <div className="gallery-chip-row">
            {CATEGORIES.map((value) => (
              <button
                key={value}
                type="button"
                className={[
                  'gallery-cat-chip',
                  value === category && 'gallery-cat-chip--active',
                ].filter(Boolean).join(' ')}
                onClick={() => setCategory(value)}
              >
                {categoryLabel(value, t)}
              </button>
            ))}
          </div>
          {loading ? <GallerySkeleton count={8} cardHeight={280} /> : null}
          {!loading && error ? (
            <GalleryEmpty
              icon={<AlertTriangle size={34} />}
              isError
              message={t('market.messages.catalogFailed', { error })}
              action={(
                <Button size="small" variant="secondary" onClick={() => void loadCatalog(false)}>
                  <RefreshCw size={14} />
                  {t('scene.retry')}
                </Button>
              )}
            />
          ) : null}
          {!loading && !error && items.length === 0 ? (
            <GalleryEmpty icon={<PackageCheck size={34} />} message={t('market.empty')} />
          ) : null}
          {!loading && items.length > 0 ? (
            <>
              <GalleryGrid minCardWidth={285}>
                {items.map((item) => {
                  const name = pickLocalizedString(item, currentLanguage, 'name');
                  const description = pickLocalizedString(item, currentLanguage, 'description');
                  return (
                    <button
                      key={item.listingId}
                      type="button"
                      className="miniapp-market-card"
                      onClick={() => void openDetail(item)}
                    >
                      <div className="miniapp-market-card__visual">
                        {item.screenshotUrls[0] ? (
                          <img src={item.screenshotUrls[0]} alt="" loading="lazy" />
                        ) : (
                          <span
                            className="miniapp-market-card__fallback"
                            style={{ background: getMiniAppIconGradient(item.icon || 'box') }}
                          >
                            {renderMiniAppIcon(item.icon || 'box', 30)}
                          </span>
                        )}
                      </div>
                      <div className="miniapp-market-card__body">
                        <div className="miniapp-market-card__eyebrow">
                          <span>{categoryLabel(item.category, t)}</span>
                          <span>v{item.latestRelease}</span>
                        </div>
                        <strong>{name}</strong>
                        <p>{description}</p>
                        <div className="miniapp-market-card__stats">
                          <span><Star size={12} /> {item.ratingAverage.toFixed(1)}</span>
                          <span><Download size={12} /> {formatNumber(item.downloadCount)}</span>
                          <span><Heart size={12} /> {formatNumber(item.favoriteCount)}</span>
                        </div>
                      </div>
                    </button>
                  );
                })}
              </GalleryGrid>
              {nextCursor ? (
                <div className="miniapp-market-native__load-more">
                  <Button
                    size="small"
                    variant="secondary"
                    disabled={loadingMore}
                    onClick={() => void loadCatalog(true)}
                  >
                    {loadingMore ? <Loader2 size={14} className="gallery-spinning" /> : null}
                    {t('market.loadMore')}
                  </Button>
                </div>
              ) : null}
            </>
          ) : null}
        </GalleryZone>
      </div>

      <GalleryDetailModal
        isOpen={detailLoading || Boolean(detail)}
        onClose={() => {
          setDetail(undefined);
          setInstalled(null);
        }}
        icon={detailLoading
          ? <Loader2 size={22} className="gallery-spinning" />
          : renderMiniAppIcon(detail?.icon || 'box', 24)}
        iconGradient={detail ? getMiniAppIconGradient(detail.icon || 'box') : undefined}
        title={detailName || t('market.detail.loading')}
        badges={detail ? (
          <>
            <Badge variant="info">{categoryLabel(detail.category, t)}</Badge>
            <Badge variant="neutral"><ShieldCheck size={11} /> {t('market.detail.reviewed')}</Badge>
          </>
        ) : null}
        description={detailDescription}
        meta={detail ? (
          <span>{t('market.detail.by', { owner: detail.owner.login })}</span>
        ) : null}
        actions={detail ? (
          <>
            <Button size="small" variant="secondary" onClick={() => void toggleFavorite()} disabled={actionBusy}>
              <Heart size={14} fill={detail.isFavorited ? 'currentColor' : 'none'} />
              {formatNumber(detail.favoriteCount)}
            </Button>
            {/* Installed and up to date: nothing left to install, so say so and let "Open" lead. */}
            {installed && !canUpdate ? (
              <span className="miniapp-market-detail__installed-state">
                <Check size={14} />
                {t('market.detail.installed')}
              </span>
            ) : null}
            {installed ? (
              <Button
                size="small"
                variant={canUpdate ? 'secondary' : 'primary'}
                disabled={actionBusy || workspaceUnsupported}
                onClick={() => openInstalledApp(installed.appId)}
              >
                <ArrowUpRight size={14} />
                {t('market.detail.open')}
              </Button>
            ) : null}
            {!installed || canUpdate ? (
              <Button
                size="small"
                variant="primary"
                disabled={actionBusy || workspaceUnsupported}
                onClick={() => setInstallPrompt(true)}
              >
                {actionBusy ? <Loader2 size={14} className="gallery-spinning" /> : canUpdate ? <RefreshCw size={14} /> : <Download size={14} />}
                {installLabel}
              </Button>
            ) : null}
          </>
        ) : null}
      >
        {detail ? (
          <div className="miniapp-market-detail">
            {detail.screenshotUrls.length ? (
              <div className="miniapp-market-detail__screenshots">
                {detail.screenshotUrls.map((url) => <img key={url} src={url} alt="" />)}
              </div>
            ) : null}
            {workspaceUnsupported ? (
              <div className="miniapp-market-detail__warning">
                <AlertTriangle size={16} />
                {t('market.detail.remoteWorkspaceUnsupported')}
              </div>
            ) : null}
            {installed?.localOverride ? (
              <div className="miniapp-market-detail__warning">
                <AlertTriangle size={16} />
                {t('market.detail.localOverride')}
              </div>
            ) : null}
            {installedReleaseYanked ? (
              <div className="miniapp-market-detail__warning">
                <AlertTriangle size={16} />
                {t('market.detail.installedYanked')}
              </div>
            ) : null}
            <section>
              <h4>{t('market.detail.permissions')}</h4>
              {permissionLines.length ? (
                <ul>{permissionLines.map((line) => <li key={line}>{line}</li>)}</ul>
              ) : (
                <p>{t('market.detail.noPermissions')}</p>
              )}
            </section>
            <section>
              <h4>{t('market.detail.rating')}</h4>
              <div className="miniapp-market-detail__rating">
                {[1, 2, 3, 4, 5].map((value) => (
                  <button key={value} type="button" onClick={() => void rate(value)} disabled={actionBusy}>
                    <Star
                      size={18}
                      fill={(detail.myRating ?? 0) >= value ? 'currentColor' : 'none'}
                    />
                  </button>
                ))}
                <span>{detail.ratingAverage.toFixed(1)} · {formatNumber(detail.ratingCount)}</span>
              </div>
            </section>
            <section>
              <h4>{t('market.detail.changelog')}</h4>
              <p>{detail.changelog}</p>
            </section>
            <section>
              <h4>{t('market.detail.releases')}</h4>
              <div className="miniapp-market-detail__releases">
                {detail.releases.map((release) => (
                  <div key={release.releaseId}>
                    <span>v{release.releaseNumber}</span>
                    <span>{release.minBitfunVersion}+</span>
                    {release.yanked ? <Badge variant="warning">{t('market.detail.yanked')}</Badge> : <Check size={14} />}
                  </div>
                ))}
              </div>
            </section>
            {detail.repositoryUrl ? (
              <Button
                size="small"
                variant="ghost"
                onClick={() => void systemAPI.openExternal(detail.repositoryUrl!)}
              >
                <ExternalLink size={14} />
                {t('market.detail.repository')}
              </Button>
            ) : null}
          </div>
        ) : null}
      </GalleryDetailModal>

      <ConfirmDialog
        isOpen={installPrompt}
        onClose={() => setInstallPrompt(false)}
        onConfirm={() => void install()}
        title={t(installed ? 'market.confirmUpdate.title' : 'market.confirmInstall.title', {
          name: detailName,
        })}
        message={[
          installed?.localOverride ? t('market.confirmUpdate.overwrite') : '',
          installed
            ? (
              addedPermissionLines.length
                ? t('market.confirmUpdate.addedPermissions', {
                  permissions: addedPermissionLines.join(', '),
                })
                : t('market.confirmUpdate.noAddedPermissions')
            )
            : (
              permissionLines.length
                ? t('market.confirmInstall.permissions', { permissions: permissionLines.join(', ') })
                : t('market.detail.noPermissions')
            ),
          installed && removedPermissionLines.length
            ? t('market.confirmUpdate.removedPermissions', {
              permissions: removedPermissionLines.join(', '),
            })
            : '',
        ].filter(Boolean).join('\n\n')}
        type="warning"
        confirmText={t(installed ? 'market.confirmUpdate.confirm' : 'market.confirmInstall.confirm')}
        cancelText={t('confirmDelete.cancel')}
      />
    </GalleryLayout>
  );
};

function requiresWorkspace(permissions: MiniAppPermissions): boolean {
  return [
    ...(permissions.fs?.read ?? []),
    ...(permissions.fs?.write ?? []),
  ].includes('{workspace}');
}

function summarizePermissions(
  permissions: MiniAppPermissions,
  t: (key: string, params?: Record<string, unknown>) => string,
): string[] {
  const result: string[] = [];
  if (permissions.fs?.read?.length) result.push(t('market.permissions.fsRead', { value: permissions.fs.read.join(', ') }));
  if (permissions.fs?.write?.length) result.push(t('market.permissions.fsWrite', { value: permissions.fs.write.join(', ') }));
  if (permissions.shell?.allow?.length) result.push(t('market.permissions.shell', { value: permissions.shell.allow.join(', ') }));
  if (permissions.net?.allow?.length) result.push(t('market.permissions.net', { value: permissions.net.allow.join(', ') }));
  if (permissions.ai?.enabled) result.push(t('market.permissions.ai'));
  if (permissions.agent?.enabled) result.push(t('market.permissions.agent'));
  if (permissions.notifications?.system) result.push(t('market.permissions.notifications'));
  if (permissions.host?.dialog) result.push(t('market.permissions.dialog'));
  if (permissions.host?.clipboard_read) result.push(t('market.permissions.clipboardRead'));
  if (permissions.host?.clipboard_write) result.push(t('market.permissions.clipboardWrite'));
  if (permissions.host?.open_external) result.push(t('market.permissions.openExternal'));
  if (permissions.host?.reveal_in_folder) result.push(t('market.permissions.revealInFolder'));
  if (permissions.host?.deck_render) result.push(t('market.permissions.deckRender'));
  if (permissions.host?.chat_composer) result.push(t('market.permissions.chatComposer'));
  if (permissions.host?.system_info) result.push(t('market.permissions.systemInfo'));
  return result;
}

function categoryLabel(
  category: string,
  t: (key: string) => string,
): string {
  switch (category) {
    case 'all': return t('market.categories.all');
    case 'developer': return t('market.categories.developer');
    case 'productivity': return t('market.categories.productivity');
    case 'data': return t('market.categories.data');
    case 'creative': return t('market.categories.creative');
    case 'education': return t('market.categories.education');
    case 'utilities': return t('market.categories.utilities');
    case 'entertainment': return t('market.categories.entertainment');
    default: return t('market.categories.other');
  }
}

export default MiniAppMarketView;
