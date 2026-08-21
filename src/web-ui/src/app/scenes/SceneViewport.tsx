/**
 * SceneViewport — renders the active scene component.
 *
 * All visible tabs and retained background scenes are mounted, but only the
 * active tab is visible, preserving state across tab switches and auto-eviction.
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
import { DotMatrixLoader } from '@/component-library';
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
const EcosystemCompatibilityScene = lazy(
  () => import('./ecosystem-compatibility/EcosystemCompatibilityScene'),
);
const MiniAppGalleryScene = lazy(() => import('./miniapps/MiniAppGalleryScene'));
const PagesScene      = lazy(() => import('./pages/PagesScene'));
const BrowserScene    = lazy(() => import('./browser/BrowserScene'));
const TodosScene      = lazy(() => import('./todos/TodosScene'));
const InsightsScene   = lazy(() => import('./my-agent/InsightsScene'));
const ShellScene      = lazy(() => import('./shell/ShellScene'));
const WelcomeScene    = lazy(() => import('./welcome/WelcomeScene'));
const MiniAppScene    = lazy(() => import('./miniapps/MiniAppScene'));
const PanelViewScene  = lazy(() => import('./panel-view/PanelViewScene'));

const SCENE_ENTRY_DURATION_MS = 480;
const EMPTY_SCENE_ID = '__empty-scene__' as const;
type RenderedSceneId = SceneTabId | typeof EMPTY_SCENE_ID;

interface SceneTransition {
  outgoingTabId: RenderedSceneId;
  incomingTabId: RenderedSceneId;
  phase: 'holding' | 'preparing' | 'running';
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
  const {
    openTabs,
    retainedScenes,
    activeTabId,
    navigationMotion,
    navigationSequence,
  } = useSceneManager();
  const { t } = useI18n('common');
  const activeRenderedSceneId: RenderedSceneId = openTabs.length === 0
    ? EMPTY_SCENE_ID
    : activeTabId;
  const [transition, setTransition] = useState<SceneTransition | null>(null);
  const [readyVersion, setReadyVersion] = useState(0);
  const readySceneIdsRef = useRef<Set<RenderedSceneId>>(new Set([EMPTY_SCENE_ID]));
  const previousActiveTabIdRef = useRef<RenderedSceneId>(activeRenderedSceneId);
  useDialogCompletionNotify();

  const markSceneReady = useCallback((sceneId: SceneTabId) => {
    if (readySceneIdsRef.current.has(sceneId)) return;
    readySceneIdsRef.current.add(sceneId);
    setReadyVersion(version => version + 1);
  }, []);

  // Derive the outgoing id during render as well as from state. Pointer
  // navigation keeps that scene as the only visible surface until the target
  // has resolved through Suspense, then swaps atomically to the incoming scene.
  const activeSceneChanged = previousActiveTabIdRef.current !== activeRenderedSceneId;
  const pendingTransition: SceneTransition | null = activeSceneChanged
    && navigationMotion === 'pointer'
    ? {
        outgoingTabId: previousActiveTabIdRef.current,
        incomingTabId: activeRenderedSceneId,
        phase: 'holding',
      }
    : activeSceneChanged
      ? null
      : transition;
  const outgoingTabId = pendingTransition?.outgoingTabId ?? null;
  const renderedTabIds: RenderedSceneId[] = openTabs.length === 0
    ? [EMPTY_SCENE_ID]
    : openTabs.map(tab => tab.id);
  for (const scene of retainedScenes) {
    if (!renderedTabIds.includes(scene.id)) {
      renderedTabIds.push(scene.id);
    }
  }
  if (outgoingTabId && !renderedTabIds.includes(outgoingTabId)) {
    renderedTabIds.push(outgoingTabId);
  }

  useLayoutEffect(() => {
    const previousActiveTabId = previousActiveTabIdRef.current;
    previousActiveTabIdRef.current = activeRenderedSceneId;

    if (previousActiveTabId === activeRenderedSceneId) {
      return;
    }

    if (navigationMotion !== 'pointer') {
      setTransition(null);
      return;
    }

    setTransition({
      outgoingTabId: previousActiveTabId,
      incomingTabId: activeRenderedSceneId,
      phase: readySceneIdsRef.current.has(activeRenderedSceneId) ? 'preparing' : 'holding',
    });
  }, [activeRenderedSceneId, navigationMotion, navigationSequence]);

  useLayoutEffect(() => {
    if (
      transition?.phase !== 'holding'
      || !readySceneIdsRef.current.has(transition.incomingTabId)
    ) {
      return;
    }

    setTransition(current => (
      current?.incomingTabId === transition.incomingTabId
        ? { ...current, phase: 'preparing' }
        : current
    ));
  }, [readyVersion, transition]);

  useEffect(() => {
    if (transition?.phase !== 'preparing') return;

    const preparedTransition = transition;
    let runningFrame: number | null = null;
    const preparationFrame = window.requestAnimationFrame(() => {
      runningFrame = window.requestAnimationFrame(() => {
        setTransition(current => (
          current === preparedTransition
            ? { ...current, phase: 'running' }
            : current
        ));
      });
    });

    return () => {
      window.cancelAnimationFrame(preparationFrame);
      if (runningFrame !== null) {
        window.cancelAnimationFrame(runningFrame);
      }
    };
  }, [transition]);

  useEffect(() => {
    if (transition?.phase !== 'running') return;

    const completedTransition = transition;
    const entryTimer = window.setTimeout(() => {
      setTransition(current => (
        current === completedTransition ? null : current
      ));
    }, SCENE_ENTRY_DURATION_MS);

    return () => window.clearTimeout(entryTimer);
  }, [transition]);

  return (
    <div
      className="bitfun-scene-viewport"
      data-testid="scene-viewport"
      data-bf-scene="workbench"
      data-bf-part="viewport"
      data-bf-state={activeRenderedSceneId === EMPTY_SCENE_ID ? 'empty' : undefined}
    >
      <div
        className="bitfun-scene-viewport__clip"
        data-testid="scene-viewport-clip"
        data-scene-motion-phase={pendingTransition?.phase}
        data-bf-scene="workbench"
        data-bf-part="viewportClip"
      >
        {renderedTabIds.map(tabId => {
          const isEmpty = tabId === EMPTY_SCENE_ID;
          const isActive = tabId === activeRenderedSceneId;
          const isOutgoing = !isActive && tabId === outgoingTabId;
          const isIncoming = isActive && pendingTransition?.incomingTabId === tabId;
          const isVisible = pendingTransition?.phase === 'holding'
            ? isOutgoing
            : isActive;
          return (
            <div
              key={tabId}
              className={[
                'bitfun-scene-viewport__scene',
                isEmpty && 'bitfun-scene-viewport__scene--empty',
                isActive && 'bitfun-scene-viewport__scene--active',
                isVisible && 'bitfun-scene-viewport__scene--visible',
                isIncoming && 'bitfun-scene-viewport__scene--incoming',
                isOutgoing && 'bitfun-scene-viewport__scene--outgoing',
              ].filter(Boolean).join(' ')}
              aria-hidden={!isActive || !isVisible}
              {...(!isActive || !isVisible ? { inert: '' } : {})}
              data-testid="scene-viewport-scene"
              data-scene-id={tabId}
              data-scene-active={isActive ? 'true' : 'false'}
              data-bf-scene="workbench"
              data-bf-part="scene"
              data-bf-scene-id={isEmpty ? undefined : tabId}
              data-bf-state={[
                isActive && 'active',
                isEmpty && 'empty',
              ].filter(Boolean).join(' ') || undefined}
            >
              {isEmpty ? (
                <div
                  className="bitfun-scene-viewport__empty"
                  data-testid="scene-viewport-empty"
                  data-bf-scene="workbench"
                  data-bf-part="empty"
                  data-bf-state="empty"
                >
                  <p className="bitfun-scene-viewport__empty-hint">{t('welcomeScene.emptyHint')}</p>
                </div>
              ) : (
                <Suspense
                  fallback={
                    isActive ? (
                      <div
                        className="bitfun-scene-viewport__lazy-fallback"
                        role="status"
                        aria-busy="true"
                        aria-label={t('loading.scenes')}
                        data-bf-scene="workbench"
                        data-bf-part="loading"
                      >
                        <DotMatrixLoader size="medium" />
                      </div>
                    ) : null
                  }
                >
                  <SceneReadyBoundary sceneId={tabId} onReady={markSceneReady}>
                    {renderScene(tabId, workspacePath, isEntering, isActive)}
                  </SceneReadyBoundary>
                </Suspense>
              )}
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
    case 'ecosystem-compatibility':
      return <EcosystemCompatibilityScene />;
    case 'miniapps':
      return <MiniAppGalleryScene />;
    case 'pages':
      return <PagesScene isActive={isActive} />;
    case 'browser':
      return <BrowserScene />;
    case 'assistant':
      return <AssistantScene workspacePath={workspacePath} />;
    case 'todos':
      return <TodosScene />;
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
