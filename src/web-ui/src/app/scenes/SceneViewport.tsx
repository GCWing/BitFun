/**
 * SceneViewport — renders the active scene component.
 *
 * All tabs are mounted but only the active one is visible,
 * preserving state across tab switches.
 *
 * 'welcome' is a proper scene tab; it auto-closes when any other
 * scene is explicitly opened.
 */

import React, { Suspense, lazy } from 'react';
import type { SceneTabId } from '../components/SceneBar/types';
import { useSceneManager } from '../hooks/useSceneManager';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { useDialogCompletionNotify } from '../hooks/useDialogCompletionNotify';
import { ProcessingIndicator } from '@/flow_chat/components/modern/ProcessingIndicator';
import SettingsScene from './settings/SettingsScene';
import AssistantScene from './assistant/AssistantScene';
import SessionScene from './session/SessionScene';
import './SceneViewport.scss';

// Session is the primary interaction path. Keep it in the main scene bundle so
// first open does not stall on a lazy chunk fetch/parse before FlowChat mounts.
const TerminalScene   = lazy(() => import('./terminal/TerminalScene'));
const GitScene        = lazy(() => import('./git/GitScene'));
const FileViewerScene = lazy(() => import('./file-viewer/FileViewerScene'));
const ProfileScene    = lazy(() => import('./profile/ProfileScene'));
const AgentsScene       = lazy(() => import('./agents/AgentsScene'));
const SkillsScene     = lazy(() => import('./skills/SkillsScene'));
const MiniAppGalleryScene = lazy(() => import('./miniapps/MiniAppGalleryScene'));
const BrowserScene    = lazy(() => import('./browser/BrowserScene'));
const InsightsScene   = lazy(() => import('./my-agent/InsightsScene'));
const ShellScene      = lazy(() => import('./shell/ShellScene'));
const WelcomeScene    = lazy(() => import('./welcome/WelcomeScene'));
const MiniAppScene    = lazy(() => import('./miniapps/MiniAppScene'));
const PanelViewScene  = lazy(() => import('./panel-view/PanelViewScene'));


interface SceneViewportProps {
  workspacePath?: string;
  isEntering?: boolean;
}

const SceneViewport: React.FC<SceneViewportProps> = ({ workspacePath, isEntering = false }) => {
  const { openTabs, activeTabId } = useSceneManager();
  const { t } = useI18n('common');
  useDialogCompletionNotify();

  // All tabs closed — show empty state
  if (openTabs.length === 0) {
    return (
      <div className="bitfun-scene-viewport">
        <div className="bitfun-scene-viewport__clip bitfun-scene-viewport__clip--empty">
          <p className="bitfun-scene-viewport__empty-hint">{t('welcomeScene.emptyHint')}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="bitfun-scene-viewport">
      <div className="bitfun-scene-viewport__clip">
        {openTabs.map(tab => {
          const isActive = tab.id === activeTabId;
          return (
            <div
              key={tab.id}
              className={[
                'bitfun-scene-viewport__scene',
                isActive && 'bitfun-scene-viewport__scene--active',
              ].filter(Boolean).join(' ')}
              aria-hidden={!isActive}
            >
              <Suspense
                fallback={
                  isActive ? (
                    <div
                      className="bitfun-scene-viewport__lazy-fallback"
                      role="status"
                      aria-busy="true"
                      aria-label={t('loading.scenes')}
                    >
                      <ProcessingIndicator visible />
                    </div>
                  ) : null
                }
              >
                {renderScene(tab.id, workspacePath, isEntering, isActive)}
              </Suspense>
            </div>
          );
        })}
      </div>
    </div>
  );
};

function renderScene(
  id: SceneTabId,
  workspacePath?: string,
  isEntering?: boolean,
  isActive: boolean = false
) {
  switch (id) {
    case 'welcome':
      return <WelcomeScene />;
    case 'session':
      return <SessionScene workspacePath={workspacePath} isEntering={isEntering} isActive={isActive} />;
    case 'terminal':
      return <TerminalScene isActive={isActive} />;
    case 'git':
      return <GitScene workspacePath={workspacePath} isActive={isActive} />;
    case 'settings':
      return <SettingsScene />;
    case 'file-viewer':
      return <FileViewerScene workspacePath={workspacePath} />;
    case 'profile':
      return <ProfileScene />;
    case 'agents':
      return <AgentsScene />;
    case 'skills':
      return <SkillsScene />;
    case 'miniapps':
      return <MiniAppGalleryScene />;
    case 'browser':
      return <BrowserScene />;
    case 'assistant':
      return <AssistantScene workspacePath={workspacePath} />;
    case 'insights':
      return <InsightsScene />;
    case 'shell':
      return <ShellScene isActive={isActive} />;
    case 'panel-view':
      return <PanelViewScene workspacePath={workspacePath} />;
    default:
      if (typeof id === 'string' && id.startsWith('miniapp:')) {
        return <MiniAppScene appId={id.slice('miniapp:'.length)} />;
      }
      return null;
  }
}

export default SceneViewport;
