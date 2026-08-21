import type { SceneTabId } from '@/app/components/SceneBar/types';
import { useSceneStore } from '@/app/stores/sceneStore';

export type OpenIntent = 'file' | 'terminal';
export type OpenTargetMode = 'agent' | 'project';
export type OpenSource = 'default' | 'project-nav';

export interface OpenTargetResolution {
  mode: OpenTargetMode;
  targetSceneId: SceneTabId;
  /**
   * True when the target scene was not in openTabs at the time of the call,
   * meaning it will be freshly mounted by React. Consumers should use the
   * pending-tab queue rather than dispatching events directly, to avoid
   * race conditions where the event fires before the listener is registered.
   */
  sceneJustOpened: boolean;
}

export interface OpenTargetContext {
  source?: OpenSource;
}

/**
 * Resolve where a content-open intent should land.
 * This is the shared policy entry for cross-scene collaboration.
 */
export function resolveOpenTarget(intent: OpenIntent, context: OpenTargetContext = {}): OpenTargetResolution {
  const { activeTabId } = useSceneStore.getState();
  const source = context.source ?? 'default';

  // Active-scene-first: if user is currently in Session scene, stay in agent AuxPane tabs.
  if (activeTabId === 'session') {
    return { mode: 'agent', targetSceneId: 'session', sceneJustOpened: false };
  }

  // Project navigation file tree opens files in file-viewer scene
  // when user is not currently working in Session scene.
  if (intent === 'file' && source === 'project-nav') {
    return { mode: 'project', targetSceneId: 'file-viewer', sceneJustOpened: false };
  }

  // Non-agent scenes route to their dedicated host scenes.
  if (intent === 'terminal') {
    return { mode: 'project', targetSceneId: 'terminal', sceneJustOpened: false };
  }

  return { mode: 'project', targetSceneId: 'file-viewer', sceneJustOpened: false };
}

/**
 * Resolve and focus the host scene for an intent.
 *
 * Returns `sceneJustOpened: true` when the target scene was not yet in
 * openTabs and is therefore being freshly mounted by React.  In that case
 * callers should route the follow-up tab event through the pending-tab queue
 * (pendingTabQueue) instead of dispatching directly, to avoid losing the
 * event before the scene's ContentCanvas listener is registered.
 */
export function resolveAndFocusOpenTarget(
  intent: OpenIntent,
  context: OpenTargetContext = {}
): OpenTargetResolution {
  const { openScene, openTabs, activeTabId } = useSceneStore.getState();
  const resolution = resolveOpenTarget(intent, context);

  // Scene is freshly added when it is neither already active nor already open.
  const sceneJustOpened =
    resolution.targetSceneId !== activeTabId &&
    !openTabs.some(t => t.id === resolution.targetSceneId);

  openScene(resolution.targetSceneId);
  return { ...resolution, sceneJustOpened };
}
