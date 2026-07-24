/**
 * SceneViewport — renders the active scene component.
 *
 * All tabs are mounted but only the active one is visible,
 * preserving state across tab switches.
 *
 * 'welcome' is a proper scene tab; it auto-closes when any other
 * scene is explicitly opened.
 */

import React, {
  Suspense,
  lazy,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from 'react';
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
const PagesScene      = lazy(() => import('./pages/PagesScene'));
const BrowserScene    = lazy(() => import('./browser/BrowserScene'));
const InsightsScene   = lazy(() => import('./my-agent/InsightsScene'));
const ShellScene      = lazy(() => import('./shell/ShellScene'));
const WelcomeScene    = lazy(() => import('./welcome/WelcomeScene'));
const MiniAppScene    = lazy(() => import('./miniapps/MiniAppScene'));
const PanelViewScene  = lazy(() => import('./panel-view/PanelViewScene'));

// Keep in sync with bitfun-motion-view-exit in app/styles/motion.scss.
const SCENE_EXIT_DURATION_MS = 140;

interface SceneTransition {
  outgoingTabId: SceneTabId;
  incomingTabId: SceneTabId;
  phase: 'holding' | 'exiting';
}

interface SceneReadyBoundaryProps {
  sceneId: SceneTabId;
  onReady: (sceneId: SceneTabId) => void;
  children: React.ReactNode;
}

/**
 * This effect commits only after a lazy scene has resolved through Suspense.
 * It lets the viewport hold the outgoing pixels until the incoming tree is
 * actually paintable instead of exposing a fallback between the two scenes.
 */
const SceneReadyBoundary: React.FC<SceneReadyBoundaryProps> = ({
  sceneId,
  onReady,
  children,
}) => {
  useLayoutEffect(() => {
    onReady(sceneId);
  }, [onReady, sceneId]);

  return <>{children}</>;
};

interface SceneViewportProps {
  workspacePath?: string;
  isEntering?: boolean;
}

const SceneViewport: React.FC<SceneViewportProps> = ({ workspacePath, isEntering = false }) => {
  const { openTabs, activeTabId } = useSceneManager();
  const { t } = useI18n('common');
  const [transition, setTransition] = useState<SceneTransition | null>(null);
  const [readyVersion, setReadyVersion] = useState(0);
  const readySceneIdsRef = useRef<Set<SceneTabId>>(new Set());
  const previousActiveTabIdRef = useRef<SceneTabId>(activeTabId);
  useDialogCompletionNotify();

  const markSceneReady = useCallback((sceneId: SceneTabId) => {
    if (readySceneIdsRef.current.has(sceneId)) return;
    readySceneIdsRef.current.add(sceneId);
    setReadyVersion(version => version + 1);
  }, []);

  // Derive the outgoing id during render as well as from state. This keeps a
  // just-closed active tab (notably the welcome tab) in the keyed React tree
  // for its exit frame instead of unmounting and remounting it after layout.
  const outgoingTabId = previousActiveTabIdRef.current !== activeTabId
    ? previousActiveTabIdRef.current
    : transition?.outgoingTabId ?? null;
  const renderedTabIds = openTabs.map(tab => tab.id);
  if (outgoingTabId && !renderedTabIds.includes(outgoingTabId)) {
    renderedTabIds.push(outgoingTabId);
  }

  useLayoutEffect(() => {
    const previousActiveTabId = previousActiveTabIdRef.current;
    previousActiveTabIdRef.current = activeTabId;

    if (!previousActiveTabId || previousActiveTabId === activeTabId) {
      return;
    }

    setTransition({
      outgoingTabId: previousActiveTabId,
      incomingTabId: activeTabId,
      phase: readySceneIdsRef.current.has(activeTabId) ? 'exiting' : 'holding',
    });
  }, [activeTabId]);

  useLayoutEffect(() => {
    if (
      transition?.phase !== 'holding'
      || !readySceneIdsRef.current.has(transition.incomingTabId)
    ) {
      return;
    }

    setTransition(current => (
      current?.incomingTabId === transition.incomingTabId
        ? { ...current, phase: 'exiting' }
        : current
    ));
  }, [readyVersion, transition]);

  useEffect(() => {
    if (transition?.phase !== 'exiting') return;

    const completedTransition = transition;
    const exitTimer = window.setTimeout(() => {
      setTransition(current => (
        current === completedTransition ? null : current
      ));
    }, SCENE_EXIT_DURATION_MS);

    return () => window.clearTimeout(exitTimer);
  }, [transition]);

  // All tabs closed — show empty state
  if (openTabs.length === 0) {
    return (
      <div className="bitfun-scene-viewport" data-testid="scene-viewport">
        <div
          className="bitfun-scene-viewport__clip bitfun-scene-viewport__clip--empty"
          data-testid="scene-viewport-empty"
        >
          <p className="bitfun-scene-viewport__empty-hint">{t('welcomeScene.emptyHint')}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="bitfun-scene-viewport" data-testid="scene-viewport">
      <div className="bitfun-scene-viewport__clip" data-testid="scene-viewport-clip">
        {renderedTabIds.map(tabId => {
          const isActive = tabId === activeTabId;
          const isOutgoing = !isActive && tabId === outgoingTabId;
          const isExiting = isOutgoing && transition?.phase === 'exiting';
          return (
            <div
              key={tabId}
              className={[
                'bitfun-scene-viewport__scene',
                isActive && 'bitfun-scene-viewport__scene--active',
                isOutgoing && 'bitfun-scene-viewport__scene--outgoing',
                isExiting && 'bitfun-scene-viewport__scene--exiting',
              ].filter(Boolean).join(' ')}
              aria-hidden={!isActive}
              data-testid="scene-viewport-scene"
              data-scene-id={tabId}
              data-scene-active={isActive ? 'true' : 'false'}
              data-scene-transition={isExiting ? 'exit' : undefined}
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
                <SceneReadyBoundary sceneId={tabId} onReady={markSceneReady}>
                  {renderScene(tabId, workspacePath, isEntering, isActive)}
                </SceneReadyBoundary>
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
    case 'pages':
      return <PagesScene isActive={isActive} />;
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
