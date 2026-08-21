/**
 * R-WF-18: independent workflow-member Claw list scene.
 *
 * Reuses the AssistantScene skeleton (workspace resolution + Suspense
 * loading) but renders the workflow-Claw list with its own data source
 * (workflowClawWorkspace partition of the shared assistant workspace list).
 * Opening a card jumps to the shared AssistantConfigPage (never re-created),
 * which resolves the workspace by assistantId.
 */

import React, { Suspense, useCallback, useMemo } from 'react';
import { Bot, GitBranch } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { useSceneManager } from '@/app/hooks/useSceneManager';
import { DotMatrixLoader } from '@/component-library';
import {
  GalleryLayout,
  GalleryPageHeader,
  GalleryZone,
  GalleryGrid,
  GalleryEmpty,
} from '@/app/components';
import { useNurseryStore } from '../profile/nurseryStore';
import { useMyAgentStore } from '../my-agent/myAgentStore';
import { splitAssistantWorkspacesByWorkflow } from './workflowClawWorkspace';
import WorkflowClawCard from './WorkflowClawCard';
import './WorkflowClawScene.scss';

const WorkflowClawScene: React.FC = () => {
  const { t } = useTranslation('scenes/profile');
  const { t: tCommon } = useI18n('common');
  const { assistantWorkspacesList } = useWorkspaceContext();
  const { openScene } = useSceneManager();
  const openAssistant = useNurseryStore((s) => s.openAssistant);
  const setSelectedAssistantWorkspaceId = useMyAgentStore((s) => s.setSelectedAssistantWorkspaceId);

  const { workflowClaws } = useMemo(
    () => splitAssistantWorkspacesByWorkflow(assistantWorkspacesList),
    [assistantWorkspacesList]
  );

  const handleOpen = useCallback((workspaceId: string) => {
    setSelectedAssistantWorkspaceId(workspaceId);
    openAssistant(workspaceId);
  }, [openAssistant, setSelectedAssistantWorkspaceId]);

  const handleCreateWorkflow = useCallback(() => {
    // The agents scene owns workflow orchestration (CreateLegionPage entry).
    openScene('agents');
  }, [openScene]);

  return (
    <div className="bitfun-workflow-claw-scene" data-bf-scene="workflow-claw" data-bf-part="root">
      <Suspense
        fallback={(
          <div
            className="bitfun-workflow-claw-scene__loading"
            data-bf-scene="workflow-claw"
            data-bf-part="loading"
            role="status"
            aria-busy="true"
            aria-label={tCommon('loading.scenes')}
          >
            <DotMatrixLoader size="medium" />
          </div>
        )}
      >
        <GalleryLayout
          className="workflow-claw-gallery"
          data-bf-component="workflow-claw-gallery"
          data-bf-part="root"
        >
          <GalleryPageHeader
            title={t('nursery.workflowClaw.gallery.title')}
            subtitle={t('nursery.workflowClaw.gallery.subtitle')}
            actions={(
              <button
                type="button"
                className="gallery-action-btn"
                onClick={handleCreateWorkflow}
                data-testid="workflow-claw-create-btn"
              >
                <GitBranch size={15} />
                <span>{t('nursery.workflowClaw.gallery.create')}</span>
              </button>
            )}
          />

          <GalleryZone
            id="workflow-claw-zone"
            title={t('nursery.workflowClaw.gallery.zoneTitle')}
            subtitle={t('nursery.workflowClaw.gallery.zoneSubtitle')}
          >
            {workflowClaws.length === 0 ? (
              <GalleryEmpty
                icon={<Bot size={32} strokeWidth={1.5} aria-hidden="true" />}
                message={(
                  <>
                    <strong>{t('nursery.workflowClaw.gallery.emptyTitle')}</strong>
                    <small>{t('nursery.workflowClaw.gallery.emptySubtitle')}</small>
                  </>
                )}
                className="workflow-claw-gallery__empty"
                testId="workflow-claw-empty"
              />
            ) : (
              <GalleryGrid
                minCardWidth={320}
                className="workflow-claw-gallery__grid"
                role="list"
              >
                {workflowClaws.map((workspace) => (
                  <WorkflowClawCard
                    key={workspace.id}
                    workspace={workspace}
                    onClick={() => handleOpen(workspace.id)}
                  />
                ))}
              </GalleryGrid>
            )}
          </GalleryZone>
        </GalleryLayout>
      </Suspense>
    </div>
  );
};

export default WorkflowClawScene;
