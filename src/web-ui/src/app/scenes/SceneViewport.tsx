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
import { MessageSquare, Terminal, GitBranch, Settings, FileCode2, CircleUserRound, Blocks, Puzzle } from 'lucide-react';
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
const CapabilitiesScene = lazy(() => import('./capabilities/CapabilitiesScene'));
const TeamScene       = lazy(() => import('./team/TeamScene'));
const SkillsScene     = lazy(() => import('./skills/SkillsScene'));
const WelcomeScene    = lazy(() => import('./welcome/WelcomeScene'));

interface SceneViewportProps {
  workspacePath?: string;
  isEntering?: boolean;
}

const SceneViewport: React.FC<SceneViewportProps> = ({ workspacePath, isEntering = false }) => {
  const { openTabs, activeTabId, openScene } = useSceneManager();
  const { t } = useI18n('common');

  // All tabs closed — show quick-launch empty state
  if (openTabs.length === 0) {
    return (
      <div className="bitfun-scene-viewport bitfun-scene-viewport--empty">
        <div className="bitfun-scene-viewport__empty-state">
          <p className="bitfun-scene-viewport__empty-hint">{t('welcomeScene.emptyHint')}</p>
          <div className="bitfun-scene-viewport__empty-actions">
            {[
              { id: 'session'      as SceneTabId, Icon: MessageSquare, labelKey: 'scenes.aiAgent'       },
              { id: 'terminal'     as SceneTabId, Icon: Terminal,      labelKey: 'scenes.terminal'      },
              { id: 'git'          as SceneTabId, Icon: GitBranch,     labelKey: 'scenes.git'           },
              { id: 'settings'     as SceneTabId, Icon: Settings,      labelKey: 'scenes.settings'      },
              { id: 'file-viewer'  as SceneTabId, Icon: FileCode2,     labelKey: 'scenes.fileViewer'    },
              { id: 'profile'      as SceneTabId, Icon: CircleUserRound, labelKey: 'scenes.projectContext' },
              { id: 'capabilities' as SceneTabId, Icon: Blocks,        labelKey: 'scenes.capabilities'  },
              { id: 'skills'       as SceneTabId, Icon: Puzzle,        labelKey: 'scenes.skills'        },
            ].map(({ id, Icon, labelKey }) => {
              const label = t(labelKey);
              return (
                <button
                  key={id}
                  className="bitfun-scene-viewport__empty-btn"
                  onClick={() => openScene(id)}
                >
                  <Icon size={16} />
                  <span>{label}</span>
                </button>
              );
            })}
          </div>
        </div>
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
      return <TerminalScene workspacePath={workspacePath} />;
    case 'git':
      return <GitScene workspacePath={workspacePath} />;
    case 'settings':
      return <SettingsScene />;
    case 'file-viewer':
      return <FileViewerScene workspacePath={workspacePath} />;
    case 'profile':
      return <ProfileScene workspacePath={workspacePath} />;
    case 'capabilities':
      return <CapabilitiesScene />;
    case 'team':
      return <TeamScene />;
    case 'skills':
      return <SkillsScene />;
    default:
      return null;
  }
}

export default SceneViewport;
