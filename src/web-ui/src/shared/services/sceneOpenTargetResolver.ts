import type { OverlaySceneId } from '@/app/overlay/types';
import { useOverlayStore } from '@/app/stores/overlayStore';

export type OpenIntent = 'file' | 'terminal';
export type OpenTargetMode = 'agent' | 'project';
export type OpenSource = 'default' | 'project-nav';

export interface OpenTargetResolution {
  mode: OpenTargetMode;
  targetSceneId: 'session' | OverlaySceneId;
  /**
   * True when the overlay was not active at the time of the call,
   * meaning the scene will be freshly mounted by React.
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
  const { activeOverlay } = useOverlayStore.getState();
  const source = context.source ?? 'default';

  // Base session active: stay in Agentic OS AuxPane tabs
  if (activeOverlay === null) {
    return { mode: 'agent', targetSceneId: 'session', sceneJustOpened: false };
  }

  // Project navigation file tree opens files in file-viewer overlay
  if (intent === 'file' && source === 'project-nav') {
    return { mode: 'project', targetSceneId: 'file-viewer', sceneJustOpened: false };
  }

  // Non-agent scenes route to their dedicated overlay scenes
  if (intent === 'terminal') {
    return { mode: 'project', targetSceneId: 'shell', sceneJustOpened: false };
  }

  return { mode: 'project', targetSceneId: 'file-viewer', sceneJustOpened: false };
}

/**
 * Resolve and focus the host scene for an intent.
 *
 * Returns `sceneJustOpened: true` when the target overlay was not active
 * and will therefore be freshly mounted. In that case callers should route
 * follow-up tab events through the pending-tab queue.
 */
export function resolveAndFocusOpenTarget(
  intent: OpenIntent,
  context: OpenTargetContext = {}
): OpenTargetResolution {
  const { activeOverlay, openOverlay } = useOverlayStore.getState();
  const resolution = resolveOpenTarget(intent, context);

  const sceneJustOpened =
    resolution.targetSceneId !== 'session' &&
    activeOverlay !== resolution.targetSceneId;

  if (resolution.targetSceneId !== 'session') {
    openOverlay(resolution.targetSceneId as OverlaySceneId);
  }
  return { ...resolution, sceneJustOpened };
}
