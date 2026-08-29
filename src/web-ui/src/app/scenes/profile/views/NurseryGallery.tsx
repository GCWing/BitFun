import { Button, Icon } from '@bitfun/ui';
import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Bot, CircleAlert, LoaderCircle, Wrench } from 'lucide-react';
import {
  GalleryEmpty,
  GalleryLayout,
  GalleryPageHeader,
  GalleryZone,
  GalleryGrid,
  GallerySkeleton,
} from '@/app/components';
import { confirmDanger } from '@/infrastructure/confirm-dialog';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { useApp } from '@/app/hooks/useApp';
import { useSceneStore } from '@/app/stores/sceneStore';
import { flowChatManager } from '@/flow_chat/services/FlowChatManager';
import type { WorkspaceInfo } from '@/shared/types';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import AssistantCard from './AssistantCard';
import { useNurseryStore } from '../nurseryStore';
import './NurseryView.scss';

const log = createLogger('NurseryGallery');
const ASSISTANT_MODE_ID = 'Claw';

interface TemplateStats {
  enabledToolCount: number;
  enabledSkillCount: number;
}

type TemplateStatsStatus = 'loading' | 'ready' | 'error';

const NurseryDefaultsArtwork: React.FC = () => (
  <div
    className="nursery-defaults__artwork"
    aria-hidden="true"
    data-bf-component="nursery-gallery"
    data-bf-part="artwork"
  >
    <img
      className="nursery-defaults__artwork-image"
      data-bf-component="nursery-gallery"
      data-bf-part="avatar"
      src="/assets/assistant/defaults-illustration.webp"
      alt=""
      decoding="async"
      draggable={false}
    />
  </div>
);

