import React, { Suspense, lazy } from 'react';
import { useMemo, useEffect } from 'react';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { WorkspaceKind } from '@/shared/types';
import { useMyAgentStore } from './myAgentStore';
import './MyAgentScene.scss';

const ProfileScene = lazy(() => import('../profile/ProfileScene'));
const AgentsScene = lazy(() => import('../agents/AgentsScene'));
const SkillsScene = lazy(() => import('../skills/SkillsScene'));
const InsightsScene = lazy(() => import('./InsightsScene'));

interface MyAgentSceneProps {
  workspacePath?: string;
}

const MyAgentScene: React.FC<MyAgentSceneProps> = ({ workspacePath }) => {
  const activeView = useMyAgentStore((s) => s.activeView);
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

    return assistantWorkspacesList.find(
      (workspace) => workspace.id === selectedAssistantWorkspaceId
    ) ?? null;
  }, [assistantWorkspacesList, selectedAssistantWorkspaceId]);

  const resolvedAssistantWorkspace = useMemo(() => {
    if (activeAssistantWorkspace) {
      return activeAssistantWorkspace;
    }

    if (selectedAssistantWorkspace) {
      return selectedAssistantWorkspace;
    }

    return defaultAssistantWorkspace;
  }, [
    activeAssistantWorkspace,
    defaultAssistantWorkspace,
    selectedAssistantWorkspace,
  ]);

  useEffect(() => {
    if (
      activeAssistantWorkspace?.id
      && activeAssistantWorkspace.id !== selectedAssistantWorkspaceId
    ) {
      setSelectedAssistantWorkspaceId(activeAssistantWorkspace.id);
    }
  }, [
    activeAssistantWorkspace,
    selectedAssistantWorkspaceId,
    setSelectedAssistantWorkspaceId,
  ]);

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
    <div className="bitfun-my-agent-scene">
      <Suspense fallback={<div className="bitfun-my-agent-scene__loading" />}>
        {activeView === 'profile' && (
          <ProfileScene
            key={resolvedAssistantWorkspace?.id ?? 'default-assistant-workspace'}
            workspacePath={resolvedAssistantWorkspace?.rootPath ?? workspacePath}
          />
        )}
        {activeView === 'agents' && <AgentsScene />}
        {activeView === 'skills' && <SkillsScene />}
        {activeView === 'insights' && <InsightsScene />}
      </Suspense>
    </div>
  );
};

export default MyAgentScene;
