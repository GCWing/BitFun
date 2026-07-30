import React, { useEffect, useMemo, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import {
  AlertTriangle,
  Camera,
  FileImage,
  Github,
  Loader2,
  PackageOpen,
  RefreshCw,
  Send,
  X,
} from 'lucide-react';
import { Badge, Button, Input } from '@/component-library';
import { GalleryEmpty, GalleryLayout, GalleryPageHeader } from '@/app/components';
import { useI18n } from '@/infrastructure/i18n';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import {
  miniAppMarketAPI,
  type MarketMe,
  type MarketSubmission,
  type MarketSubmissionDraftRequest,
  type MarketUploadProgress,
} from '@/infrastructure/api/service-api/MiniAppMarketAPI';
import type { MiniAppMeta } from '@/infrastructure/api/service-api/MiniAppAPI';
import { miniAppAPI } from '@/infrastructure/api/service-api/MiniAppAPI';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { isRemoteWorkspace } from '@/shared/types';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { useSceneManager } from '@/app/hooks/useSceneManager';
import type { SceneTabId } from '@/app/components/SceneBar/types';
import './MiniAppSubmissionsView.scss';

const log = createLogger('MiniAppSubmissionsView');

const MARKET_CATEGORIES = [
  'developer',
  'productivity',
  'data',
  'creative',
  'education',
  'utilities',
  'entertainment',
  'other',
] as const;

const emptyDraft: MarketSubmissionDraftRequest = {
  slug: '',
  releaseNumber: 1,
  name: '',
  description: '',
  icon: 'box',
  category: 'other',
  tags: [],
  minBitfunVersion: '0.1.0',
  changelog: '',
  license: { spdxExpression: 'MIT' },
};

const MiniAppSubmissionsView: React.FC = () => {
  const { t } = useI18n('scenes/miniapp');
  const notification = useNotification();
  const { workspace } = useCurrentWorkspace();
  const { openScene, activateScene, openTabs } = useSceneManager();
  const [me, setMe] = useState<MarketMe | null>(null);
  const [authResolved, setAuthResolved] = useState(false);
  const [authBusy, setAuthBusy] = useState(false);
  const [apps, setApps] = useState<MiniAppMeta[]>([]);
  const [submissions, setSubmissions] = useState<MarketSubmission[]>([]);
  const [selectedAppId, setSelectedAppId] = useState('');
  const [draft, setDraft] = useState<MarketSubmissionDraftRequest>(emptyDraft);
  const [screenshotPaths, setScreenshotPaths] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  const [progress, setProgress] = useState<MarketUploadProgress>();

  const localActionsDisabled = Boolean(workspace && isRemoteWorkspace(workspace));
  const selectedApp = useMemo(
    () => apps.find((app) => app.id === selectedAppId),
    [apps, selectedAppId],
  );

  const refresh = async () => {
    setLoading(true);
    try {
      const profile = await miniAppMarketAPI.me();
      setMe(profile);
      const installed = await miniAppAPI.listMiniApps();
      setApps(installed);
      if (!selectedAppId && installed[0]) {
        selectApp(installed[0]);
      }
      if (profile) {
        setSubmissions(await miniAppMarketAPI.listSubmissions());
      } else {
        setSubmissions([]);
      }
    } catch (error) {
      log.error('Failed to load MiniApp submission workspace', error);
      notification.error(t('market.messages.submissionsFailed', { error: String(error) }));
    } finally {
      setAuthResolved(true);
      setLoading(false);
    }
  };

  useEffect(() => {
    void refresh();
    const stop = miniAppMarketAPI.onUploadProgress(setProgress);
    return stop;
    // The submission workspace intentionally loads once when its native tab mounts.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function selectApp(app: MiniAppMeta) {
    setSelectedAppId(app.id);
    setDraft((current) => ({
      ...current,
      slug: current.slug || suggestSlug(app.name),
      name: app.name,
      description: app.description,
      icon: app.icon || 'box',
      category: normalizeCategory(app.category),
      tags: app.tags,
    }));
  }

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
          await refresh();
          notification.success(t('market.messages.signedIn'));
          return;
        }
        if (status === 'expired') break;
      }
      notification.error(t('market.messages.authExpired'));
    } catch (error) {
      notification.error(t('market.messages.authFailed', { error: String(error) }));
    } finally {
      setAuthBusy(false);
    }
  };

  const chooseScreenshots = async () => {
    const selected = await open({
      multiple: true,
      directory: false,
      title: t('market.submissions.chooseScreenshots'),
      filters: [
        {
          name: t('market.submissions.screenshotFiles'),
          extensions: ['png', 'jpg', 'jpeg', 'webp'],
        },
      ],
    });
    const paths = Array.isArray(selected) ? selected : selected ? [selected] : [];
    if (paths.length > 5) {
      notification.error(t('market.submissions.screenshotLimit'));
      return;
    }
    setScreenshotPaths(paths);
  };

  const captureCurrentApp = async () => {
    if (!selectedApp) return;
    setBusy(true);
    const tabId: SceneTabId = `miniapp:${selectedApp.id}`;
    try {
      if (openTabs.some((tab) => tab.id === tabId)) {
        activateScene(tabId);
      } else {
        openScene(tabId);
      }
      await new Promise((resolve) => window.setTimeout(resolve, 1200));
      const path = await miniAppMarketAPI.captureWindow();
      setScreenshotPaths((current) => [path, ...current].slice(0, 5));
      notification.success(t('market.submissions.captured'));
    } catch (error) {
      notification.error(t('market.submissions.captureFailed', { error: String(error) }));
    } finally {
      activateScene('miniapps');
      setBusy(false);
    }
  };

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!selectedApp || screenshotPaths.length === 0) {
      notification.error(t('market.submissions.missingFiles'));
      return;
    }
    setBusy(true);
    setProgress({ phase: 'validating', completed: 0, total: 1 });
    try {
      const created = await miniAppMarketAPI.submitInstalled(
        selectedApp.id,
        {
          ...draft,
          slug: draft.slug.trim().toLowerCase(),
          name: draft.name.trim(),
          description: draft.description.trim(),
          changelog: draft.changelog.trim(),
          tags: draft.tags.map((tag) => tag.trim()).filter(Boolean),
          repositoryUrl: draft.repositoryUrl?.trim() || undefined,
          license: {
            spdxExpression: draft.license.spdxExpression?.trim() || undefined,
            customUrl: draft.license.customUrl?.trim() || undefined,
          },
        },
        screenshotPaths,
      );
      setSubmissions((current) => [created, ...current]);
      setDraft((current) => ({
        ...current,
        listingId: created.listingId,
        releaseNumber: created.releaseNumber + 1,
        changelog: '',
      }));
      setScreenshotPaths([]);
      notification.success(t('market.submissions.submitted'));
    } catch (error) {
      notification.error(t('market.submissions.submitFailed', { error: String(error) }));
    } finally {
      setBusy(false);
    }
  };

  const withdraw = async (submissionId: string) => {
    setBusy(true);
    try {
      const updated = await miniAppMarketAPI.withdrawSubmission(submissionId);
      setSubmissions((current) =>
        current.map((item) => item.submissionId === submissionId ? updated : item),
      );
      notification.success(t('market.submissions.withdrawn'));
    } catch (error) {
      notification.error(t('market.submissions.withdrawFailed', { error: String(error) }));
    } finally {
      setBusy(false);
    }
  };

  if (!authResolved || loading) {
    return (
      <GalleryLayout className="miniapp-submissions">
        <div className="miniapp-submissions__loading"><Loader2 className="gallery-spinning" /></div>
      </GalleryLayout>
    );
  }

  if (!me) {
    return (
      <GalleryLayout className="miniapp-submissions">
        <GalleryPageHeader
          title={t('market.submissions.title')}
          subtitle={t('market.submissions.subtitle')}
        />
        <GalleryEmpty
          icon={<Github size={36} />}
          message={t('market.submissions.signInRequired')}
          action={(
            <Button variant="primary" onClick={() => void signIn()} disabled={authBusy}>
              {authBusy ? <Loader2 size={14} className="gallery-spinning" /> : <Github size={14} />}
              {t('market.signIn')}
            </Button>
          )}
        />
      </GalleryLayout>
    );
  }

  return (
    <GalleryLayout className="miniapp-submissions">
      <GalleryPageHeader
        title={t('market.submissions.title')}
        subtitle={t('market.submissions.subtitle')}
        actions={(
          <Button size="small" variant="ghost" onClick={() => void refresh()} disabled={busy}>
            <RefreshCw size={14} />
            {t('market.submissions.refresh')}
          </Button>
        )}
      />

      {localActionsDisabled ? (
        <div className="miniapp-submissions__remote-warning">
          <AlertTriangle size={16} />
          {t('market.submissions.remoteUnsupported')}
        </div>
      ) : null}

      <div className="miniapp-submissions__workspace">
        <form className="miniapp-submissions__form" onSubmit={(event) => void submit(event)}>
          <div className="miniapp-submissions__section-heading">
            <PackageOpen size={18} />
            <div>
              <h3>{t('market.submissions.newTitle')}</h3>
              <p>{t('market.submissions.newHint')}</p>
            </div>
          </div>

          <label className="miniapp-submissions__field">
            <span>{t('market.submissions.app')}</span>
            <select
              value={selectedAppId}
              onChange={(event) => {
                const app = apps.find((item) => item.id === event.target.value);
                if (app) selectApp(app);
              }}
              disabled={busy || localActionsDisabled}
            >
              {apps.map((app) => <option key={app.id} value={app.id}>{app.name}</option>)}
            </select>
          </label>

          <div className="miniapp-submissions__form-grid">
            <Input
              label={t('market.submissions.slug')}
              value={draft.slug}
              pattern="[a-z0-9][a-z0-9-]{2,62}"
              required
              disabled={busy}
              onChange={(event) => setDraft({ ...draft, slug: event.target.value })}
              hint={t('market.submissions.slugHint')}
            />
            <Input
              label={t('market.submissions.release')}
              type="number"
              min={1}
              max={4294967295}
              value={draft.releaseNumber}
              required
              disabled={busy}
              onChange={(event) =>
                setDraft({ ...draft, releaseNumber: Number(event.target.value) })
              }
            />
          </div>

          <Input
            label={t('market.submissions.name')}
            value={draft.name}
            maxLength={80}
            required
            disabled={busy}
            onChange={(event) => setDraft({ ...draft, name: event.target.value })}
          />

          <label className="miniapp-submissions__field">
            <span>{t('market.submissions.description')}</span>
            <textarea
              value={draft.description}
              maxLength={500}
              required
              disabled={busy}
              onChange={(event) => setDraft({ ...draft, description: event.target.value })}
            />
          </label>

          <div className="miniapp-submissions__form-grid">
            <label className="miniapp-submissions__field">
              <span>{t('market.submissions.category')}</span>
              <select
                value={draft.category}
                disabled={busy}
                onChange={(event) => setDraft({ ...draft, category: event.target.value })}
              >
                {MARKET_CATEGORIES.map((value) => (
                  <option key={value} value={value}>
                    {marketCategoryLabel(value, t)}
                  </option>
                ))}
              </select>
            </label>
            <Input
              label={t('market.submissions.minVersion')}
              value={draft.minBitfunVersion}
              required
              disabled={busy}
              onChange={(event) => setDraft({ ...draft, minBitfunVersion: event.target.value })}
            />
          </div>

          <Input
            label={t('market.submissions.tags')}
            value={draft.tags.join(', ')}
            disabled={busy}
            onChange={(event) =>
              setDraft({ ...draft, tags: event.target.value.split(',').slice(0, 10) })
            }
            hint={t('market.submissions.tagsHint')}
          />

          <label className="miniapp-submissions__field">
            <span>{t('market.submissions.changelog')}</span>
            <textarea
              value={draft.changelog}
              maxLength={4000}
              required
              disabled={busy}
              onChange={(event) => setDraft({ ...draft, changelog: event.target.value })}
            />
          </label>

          <div className="miniapp-submissions__form-grid">
            <Input
              label={t('market.submissions.spdx')}
              value={draft.license.spdxExpression ?? ''}
              disabled={busy}
              onChange={(event) =>
                setDraft({
                  ...draft,
                  license: { ...draft.license, spdxExpression: event.target.value },
                })
              }
            />
            <Input
              label={t('market.submissions.licenseUrl')}
              type="url"
              value={draft.license.customUrl ?? ''}
              disabled={busy}
              onChange={(event) =>
                setDraft({
                  ...draft,
                  license: { ...draft.license, customUrl: event.target.value },
                })
              }
            />
          </div>

          <Input
            label={t('market.submissions.repository')}
            type="url"
            value={draft.repositoryUrl ?? ''}
            disabled={busy}
            onChange={(event) => setDraft({ ...draft, repositoryUrl: event.target.value })}
          />

          <div className="miniapp-submissions__screenshots">
            <div>
              <span>{t('market.submissions.screenshots')}</span>
              <small>{t('market.submissions.screenshotHint')}</small>
            </div>
            <Button
              type="button"
              size="small"
              variant="secondary"
              disabled={busy || localActionsDisabled}
              onClick={() => void chooseScreenshots()}
            >
              <Camera size={14} />
              {t('market.submissions.choose')}
            </Button>
            <Button
              type="button"
              size="small"
              variant="secondary"
              disabled={busy || localActionsDisabled || !selectedApp}
              onClick={() => void captureCurrentApp()}
            >
              <Camera size={14} />
              {t('market.submissions.capture')}
            </Button>
          </div>
          {screenshotPaths.length ? (
            <div className="miniapp-submissions__files">
              {screenshotPaths.map((path) => (
                <div key={path}>
                  <FileImage size={14} />
                  <span title={path}>{fileName(path)}</span>
                  <button
                    type="button"
                    aria-label={t('market.submissions.removeScreenshot')}
                    onClick={() =>
                      setScreenshotPaths((current) => current.filter((item) => item !== path))
                    }
                  >
                    <X size={13} />
                  </button>
                </div>
              ))}
            </div>
          ) : null}

          {progress && busy ? (
            <div className="miniapp-submissions__progress">
              <div>
                <span>{progressLabel(progress.phase, t)}</span>
                <strong>{progress.completed}/{progress.total}</strong>
              </div>
              <progress value={progress.completed} max={Math.max(progress.total, 1)} />
            </div>
          ) : null}

          <Button
            type="submit"
            variant="primary"
            disabled={busy || localActionsDisabled || apps.length === 0}
          >
            {busy ? <Loader2 size={15} className="gallery-spinning" /> : <Send size={15} />}
            {t('market.submissions.submit')}
          </Button>
        </form>

        <section className="miniapp-submissions__history">
          <div className="miniapp-submissions__section-heading">
            <RefreshCw size={18} />
            <div>
              <h3>{t('market.submissions.history')}</h3>
              <p>{t('market.submissions.historyHint')}</p>
            </div>
          </div>
          {submissions.length ? (
            <div className="miniapp-submissions__list">
              {submissions.map((submission) => (
                <article key={submission.submissionId}>
                  <div>
                    <span className="miniapp-submissions__app-icon">{submission.icon}</span>
                    <div>
                      <strong>{submission.name}</strong>
                      <small>{submission.slug} · v{submission.releaseNumber}</small>
                    </div>
                  </div>
                  <div className="miniapp-submissions__status">
                    <Badge variant={statusVariant(submission.status)}>
                      {submissionStatusLabel(submission.status, t)}
                    </Badge>
                    {(submission.status === 'draft' || submission.status === 'submitted') ? (
                      <Button
                        size="small"
                        variant="ghost"
                        disabled={busy}
                        onClick={() => void withdraw(submission.submissionId)}
                      >
                        {t('market.submissions.withdraw')}
                      </Button>
                    ) : null}
                  </div>
                  {submission.rejectionReason ? (
                    <p>{t('market.submissions.rejection', {
                      reason: submission.rejectionReason,
                    })}</p>
                  ) : null}
                </article>
              ))}
            </div>
          ) : (
            <div className="miniapp-submissions__history-empty">
              {t('market.submissions.noSubmissions')}
            </div>
          )}
        </section>
      </div>
    </GalleryLayout>
  );
};