const NurseryGallery: React.FC = () => {
  const { t } = useTranslation('scenes/profile');
  const {
    assistantWorkspacesList,
    createAssistantWorkspace,
    deleteAssistantWorkspace,
    primaryAssistantWorkspaceId,
    error: workspaceError,
    loading: workspaceLoading,
    setActiveWorkspace,
    setPrimaryAssistantWorkspace,
  } = useWorkspaceContext();
  const openScene = useSceneStore(s => s.openScene);
  const { switchLeftPanelTab } = useApp();
  const { openDefaults, openAssistant } = useNurseryStore();
  const notification = useNotification();
  const [creating, setCreating] = useState(false);
  const [deletingWorkspaceId, setDeletingWorkspaceId] = useState<string | null>(null);
  const [settingPrimaryWorkspaceId, setSettingPrimaryWorkspaceId] = useState<string | null>(null);
  const [startingSessionWorkspaceId, setStartingSessionWorkspaceId] = useState<string | null>(null);
  const [templateStats, setTemplateStats] = useState<TemplateStats | null>(null);
  const [templateStatsStatus, setTemplateStatsStatus] = useState<TemplateStatsStatus>('loading');

  useEffect(() => {
    let cancelled = false;

    void (async () => {
      setTemplateStatsStatus('loading');
      try {
        const [modeConf, skills] = await Promise.all([
          configAPI.getAgentProfileConfig(ASSISTANT_MODE_ID),
          configAPI.getModeSkillConfigs({ modeId: ASSISTANT_MODE_ID }),
        ]);

        if (cancelled) return;
        setTemplateStats({
          enabledToolCount: modeConf?.enabled_tools?.length ?? 0,
          enabledSkillCount: skills.filter((skill) => skill.effectiveEnabled).length,
        });
        setTemplateStatsStatus('ready');
      } catch (e) {
        log.error('Failed to load template stats', e);
        if (!cancelled) {
          setTemplateStats(null);
          setTemplateStatsStatus('error');
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, []);

  const handleCreateAssistant = useCallback(async () => {
    if (creating) return;
    setCreating(true);
    try {
      const newWorkspace = await createAssistantWorkspace();
      openAssistant(newWorkspace.id);
    } catch (e) {
      log.error('Failed to create assistant workspace', e);
      notification.error(t('nursery.gallery.createFailed'));
    } finally {
      setCreating(false);
    }
  }, [creating, createAssistantWorkspace, notification, openAssistant, t]);

  const sortedAssistantWorkspacesList = useMemo(
    () => {
      const primary = assistantWorkspacesList.filter(w => w.id === primaryAssistantWorkspaceId);
      const secondary = assistantWorkspacesList.filter(w => w.id !== primaryAssistantWorkspaceId);
      return [...primary, ...secondary];
    },
    [assistantWorkspacesList, primaryAssistantWorkspaceId]
  );
  const showGalleryCompanion = sortedAssistantWorkspacesList.length > 0
    && sortedAssistantWorkspacesList.length <= 2;

  const handleSetPrimary = useCallback(async (workspace: WorkspaceInfo) => {
    if (settingPrimaryWorkspaceId || workspace.id === primaryAssistantWorkspaceId) return;
    setSettingPrimaryWorkspaceId(workspace.id);
    try {
      await setPrimaryAssistantWorkspace(workspace.id);
      notification.success(t('nursery.card.setPrimarySuccess'));
    } catch (e) {
      log.error('Failed to set primary assistant workspace', e);
      notification.error(t('nursery.card.setPrimaryFailed'));
    } finally {
      setSettingPrimaryWorkspaceId(null);
    }
  }, [
    notification,
    primaryAssistantWorkspaceId,
    setPrimaryAssistantWorkspace,
    settingPrimaryWorkspaceId,
    t,
  ]);

  const handleDeleteRequest = useCallback(async (workspace: WorkspaceInfo) => {
    if (deletingWorkspaceId) return;
    const identity = workspace.identity;
    const name = identity?.name?.trim() || workspace.name || t('nursery.card.unnamed');
    const confirmed = await confirmDanger(
      t('nursery.card.deleteConfirmTitle'),
      t('nursery.card.deleteConfirmMessage', { name }),
      {
        confirmText: t('nursery.card.deleteConfirm'),
        cancelText: t('nursery.card.deleteCancel'),
      },
    );
    if (!confirmed) return;

    setDeletingWorkspaceId(workspace.id);
    try {
      await deleteAssistantWorkspace(workspace.id);
    } catch (e) {
      log.error('Failed to delete assistant workspace', e);
      notification.error(t('nursery.card.deleteFailed'));
    } finally {
      setDeletingWorkspaceId(null);
    }
  }, [deleteAssistantWorkspace, deletingWorkspaceId, notification, t]);

  const handleNewAssistantSession = useCallback(
    async (workspace: WorkspaceInfo) => {
      if (startingSessionWorkspaceId) return;
      setStartingSessionWorkspaceId(workspace.id);
      openScene('session');
      switchLeftPanelTab('sessions');
      try {
        await flowChatManager.createChatSession({ workspacePath: workspace.rootPath }, 'Claw');
        await setActiveWorkspace(workspace.id);
      } catch (e) {
        log.error('Failed to create assistant session from gallery', e);
        notification.error(t('nursery.card.newSessionFailed'));
      } finally {
        setStartingSessionWorkspaceId(null);
      }
    },
    [
      notification,
      openScene,
      setActiveWorkspace,
      startingSessionWorkspaceId,
      switchLeftPanelTab,
      t,
    ],
  );

  return (
    <GalleryLayout
      className="nursery-gallery"
      data-bf-component="nursery-gallery"
      data-bf-part="root"
    >
      <GalleryPageHeader
        title={(
          <span className="nursery-gallery__page-title">
            {t('nursery.gallery.title')}
            <Icon name="spark" size="lg" aria-hidden="true" />
          </span>
        )}
        subtitle={t('nursery.gallery.subtitle')}
        actions={(
          <Button
            type="button"
            variant="fill"
            size="sm"
            onClick={handleCreateAssistant}
            disabled={creating}
            aria-busy={creating}
          >
            {creating ? (
              <LoaderCircle className="nursery-spinning" size={15} aria-hidden="true" />
            ) : (
              <Icon name="plus" size="sm" aria-hidden="true" />
            )}
            <span>
              {t(creating ? 'nursery.gallery.creating' : 'nursery.gallery.newAssistant')}
            </span>
          </Button>
        )}
      />

      <div className="gallery-zones" data-bf-component="nursery-gallery" data-bf-part="content">
        <section className="nursery-defaults" aria-labelledby="nursery-defaults-title" data-bf-component="nursery-gallery" data-bf-part="defaults">
          <NurseryDefaultsArtwork />

          <div className="nursery-defaults__content" data-bf-component="nursery-gallery" data-bf-part="defaultsContent">
            <div className="nursery-defaults__title-row">
              <h3 className="nursery-defaults__title" id="nursery-defaults-title">
                {t('nursery.template.title')}
              </h3>
              <span className="nursery-defaults__badge">{t('nursery.template.defaultBadge')}</span>
            </div>
            <p className="nursery-defaults__subtitle">{t('nursery.template.subtitle')}</p>

            <div
              className="nursery-defaults__stats"
              data-bf-component="nursery-gallery"
              data-bf-part="stats"
              aria-live="polite"
              aria-busy={templateStatsStatus === 'loading'}
            >
              {templateStatsStatus === 'loading' ? (
                <>
                  <span className="nursery-defaults__stat-skeleton" aria-hidden="true" />
                  <span className="nursery-defaults__stat-skeleton" aria-hidden="true" />
                </>
              ) : templateStatsStatus === 'error' ? (
                <span className="nursery-defaults__stat nursery-defaults__stat--error">
                  <CircleAlert size={13} strokeWidth={1.8} aria-hidden="true" />
                  {t('nursery.template.statsUnavailable')}
                </span>
              ) : templateStats ? (
                <>
                  <span className="nursery-defaults__stat">
                    <Wrench size={13} strokeWidth={1.8} aria-hidden="true" />
                    {t('nursery.template.stats.tools', { count: templateStats.enabledToolCount })}
                  </span>
                  <span className="nursery-defaults__stat">
                    <Icon name="extension" size="xs" aria-hidden="true" />
                    {t('nursery.template.stats.skills', { count: templateStats.enabledSkillCount })}
                  </span>
                </>
              ) : null}
            </div>
          </div>

          <button
            type="button"
            className="nursery-defaults__action"
            onClick={openDefaults}
          >
            <span className="nursery-defaults__action-icon" aria-hidden="true">
              <Icon name="settings" size="md" />
            </span>
            <span className="nursery-defaults__action-label">{t('nursery.template.configure')}</span>
            <Icon name="chevron-right" size="sm" className="nursery-defaults__action-chevron" aria-hidden="true" />
          </button>
        </section>

        <GalleryZone
          id="nursery-assistants-zone"
          className="nursery-gallery__assistant-zone"
          title={t('nursery.gallery.assistantsTitle')}
          subtitle={t('nursery.gallery.assistantsSubtitle')}
          tools={(
            <span className="gallery-zone-count">{sortedAssistantWorkspacesList.length}</span>
          )}
        >
          {workspaceLoading && sortedAssistantWorkspacesList.length === 0 ? (
            <GallerySkeleton
              count={3}
              cardHeight={168}
              minCardWidth={340}
              className="nursery-gallery__skeleton"
            />
          ) : workspaceError && sortedAssistantWorkspacesList.length === 0 ? (
            <GalleryEmpty
              icon={<CircleAlert size={32} strokeWidth={1.5} aria-hidden="true" />}
              message={t('nursery.gallery.loadFailed')}
              isError
              className="nursery-gallery__empty"
              testId="nursery-gallery-error"
            />
          ) : sortedAssistantWorkspacesList.length === 0 ? (
            <GalleryEmpty
              icon={<Bot size={32} strokeWidth={1.5} aria-hidden="true" />}
              message={(
                <>
                  <strong>{t('nursery.gallery.emptyTitle')}</strong>
                  <small>{t('nursery.gallery.emptySubtitle')}</small>
                </>
              )}
              action={(
                <Button
                  type="button"
                  variant="fill"
                  size="sm"
                  onClick={handleCreateAssistant}
                  disabled={creating}
                  leadingIcon={<Icon name="plus" size="sm" aria-hidden="true" />}
                >

                  {t('nursery.gallery.newAssistant')}
                </Button>
              )}
              className="nursery-gallery__empty"
              testId="nursery-gallery-empty"
            />
          ) : (
            <div className={[
              'nursery-gallery__assistant-showcase',
              showGalleryCompanion && 'nursery-gallery__assistant-showcase--with-companion',
            ].filter(Boolean).join(' ')}>
              <GalleryGrid
                minCardWidth={340}
                className="nursery-gallery__assistant-grid"
                role="list"
              >
                {sortedAssistantWorkspacesList.map((workspace, i) => {
                  const isPrimary = workspace.id === primaryAssistantWorkspaceId;
                  return (
                    <AssistantCard
                      key={workspace.id}
                      workspace={workspace}
                      isPrimary={isPrimary}
                      isDeleting={deletingWorkspaceId === workspace.id}
                      isStartingSession={startingSessionWorkspaceId === workspace.id}
                      isSettingPrimary={settingPrimaryWorkspaceId === workspace.id}
                      onClick={() => openAssistant(workspace.id)}
                      onNewSession={() => { void handleNewAssistantSession(workspace); }}
                      onDelete={isPrimary ? undefined : () => { void handleDeleteRequest(workspace); }}
                      onSetPrimary={isPrimary ? undefined : () => { void handleSetPrimary(workspace); }}
                      style={{ '--surface-stagger-index': i } as React.CSSProperties}
                    />
                  );
                })}
              </GalleryGrid>

              {showGalleryCompanion ? (
                <img
                  className="nursery-gallery__companion"
                  data-bf-component="nursery-gallery"
                  data-bf-part="companion"
                  src="/assets/assistant/gallery-companion.webp"
                  alt=""
                  aria-hidden="true"
                  decoding="async"
                  loading="lazy"
                  draggable={false}
                />
              ) : null}
            </div>
          )}
        </GalleryZone>
      </div>
    </GalleryLayout>
  );
};

export default NurseryGallery;
