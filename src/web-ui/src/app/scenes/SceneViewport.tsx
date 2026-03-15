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
import './SceneViewport.scss';

const SessionScene    = lazy(() => import('./session/SessionScene'));
const TerminalScene   = lazy(() => import('./terminal/TerminalScene'));
const GitScene        = lazy(() => import('./git/GitScene'));
const SettingsScene   = lazy(() => import('./settings/SettingsScene'));
const FileViewerScene = lazy(() => import('./file-viewer/FileViewerScene'));
const ProfileScene    = lazy(() => import('./profile/ProfileScene'));
const AgentsScene       = lazy(() => import('./agents/AgentsScene'));
const SkillsScene     = lazy(() => import('./skills/SkillsScene'));
const MiniAppGalleryScene = lazy(() => import('./miniapps/MiniAppGalleryScene'));
const BrowserScene    = lazy(() => import('./browser/BrowserScene'));
const MyAgentScene    = lazy(() => import('./my-agent/MyAgentScene'));
const ShellScene      = lazy(() => import('./shell/ShellScene'));
const WelcomeScene    = lazy(() => import('./welcome/WelcomeScene'));
const MiniAppScene    = lazy(() => import('./miniapps/MiniAppScene'));

interface SceneViewportProps {
  workspacePath?: string;
  isEntering?: boolean;
}

const SceneViewport: React.FC<SceneViewportProps> = ({ workspacePath, isEntering = false }) => {
  const { openTabs, activeTabId } = useSceneManager();
  const { t } = useI18n('common');

  // All tabs closed — show empty state
  if (openTabs.length === 0) {
    return (
      <div className="bitfun-scene-viewport bitfun-scene-viewport--empty">
        <p className="bitfun-scene-viewport__empty-hint">{t('welcomeScene.emptyHint')}</p>
      </div>
    );
  }

  return (
    <div className="bitfun-scene-viewport">
      <Suspense fallback={null}>
        {openTabs.map(tab => (
          <div
            key={tab.id}
            className={[
              'bitfun-scene-viewport__scene',
              tab.id === activeTabId && 'bitfun-scene-viewport__scene--active',
            ].filter(Boolean).join(' ')}
            aria-hidden={tab.id !== activeTabId}
          >
            {renderScene(tab.id, workspacePath, isEntering)}
          </div>
        ))}
      </Suspense>
    </div>
  );
};

function renderScene(id: SceneTabId, workspacePath?: string, isEntering?: boolean) {
  switch (id) {
    case 'welcome':
      return <WelcomeScene />;
    case 'session':
      return <SessionScene workspacePath={workspacePath} isEntering={isEntering} />;
    case 'terminal':
      return <TerminalScene />;
    case 'git':
      return <GitScene workspacePath={workspacePath} />;
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
    case 'my-agent':
      return <MyAgentScene workspacePath={workspacePath} />;
    case 'shell':
      return <ShellScene />;
    default:
      if (typeof id === 'string' && id.startsWith('miniapp:')) {
        return <MiniAppScene appId={id.slice('miniapp:'.length)} />;
      }
      return null;
  }
}

export default SceneViewport;
