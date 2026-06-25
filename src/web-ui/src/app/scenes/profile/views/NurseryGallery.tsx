import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Egg, Settings, Star, Wrench } from 'lucide-react';
import {
  GalleryLayout,
  GalleryPageHeader,
  GalleryZone,
  GalleryGrid,
} from '@/app/components';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { useApp } from '@/app/hooks/useApp';
import { useSceneStore } from '@/app/stores/sceneStore';
import { flowChatManager } from '@/flow_chat/services/FlowChatManager';
import type { WorkspaceInfo } from '@/shared/types';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import type { AIModelConfig, DefaultModelsConfig } from '@/infrastructure/config/types';
import { getModelDisplayName } from '@/infrastructure/config/services/modelConfigs';
import { createLogger } from '@/shared/utils/logger';
import AssistantCard from './AssistantCard';
import { useNurseryStore } from '../nurseryStore';

interface DeleteConfirmState {
  workspaceId: string;
  name: string;
}

const log = createLogger('NurseryGallery');
const ASSISTANT_MODE_ID = 'Claw';

interface TemplateStats {
  defaultModelName: string;
  enabledToolCount: number;
}

const NurseryGallery: React.FC = () => {
  const { t } = useTranslation('scenes/profile');
  const { assistantWorkspacesList, createAssistantWorkspace, setActiveWorkspace, deleteAssistantWorkspace } = useWorkspaceContext();
  const openScene = useSceneStore(s => s.openScene);
  const { switchLeftPanelTab } = useApp();
  const { openDefaults, openAssistant } = useNurseryStore();
  const [creating, setCreating] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [deleteConfirm, setDeleteConfirm] = useState<DeleteConfirmState | null>(null);
  const [templateStats, setTemplateStats] = useState<TemplateStats | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const [allModels, defaultModels, agentModels, modeConf] = await Promise.all([
          configManager.getConfig<AIModelConfig[]>('ai.models').catch(() => [] as AIModelConfig[]),
          configManager.getConfig<DefaultModelsConfig>('ai.default_models').catch(() => ({} as DefaultModelsConfig)),
          configManager.getConfig<Record<string, string>>('ai.agent_models').catch(() => ({} as Record<string, string>)),
          configAPI.getAgentProfileConfig(ASSISTANT_MODE_ID).catch(() => null),
        ]);
        const models = allModels ?? [];
        const defaults = defaultModels ?? {};
        const configuredAgentModels = agentModels ?? {};

        const findEnabledModelByRef = (modelRef?: string | null): AIModelConfig | null => {
          const trimmed = modelRef?.trim();
          if (!trimmed) return null;
          return models.find((model) => model.enabled && model.id === trimmed) ?? null;
        };

        const resolveClawDefaultModelName = (): string => {
          const configuredModel = configuredAgentModels[ASSISTANT_MODE_ID]?.trim() || 'auto';
          if (configuredModel === 'auto') {
            return t('nursery.template.stats.autoDefault');
          }
          if (configuredModel === 'primary') {
            return findEnabledModelByRef(defaults.primary)
              ? getModelDisplayName(findEnabledModelByRef(defaults.primary)!)
              : t('nursery.template.stats.primaryDefault');
          }
          if (configuredModel === 'fast') {
            const fastModel = findEnabledModelByRef(defaults.fast) ?? findEnabledModelByRef(defaults.primary);
            return fastModel ? getModelDisplayName(fastModel) : t('nursery.template.stats.fastDefault');
          }

          const explicitModel = findEnabledModelByRef(configuredModel);
          return explicitModel ? getModelDisplayName(explicitModel) : configuredModel;
        };

        setTemplateStats({
          defaultModelName: resolveClawDefaultModelName(),
          enabledToolCount: modeConf?.enabled_tools?.length ?? 0,
        });
      } catch (e) {
        log.error('Failed to load template stats', e);
      }
    })();
  }, [t]);

  const handleCreateAssistant = useCallback(async () => {
    if (creating) return;
    setCreating(true);
    try {
      const newWorkspace = await createAssistantWorkspace();
      openAssistant(newWorkspace.id);
    } catch (e) {
      log.error('Failed to create assistant workspace', e);
    } finally {
      setCreating(false);
    }
  }, [creating, createAssistantWorkspace, openAssistant]);

  const sortedAssistantWorkspacesList = useMemo(
    () => {
      const primary = assistantWorkspacesList.filter(w => !w.assistantId);
      const secondary = assistantWorkspacesList.filter(w => w.assistantId);
      return [...primary, ...secondary];
    },
    [assistantWorkspacesList]
  );

  const handleDeleteRequest = useCallback((workspace: WorkspaceInfo) => {
    const identity = workspace.identity;
    const name = identity?.name?.trim() || workspace.name || t('nursery.card.unnamed');
    setDeleteConfirm({ workspaceId: workspace.id, name });
  }, [t]);

  const handleDeleteConfirm = useCallback(async () => {
    if (!deleteConfirm || deleting) return;
    setDeleting(true);
    try {
      await deleteAssistantWorkspace(deleteConfirm.workspaceId);
    } catch (e) {
      log.error('Failed to delete assistant workspace', e);
    } finally {
      setDeleting(false);
      setDeleteConfirm(null);
    }
  }, [deleteConfirm, deleting, deleteAssistantWorkspace]);

  const handleDeleteCancel = useCallback(() => {
    setDeleteConfirm(null);
  }, []);

  const handleNewAssistantSession = useCallback(
    async (workspace: WorkspaceInfo) => {
      openScene('session');
      switchLeftPanelTab('sessions');
      try {
        await flowChatManager.createChatSession({ workspacePath: workspace.rootPath }, 'Claw');
        await setActiveWorkspace(workspace.id);
      } catch (e) {
        log.error('Failed to create assistant session from gallery', e);
      }
    },
    [openScene, setActiveWorkspace, switchLeftPanelTab],
  );

  return (
    <GalleryLayout className="nursery-gallery">
      <GalleryPageHeader
        title={t('nursery.gallery.title')}
        subtitle={t('nursery.gallery.subtitle')}
        actions={(
          <button
            type="button"
            className="gallery-action-btn gallery-action-btn--primary"
            onClick={handleCreateAssistant}
            disabled={creating}
          >
            <Plus size={15} />
            <span>{t('nursery.gallery.newAssistant')}</span>
          </button>
        )}
      />

      {/* Template hero: panda + card side by side, bottom-aligned */}
      <div className="nursery-template-hero">
        {/* Panda — hover independently, no card linkage */}
        <div className="nursery-template-panda">
          <img
            className="nursery-template-panda__img nursery-template-panda__img--default"
            src="/panda_full_1.png"
            alt=""
            onError={(e) => { (e.target as HTMLImageElement).src = '/Logo-ICON.png'; }}
          />
          <img
            className="nursery-template-panda__img nursery-template-panda__img--hover"
            src="/panda_full_2.png"
            alt=""
            onError={(e) => { (e.target as HTMLImageElement).src = '/Logo-ICON.png'; }}
          />
        </div>

        {/* Card */}
        <button
          type="button"
          className="nursery-template-card"
          onClick={openDefaults}
          aria-label={t('nursery.template.title')}
        >
          <div className="nursery-template-card__content">
            <h3 className="nursery-template-card__title">{t('nursery.template.title')}</h3>
            <p className="nursery-template-card__subtitle">{t('nursery.template.subtitle')}</p>

            {/* Key stats */}
            {templateStats && (
              <div className="nursery-template-card__stats">
                <span className="nursery-template-card__stat">
                  <Star size={10} strokeWidth={2} />
                  {templateStats.defaultModelName}
                </span>
                <span className="nursery-template-card__stat nursery-template-card__stat--muted">
                  <Wrench size={10} strokeWidth={2} />
                  {t('nursery.template.stats.tools', { count: templateStats.enabledToolCount })}
                </span>
              </div>
            )}

            <span className="nursery-template-card__action">
              <Settings size={13} strokeWidth={1.8} />
              <span>{t('nursery.template.configure')}</span>
            </span>
          </div>

          {/* Decorative eggs */}
          <div className="nursery-template-card__deco" aria-hidden="true">
            <Egg size={56} strokeWidth={1} className="nursery-template-card__deco-egg nursery-template-card__deco-egg--1" />
            <Egg size={32} strokeWidth={1} className="nursery-template-card__deco-egg nursery-template-card__deco-egg--2" />
          </div>
        </button>
      </div>

      {/* Assistants */}
      <div className="gallery-zones">
        <GalleryZone
          id="nursery-assistants-zone"
          title={t('nursery.gallery.assistantsTitle')}
          subtitle={t('nursery.gallery.assistantsSubtitle')}
          tools={(
            <span className="gallery-zone-count">{sortedAssistantWorkspacesList.length}</span>
          )}
        >
          <GalleryGrid minCardWidth={360}>
            {sortedAssistantWorkspacesList.map((workspace, i) => {
              const isPrimary = !workspace.assistantId;
              return (
                <AssistantCard
                  key={workspace.id}
                  workspace={workspace}
                  isPrimary={isPrimary}
                  onClick={() => openAssistant(workspace.id)}
                  onNewSession={() => { void handleNewAssistantSession(workspace); }}
                  onDelete={isPrimary ? undefined : () => handleDeleteRequest(workspace)}
                  style={{ '--surface-stagger-index': i } as React.CSSProperties}
                />
              );
            })}
          </GalleryGrid>
        </GalleryZone>
      </div>

      {/* Delete confirmation dialog */}
      {deleteConfirm && (
        <div className="nursery-delete-overlay" role="dialog" aria-modal="true">
          <div className="nursery-delete-dialog">
            <h3 className="nursery-delete-dialog__title">{t('nursery.card.deleteConfirmTitle')}</h3>
            <p className="nursery-delete-dialog__message">
              {t('nursery.card.deleteConfirmMessage', { name: deleteConfirm.name })}
            </p>
            <div className="nursery-delete-dialog__actions">
              <button
                type="button"
                className="nursery-delete-dialog__btn nursery-delete-dialog__btn--cancel"
                onClick={handleDeleteCancel}
                disabled={deleting}
              >
                {t('nursery.card.deleteCancel')}
              </button>
              <button
                type="button"
                className="nursery-delete-dialog__btn nursery-delete-dialog__btn--confirm"
                onClick={() => { void handleDeleteConfirm(); }}
                disabled={deleting}
              >
                {t('nursery.card.deleteConfirm')}
              </button>
            </div>
          </div>
        </div>
      )}
    </GalleryLayout>
  );
};

export default NurseryGallery;
