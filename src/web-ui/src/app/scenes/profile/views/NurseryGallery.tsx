import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Egg, Settings, Star, Wrench, BarChart2 } from 'lucide-react';
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
import type { AIModelConfig } from '@/infrastructure/config/types';
import { createLogger } from '@/shared/utils/logger';
import AssistantCard from './AssistantCard';
import { useNurseryStore } from '../nurseryStore';
import { estimateTokens, formatTokenCount } from './useTokenEstimate';

interface DeleteConfirmState {
  workspaceId: string;
  name: string;
}

const log = createLogger('NurseryGallery');

interface TemplateStats {
  primaryModelName: string;
  fastModelName: string;
  enabledToolCount: number;
}

const NurseryGallery: React.FC = () => {
  const { t } = useTranslation('scenes/profile');
  const { assistantWorkspacesList, createAssistantWorkspace, setActiveWorkspace, deleteAssistantWorkspace } = useWorkspaceContext();
  const openScene = useSceneStore(s => s.openScene);
  const { switchLeftPanelTab } = useApp();
  const { openTemplate, openAssistant } = useNurseryStore();
  const [creating, setCreating] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [deleteConfirm, setDeleteConfirm] = useState<DeleteConfirmState | null>(null);
  const [templateStats, setTemplateStats] = useState<TemplateStats | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const [allModels, funcModels, modeConf] = await Promise.all([
          configManager.getConfig<AIModelConfig[]>('ai.models').catch(() => [] as AIModelConfig[]),
          configManager.getConfig<Record<string, string>>('ai.func_agent_models').catch(() => ({} as Record<string, string>)),
          configAPI.getModeConfig('agentic').catch(() => null),
        ]);
        const models = allModels ?? [];
        const fm = funcModels ?? {};

        const resolveModelName = (slotId: string, fallback: string): string => {
          const id = fm[slotId] ?? '';
          if (!id || id === slotId) return fallback;
          const found = models.find((m) => m.id === id && m.enabled);
          return found?.name ?? fallback;
        };

        setTemplateStats({
          primaryModelName: resolveModelName('primary', t('nursery.template.stats.primaryDefault')),
          fastModelName: resolveModelName('fast', t('nursery.template.stats.fastDefault')),
          enabledToolCount: modeConf?.enabled_tools?.length ?? 0,
        });
      } catch (e) {
        log.error('Failed to load template stats', e);
      }
    })();
  }, [t]);

  const tokenBreakdown = useMemo(
    () => (templateStats ? estimateTokens('', templateStats.enabledToolCount, 0, 0) : null),
    [templateStats],
  );

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
          onClick={openTemplate}
          aria-label={t('nursery.template.title')}
        >
          <div className="nursery-template-card__content">
            <h3 className="nursery-template-card__title">{t('nursery.template.title')}</h3>
            <p className="nursery-template-card__subtitle">{t('nursery.template.subtitle')}</p>

            {/* Key stats */}
            {templateStats && tokenBreakdown && (
              <div className="nursery-template-card__stats">
                <span className="nursery-template-card__stat">
                  <Star size={10} strokeWidth={2} />
                  {templateStats.primaryModelName}
                </span>
                {templateStats.fastModelName !== templateStats.primaryModelName && (
                  <span className="nursery-template-card__stat nursery-template-card__stat--accent">
                    <Star size={10} strokeWidth={1.5} style={{ opacity: 0.7 }} />
                    {templateStats.fastModelName}
                  </span>
                )}
                <span className="nursery-template-card__stat nursery-template-card__stat--muted">
                  <Wrench size={10} strokeWidth={2} />
                  {t('nursery.template.stats.tools', { count: templateStats.enabledToolCount })}
                </span>
                <span className="nursery-template-card__stat nursery-template-card__stat--token">
                  <BarChart2 size={10} strokeWidth={2} />
                  ~{formatTokenCount(tokenBreakdown.total)} tok · {tokenBreakdown.percentage}
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
                  style={{ '--card-index': i } as React.CSSProperties}
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
