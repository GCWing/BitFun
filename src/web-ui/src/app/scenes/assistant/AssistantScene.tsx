import React, { Suspense, lazy, useMemo, useEffect } from 'react';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { WorkspaceKind } from '@/shared/types';
import { ProcessingIndicator } from '@/flow_chat/components/modern/ProcessingIndicator';
import { useMyAgentStore } from '../my-agent/myAgentStore';
import './AssistantScene.scss';

const ProfileScene = lazy(() => import('../profile/ProfileScene'));

interface AssistantSceneProps {
  workspacePath?: string;
}

const AssistantScene: React.FC<AssistantSceneProps> = ({ workspacePath }) => {
  const { t } = useI18n('common');
  const selectedAssistantWorkspaceId = useMyAgentStore((s) => s.selectedAssistantWorkspaceId);
  const setSelectedAssistantWorkspaceId = useMyAgentStore((s) => s.setSelectedAssistantWorkspaceId);
  const { currentWorkspace, assistantWorkspacesList } = useWorkspaceContext();
  const activeAssistantWorkspace =
    currentWorkspace?.workspaceKind === WorkspaceKind.Assistant ? currentWorkspace : null;

  const defaultAssistantWorkspace = useMemo(
    () => assistantWorkspacesList.find((workspace) => !workspace.assistantId) ?? assistantWorkspacesList[0] ?? null,
    [assistantWorkspacesList]
  );

  const selectedAssistantWorkspace = useMemo(() => {
    if (!selectedAssistantWorkspaceId) {
      return null;
    }
    return assistantWorkspacesList.find((workspace) => workspace.id === selectedAssistantWorkspaceId) ?? null;
  }, [assistantWorkspacesList, selectedAssistantWorkspaceId]);

  const resolvedAssistantWorkspace = useMemo(() => {
    if (activeAssistantWorkspace) {
      return activeAssistantWorkspace;
    }
    if (selectedAssistantWorkspace) {
      return selectedAssistantWorkspace;
    }
    return defaultAssistantWorkspace;
  }, [activeAssistantWorkspace, defaultAssistantWorkspace, selectedAssistantWorkspace]);

  useEffect(() => {
    if (activeAssistantWorkspace?.id && activeAssistantWorkspace.id !== selectedAssistantWorkspaceId) {
      setSelectedAssistantWorkspaceId(activeAssistantWorkspace.id);
    }
  }, [activeAssistantWorkspace, selectedAssistantWorkspaceId, setSelectedAssistantWorkspaceId]);

  useEffect(() => {
    const selectedExists = selectedAssistantWorkspaceId
      ? assistantWorkspacesList.some((workspace) => workspace.id === selectedAssistantWorkspaceId)
      : false;

    if (activeAssistantWorkspace?.id) {
      return;
    }

    if (!selectedExists && resolvedAssistantWorkspace?.id !== selectedAssistantWorkspaceId) {
      setSelectedAssistantWorkspaceId(resolvedAssistantWorkspace?.id ?? null);
    }
  }, [
    activeAssistantWorkspace,
    assistantWorkspacesList,
    resolvedAssistantWorkspace,
    selectedAssistantWorkspaceId,
    setSelectedAssistantWorkspaceId,
  ]);

  return (
    <div className="bitfun-assistant-scene">
      <Suspense
        fallback={(
          <div
            className="bitfun-assistant-scene__loading"
            role="status"
            aria-busy="true"
            aria-label={t('loading.scenes')}
          >
            <ProcessingIndicator visible />
          </div>
        )}
      >
        <ProfileScene
          key={resolvedAssistantWorkspace?.id ?? 'default-assistant-workspace'}
          workspacePath={resolvedAssistantWorkspace?.rootPath ?? workspacePath}
        />
      </Suspense>
    </div>
  );
};

export default AssistantScene;