function suggestSlug(name: string): string {
  const value = name
    .normalize('NFKD')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 63);
  return value.length >= 3 ? value : 'my-miniapp';
}

function normalizeCategory(category: string): string {
  const aliases: Record<string, string> = {
    utility: 'utilities',
    game: 'entertainment',
    games: 'entertainment',
    dev: 'developer',
  };
  const normalized = aliases[category] ?? category;
  return MARKET_CATEGORIES.includes(normalized as (typeof MARKET_CATEGORIES)[number])
    ? normalized
    : 'other';
}

function fileName(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}

function statusVariant(
  status: MarketSubmission['status'],
): 'neutral' | 'info' | 'success' | 'warning' {
  switch (status) {
    case 'approved': return 'success';
    case 'submitted': return 'info';
    case 'rejected': return 'warning';
    default: return 'neutral';
  }
}

function marketCategoryLabel(
  category: (typeof MARKET_CATEGORIES)[number],
  t: (key: string) => string,
): string {
  switch (category) {
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

function progressLabel(
  phase: MarketUploadProgress['phase'],
  t: (key: string) => string,
): string {
  switch (phase) {
    case 'validating': return t('market.submissions.progress.validating');
    case 'package': return t('market.submissions.progress.package');
    case 'screenshots': return t('market.submissions.progress.screenshots');
    case 'submitted': return t('market.submissions.progress.submitted');
  }
}

function submissionStatusLabel(
  status: MarketSubmission['status'],
  t: (key: string) => string,
): string {
  switch (status) {
    case 'draft': return t('market.submissions.status.draft');
    case 'submitted': return t('market.submissions.status.submitted');
    case 'approved': return t('market.submissions.status.approved');
    case 'rejected': return t('market.submissions.status.rejected');
    case 'withdrawn': return t('market.submissions.status.withdrawn');
  }
}

export default MiniAppSubmissionsView;
