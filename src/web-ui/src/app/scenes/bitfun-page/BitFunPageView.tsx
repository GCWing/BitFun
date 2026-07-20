/**
 * BitFun Page master-detail view: Save version → Review preview → Deploy.
 */

import React, { useCallback, useEffect, useState } from 'react';
import {
  Copy,
  ExternalLink,
  FolderOpen,
  RefreshCw,
  Rocket,
  Save,
  Trash2,
} from 'lucide-react';
import { Alert, Button, Input } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { remoteConnectAPI } from '@/infrastructure/api/service-api/RemoteConnectAPI';
import {
  bitFunPageAPI,
  isValidPageSlug,
  type PageInfo,
  type PageVersionInfo,
  type PageVisibility,
} from '@/infrastructure/api/service-api/BitFunPageAPI';
import { useNotification } from '@/shared/notification-system';
import './BitFunPageView.scss';

const VISIBILITY_OPTIONS: PageVisibility[] = ['private', 'relay', 'public'];

const BitFunPageView: React.FC = () => {
  const { t, formatDate } = useI18n('scenes/bitfun-page');
  const { success, error: notifyError } = useNotification();

  const [loggedIn, setLoggedIn] = useState(false);
  const [relayUrl, setRelayUrl] = useState('');
  const [loading, setLoading] = useState(false);
  const [pages, setPages] = useState<PageInfo[]>([]);
  const [listError, setListError] = useState<string | null>(null);
  const [selectedSlug, setSelectedSlug] = useState<string | null>(null);

  const [directory, setDirectory] = useState('');
  const [slug, setSlug] = useState('');
  const [title, setTitle] = useState('');
  const [note, setNote] = useState('');
  const [visibility, setVisibility] = useState<PageVisibility>('private');
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const [versions, setVersions] = useState<PageVersionInfo[]>([]);
  const [versionsLoading, setVersionsLoading] = useState(false);
  const [deployingId, setDeployingId] = useState<string | null>(null);
  const [deleteConfirmSlug, setDeleteConfirmSlug] = useState('');

  const selectedPage = pages.find((p) => p.slug === selectedSlug) ?? null;

  const absoluteUrl = useCallback(
    (path: string) => {
      if (!path) return '';
      if (!relayUrl) return path;
      return `${relayUrl}${path.startsWith('/') ? '' : '/'}${path}`;
    },
    [relayUrl]
  );

  const loadVersions = useCallback(
    async (pageSlug: string) => {
      setVersionsLoading(true);
      try {
        const list = await bitFunPageAPI.listVersions(pageSlug);
        setVersions(list);
      } catch (e) {
        const message = e instanceof Error ? e.message : String(e);
        notifyError(message);
        setVersions([]);
      } finally {
        setVersionsLoading(false);
      }
    },
    [notifyError]
  );

  const selectPage = useCallback((page: PageInfo) => {
    setSelectedSlug(page.slug);
    setSlug(page.slug);
    setTitle(page.title || '');
    setVisibility(
      (VISIBILITY_OPTIONS.includes(page.visibility as PageVisibility)
        ? page.visibility
        : 'private') as PageVisibility
    );
    setDeleteConfirmSlug('');
    setSaveError(null);
  }, []);

  const refresh = useCallback(async () => {
    setLoading(true);
    setListError(null);
    try {
      const status = await remoteConnectAPI.accountStatus();
      setLoggedIn(Boolean(status.logged_in));
      if (!status.logged_in) {
        setPages([]);
        setRelayUrl('');
        setSelectedSlug(null);
        setVersions([]);
        return;
      }
      const hint = await remoteConnectAPI.accountGetCredentialHint();
      setRelayUrl(hint?.relay_url?.replace(/\/$/, '') ?? '');
      const list = await bitFunPageAPI.list();
      setPages(list);
      setSelectedSlug((prev) => {
        const next =
          prev && list.some((p) => p.slug === prev) ? prev : list[0]?.slug ?? null;
        if (next) {
          const page = list.find((p) => p.slug === next);
          if (page) {
            setSlug(page.slug);
            setTitle(page.title || '');
            setVisibility(
              (VISIBILITY_OPTIONS.includes(page.visibility as PageVisibility)
                ? page.visibility
                : 'private') as PageVisibility
            );
          }
        }
        return next;
      });
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      setListError(message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    if (!selectedSlug || !loggedIn) {
      if (!selectedSlug) setVersions([]);
      return;
    }
    void loadVersions(selectedSlug);
  }, [selectedSlug, loggedIn, loadVersions]);

  const handlePickDirectory = useCallback(async () => {
    try {
      const { pickWorkspaceDirectory } = await import(
        '@/infrastructure/peer-device/pickWorkspaceDirectory'
      );
      const selected = await pickWorkspaceDirectory({
        title: t('pickDirectoryTitle'),
      });
      if (selected) {
        setDirectory(selected);
        setSaveError(null);
      }
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      notifyError(message);
    }
  }, [notifyError, t]);

  const handleSaveVersion = useCallback(async () => {
    setSaveError(null);
    if (!directory) {
      setSaveError(t('errors.directoryRequired'));
      return;
    }
    if (!isValidPageSlug(slug)) {
      setSaveError(t('errors.invalidSlug'));
      return;
    }
    setSaving(true);
    try {
      const result = await bitFunPageAPI.saveVersion({
        directory,
        slug,
        visibility,
        title: title.trim() || undefined,
        note: note.trim() || undefined,
      });
      success(
        t('saveVersionSuccess', {
          slug: result.slug,
          versionId: result.version_id,
        })
      );
      setSelectedSlug(result.slug);
      await refresh();
      await loadVersions(result.slug);
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      setSaveError(message);
    } finally {
      setSaving(false);
    }
  }, [
    directory,
    loadVersions,
    note,
    refresh,
    slug,
    success,
    t,
    title,
    visibility,
  ]);

  const handleCopy = useCallback(
    async (url: string) => {
      if (!url) return;
      try {
        await navigator.clipboard.writeText(url);
        success(t('copied'));
      } catch {
        notifyError(t('errors.copyFailed'));
      }
    },
    [notifyError, success, t]
  );

  const handlePreview = useCallback(
    (version: PageVersionInfo) => {
      const url = absoluteUrl(version.preview_url_path);
      if (!url) {
        notifyError(t('errors.openPreviewFailed'));
        return;
      }
      window.open(url, '_blank', 'noopener,noreferrer');
    },
    [absoluteUrl, notifyError, t]
  );

  const handleDeploy = useCallback(
    async (version: PageVersionInfo) => {
      if (!selectedPage) return;
      setDeployingId(version.version_id);
      try {
        const updated = await bitFunPageAPI.deploy({
          slug: selectedPage.slug,
          version_id: version.version_id,
        });
        setPages((prev) => prev.map((p) => (p.slug === updated.slug ? updated : p)));
        await loadVersions(selectedPage.slug);
        success(t('deploySuccess', { versionId: version.version_id }));
      } catch (e) {
        const message = e instanceof Error ? e.message : String(e);
        notifyError(message);
      } finally {
        setDeployingId(null);
      }
    },
    [loadVersions, notifyError, selectedPage, success, t]
  );

  const handleDeleteVersion = useCallback(
    async (version: PageVersionInfo) => {
      if (!selectedPage || version.deployed) return;
      const confirmed = window.confirm(
        t('confirmDeleteVersion', { versionId: version.version_id })
      );
      if (!confirmed) return;
      try {
        await bitFunPageAPI.deleteVersion(selectedPage.slug, version.version_id);
        await loadVersions(selectedPage.slug);
        await refresh();
        success(t('deleteVersionSuccess'));
      } catch (e) {
        const message = e instanceof Error ? e.message : String(e);
        notifyError(message);
      }
    },
    [loadVersions, notifyError, refresh, selectedPage, success, t]
  );

  const handleVisibilityChange = useCallback(
    async (next: PageVisibility) => {
      if (!selectedPage || selectedPage.visibility === next) return;
      try {
        const updated = await bitFunPageAPI.update({
          slug: selectedPage.slug,
          visibility: next,
        });
        setPages((prev) => prev.map((p) => (p.slug === updated.slug ? updated : p)));
        setVisibility(next);
        success(t('visibilityUpdated'));
      } catch (e) {
        const message = e instanceof Error ? e.message : String(e);
        notifyError(message);
      }
    },
    [notifyError, selectedPage, success, t]
  );

  const handleDeletePage = useCallback(async () => {
    if (!selectedPage) return;
    if (deleteConfirmSlug !== selectedPage.slug) {
      notifyError(t('errors.slugMismatch'));
      return;
    }
    try {
      await bitFunPageAPI.unpublish(selectedPage.slug);
      success(t('unpublishSuccess'));
      setDeleteConfirmSlug('');
      setSelectedSlug(null);
      await refresh();
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      notifyError(message);
    }
  }, [deleteConfirmSlug, notifyError, refresh, selectedPage, success, t]);

  const productionUrl =
    selectedPage?.deployed_version_id && selectedPage.url_path
      ? absoluteUrl(selectedPage.url_path)
      : '';

  if (!loggedIn) {
    return (
      <div className="bitfun-page-view bitfun-page-view--empty" data-testid="bitfun-page-view">
        <Alert type="info" message={t('loginRequired')} />
      </div>
    );
  }

  return (
    <div className="bitfun-page-view" data-testid="bitfun-page-view">
      <aside className="bitfun-page-view__sidebar">
        <div className="bitfun-page-view__sidebar-header">
          <h2 className="bitfun-page-view__sidebar-title">{t('manageHeading')}</h2>
          <Button
            type="button"
            variant="ghost"
            size="small"
            onClick={() => void refresh()}
            disabled={loading}
            aria-label={t('refresh')}
            iconOnly
          >
            <RefreshCw size={14} />
          </Button>
        </div>
        {listError && <Alert type="error" message={listError} />}
        {loading && <p className="bitfun-page-view__hint">{t('loading')}</p>}
        {!loading && pages.length === 0 && (
          <p className="bitfun-page-view__hint">{t('empty')}</p>
        )}
        <ul className="bitfun-page-view__site-list">
          {pages.map((page) => (
            <li key={page.slug}>
              <button
                type="button"
                className={[
                  'bitfun-page-view__site-item',
                  selectedSlug === page.slug && 'is-selected',
                ]
                  .filter(Boolean)
                  .join(' ')}
                onClick={() => selectPage(page)}
                data-testid={`bitfun-page-site-${page.slug}`}
              >
                <span className="bitfun-page-view__site-name">
                  {page.title || page.slug}
                </span>
                <span className="bitfun-page-view__site-meta">
                  {page.slug}
                  {page.deployed_version_id ? (
                    <span className="bitfun-page-view__badge">{t('deployedBadge')}</span>
                  ) : (
                    <span className="bitfun-page-view__badge bitfun-page-view__badge--muted">
                      {t('notDeployed')}
                    </span>
                  )}
                </span>
              </button>
            </li>
          ))}
        </ul>
        <button
          type="button"
          className={[
            'bitfun-page-view__site-item bitfun-page-view__site-item--new',
            selectedSlug === null && 'is-selected',
          ]
            .filter(Boolean)
            .join(' ')}
          onClick={() => {
            setSelectedSlug(null);
            setSlug('');
            setTitle('');
            setNote('');
            setVisibility('private');
            setVersions([]);
            setDeleteConfirmSlug('');
          }}
          data-testid="bitfun-page-new-site"
        >
          {t('newSite')}
        </button>
      </aside>

      <main className="bitfun-page-view__main">
        <section className="bitfun-page-view__section">
          <h3 className="bitfun-page-view__heading">{t('publishHeading')}</h3>
          <p className="bitfun-page-view__hint">{t('publishHint')}</p>
          <p className="bitfun-page-view__hint">{t('reviewHint')}</p>

          <div className="bitfun-page-view__field">
            <label>{t('directory')}</label>
            <div className="bitfun-page-view__row">
              <Input
                value={directory}
                readOnly
                placeholder={t('directoryPlaceholder')}
                data-testid="bitfun-page-directory"
              />
              <Button
                type="button"
                variant="secondary"
                onClick={() => void handlePickDirectory()}
                data-testid="bitfun-page-pick-dir"
              >
                <FolderOpen size={14} />
                {t('pickDirectory')}
              </Button>
            </div>
          </div>

          <div className="bitfun-page-view__field">
            <label htmlFor="bitfun-page-slug">{t('slug')}</label>
            <Input
              id="bitfun-page-slug"
              value={slug}
              onChange={(e) => setSlug(e.target.value.trim().toLowerCase())}
              placeholder={t('slugPlaceholder')}
              data-testid="bitfun-page-slug"
            />
            <span className="bitfun-page-view__field-hint">{t('slugHint')}</span>
          </div>

          <div className="bitfun-page-view__field">
            <label htmlFor="bitfun-page-title">{t('pageTitle')}</label>
            <Input
              id="bitfun-page-title"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder={t('pageTitlePlaceholder')}
            />
          </div>

          <div className="bitfun-page-view__field">
            <label htmlFor="bitfun-page-note">{t('versionNote')}</label>
            <Input
              id="bitfun-page-note"
              value={note}
              onChange={(e) => setNote(e.target.value)}
              placeholder={t('versionNotePlaceholder')}
            />
          </div>

          <fieldset className="bitfun-page-view__visibility">
            <legend>{t('visibility')}</legend>
            {VISIBILITY_OPTIONS.map((option) => (
              <label key={option} className="bitfun-page-view__radio">
                <input
                  type="radio"
                  name="page-visibility"
                  value={option}
                  checked={visibility === option}
                  onChange={() => {
                    setVisibility(option);
                    if (selectedPage && selectedPage.visibility !== option) {
                      void handleVisibilityChange(option);
                    }
                  }}
                />
                <span>
                  <strong>{t(`visibilityOptions.${option}.label`)}</strong>
                  <span className="bitfun-page-view__radio-desc">
                    {t(`visibilityOptions.${option}.description`)}
                  </span>
                </span>
              </label>
            ))}
          </fieldset>

          {saveError && <Alert type="error" message={saveError} />}

          <div className="bitfun-page-view__actions">
            <Button
              type="button"
              variant="primary"
              disabled={saving}
              isLoading={saving}
              onClick={() => void handleSaveVersion()}
              data-testid="bitfun-page-save-version"
            >
              <Save size={14} />
              {saving ? t('savingVersion') : t('saveVersion')}
            </Button>
          </div>
        </section>

        {selectedPage && (
          <>
            <section className="bitfun-page-view__section">
              <h3 className="bitfun-page-view__heading">{t('siteDetail')}</h3>
              <div className="bitfun-page-view__detail-row">
                <span className="bitfun-page-view__detail-label">{t('productionUrl')}</span>
                {productionUrl ? (
                  <div className="bitfun-page-view__row">
                    <code className="bitfun-page-view__url">{productionUrl}</code>
                    <Button
                      type="button"
                      variant="ghost"
                      size="small"
                      onClick={() => void handleCopy(productionUrl)}
                      aria-label={t('copyUrl')}
                    >
                      <Copy size={14} />
                    </Button>
                  </div>
                ) : (
                  <span className="bitfun-page-view__hint">{t('notDeployed')}</span>
                )}
              </div>
              {selectedPage.deployed_version_id && (
                <p className="bitfun-page-view__hint">
                  {t('deployedVersion', {
                    versionId: selectedPage.deployed_version_id,
                  })}
                </p>
              )}
            </section>

            <section className="bitfun-page-view__section">
              <h3 className="bitfun-page-view__heading">{t('versions')}</h3>
              {versionsLoading && (
                <p className="bitfun-page-view__hint">{t('loadingVersions')}</p>
              )}
              {!versionsLoading && versions.length === 0 && (
                <p className="bitfun-page-view__hint">{t('noVersions')}</p>
              )}
              <ul className="bitfun-page-view__version-list">
                {versions.map((version) => (
                  <li
                    key={version.version_id}
                    className="bitfun-page-view__version"
                    data-testid={`bitfun-page-version-${version.version_id}`}
                  >
                    <div className="bitfun-page-view__version-main">
                      <span className="bitfun-page-view__version-id">
                        {version.version_id}
                        {version.deployed && (
                          <span className="bitfun-page-view__badge">{t('deployedBadge')}</span>
                        )}
                        {version.has_worker && (
                          <span className="bitfun-page-view__badge bitfun-page-view__badge--worker">
                            {t('hasWorker')}
                          </span>
                        )}
                      </span>
                      <span className="bitfun-page-view__version-meta">
                        {version.created_at
                          ? formatDate(
                              version.created_at < 1e12
                                ? version.created_at * 1000
                                : version.created_at,
                              {
                                year: 'numeric',
                                month: 'short',
                                day: 'numeric',
                                hour: '2-digit',
                                minute: '2-digit',
                              }
                            )
                          : ''}
                        {version.note ? ` · ${version.note}` : ''}
                      </span>
                    </div>
                    <div className="bitfun-page-view__version-actions">
                      <Button
                        type="button"
                        variant="ghost"
                        size="small"
                        onClick={() => handlePreview(version)}
                        aria-label={t('preview')}
                      >
                        <ExternalLink size={14} />
                        {t('preview')}
                      </Button>
                      <Button
                        type="button"
                        variant="secondary"
                        size="small"
                        disabled={
                          version.deployed || deployingId === version.version_id
                        }
                        isLoading={deployingId === version.version_id}
                        onClick={() => void handleDeploy(version)}
                        data-testid={`bitfun-page-deploy-${version.version_id}`}
                      >
                        <Rocket size={14} />
                        {t('deploy')}
                      </Button>
                      <Button
                        type="button"
                        variant="ghost"
                        size="small"
                        disabled={version.deployed}
                        onClick={() => void handleDeleteVersion(version)}
                        aria-label={t('deleteVersion')}
                      >
                        <Trash2 size={14} />
                      </Button>
                    </div>
                  </li>
                ))}
              </ul>
            </section>

            <section className="bitfun-page-view__section bitfun-page-view__section--danger">
              <h3 className="bitfun-page-view__heading">{t('unpublish')}</h3>
              <p className="bitfun-page-view__hint">
                {t('deletePageHint', { slug: selectedPage.slug })}
              </p>
              <div className="bitfun-page-view__field">
                <label htmlFor="bitfun-page-delete-confirm">{t('deleteConfirmLabel')}</label>
                <Input
                  id="bitfun-page-delete-confirm"
                  value={deleteConfirmSlug}
                  onChange={(e) => setDeleteConfirmSlug(e.target.value.trim().toLowerCase())}
                  placeholder={selectedPage.slug}
                  data-testid="bitfun-page-delete-confirm"
                />
              </div>
              <Button
                type="button"
                variant="secondary"
                disabled={deleteConfirmSlug !== selectedPage.slug}
                onClick={() => void handleDeletePage()}
                data-testid="bitfun-page-delete-page"
              >
                <Trash2 size={14} />
                {t('unpublish')}
              </Button>
            </section>
          </>
        )}
      </main>
    </div>
  );
};

export default BitFunPageView;
