/**
 * OverlaySceneRenderer — renders the content for the active overlay scene.
 *
 * Only overlay scenes are rendered here; the base session (SessionScene)
 * is always mounted directly in AgenticOSWorkspace.
 *
 * Each overlay is lazy-loaded to keep the initial bundle small.
 */

import React, { Suspense, lazy } from 'react';
import type { OverlaySceneId } from './types';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { ProcessingIndicator } from '@/flow_chat/components/modern/ProcessingIndicator';

import SettingsScene from '../scenes/settings/SettingsScene';
import AssistantScene from '../scenes/assistant/AssistantScene';

const TerminalScene     = lazy(() => import('../scenes/terminal/TerminalScene'));
const FileViewerScene   = lazy(() => import('../scenes/file-viewer/FileViewerScene'));
const ProfileScene      = lazy(() => import('../scenes/profile/ProfileScene'));
import AppsScene from '../scenes/apps/AppsScene';
const SubagentsScene    = lazy(() => import('../scenes/subagents/SubagentsScene'));
const SkillsScene       = lazy(() => import('../scenes/skills/SkillsScene'));
const ToolsScene        = lazy(() => import('../scenes/tools/ToolsScene'));
const ShellScene        = lazy(() => import('../scenes/shell/ShellScene'));
const LiveAppScene      = lazy(() => import('../scenes/apps/LiveAppScene'));
const PanelViewScene    = lazy(() => import('../scenes/panel-view/PanelViewScene'));
const TaskDetailScene   = lazy(() => import('../scenes/task-detail/TaskDetailScene'));

interface OverlaySceneRendererProps {
  overlayId: OverlaySceneId;
  workspacePath?: string;
}

const OverlaySceneRenderer: React.FC<OverlaySceneRendererProps> = ({
  overlayId,
  workspacePath,
}) => {
  const { t } = useI18n('common');

  return (
    <div className="overlay-scene-renderer">
      <Suspense
        fallback={
          <div
            className="overlay-scene-renderer__fallback"
            role="status"
            aria-busy="true"
            aria-label={t('loading.scenes')}
          >
            <ProcessingIndicator visible />
          </div>
        }
      >
        {renderOverlayScene(overlayId, workspacePath)}
      </Suspense>
    </div>
  );
};

function renderOverlayScene(id: OverlaySceneId, workspacePath?: string): React.ReactNode {
  switch (id) {
    case 'terminal':
      return <TerminalScene isActive />;
    case 'settings':
      return <SettingsScene />;
    case 'file-viewer':
      return <FileViewerScene workspacePath={workspacePath} />;
    case 'profile':
      return <ProfileScene />;
    case 'apps':
      return <AppsScene />;
    case 'subagents':
      return <SubagentsScene />;
    case 'skills':
      return <SkillsScene />;
    case 'tools':
      return <ToolsScene />;
    case 'assistant':
      return <AssistantScene workspacePath={workspacePath} />;
    case 'shell':
      return <ShellScene isActive />;
    case 'panel-view':
      return <PanelViewScene workspacePath={workspacePath} />;
    case 'task-detail':
      return <TaskDetailScene />;
    default:
      if (typeof id === 'string' && id.startsWith('live-app:')) {
        return <LiveAppScene appId={id.slice('live-app:'.length)} />;
      }
      return null;
  }
}

export default OverlaySceneRenderer;
