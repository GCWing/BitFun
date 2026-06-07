import type { SceneTabId } from '@/app/components/SceneBar/types';
import { useSceneStore } from '@/app/stores/sceneStore';
import { useTerminalSceneStore } from '@/app/stores/terminalSceneStore';
import { createTerminalTab } from '@/shared/utils/tabUtils';
import { getCachedTerminalPanelPosition } from '@/tools/terminal';

interface OpenShellSessionTargetOptions {
  sessionId: string;
  sessionName: string;
}

function openStandaloneShellSession(sessionId: string): void {
  const { openScene } = useSceneStore.getState();
  const terminalState = useTerminalSceneStore.getState();

  openScene('shell' as SceneTabId);

  // Force a remount when reopening the same session so the terminal view
  // can recover from stale/error state and always reflect the latest selection.
  if (terminalState.activeSessionId === sessionId) {
    terminalState.setActiveSession(null);
    window.setTimeout(() => {
      useTerminalSceneStore.getState().setActiveSession(sessionId);
    }, 0);
    return;
  }

  terminalState.setActiveSession(sessionId);
}

/**
 * Unified shell open strategy:
 * - stay inside Agent right tabs when the active scene is session
 * - otherwise open the standalone shell scene
 */
export function openShellSessionTarget(options: OpenShellSessionTargetOptions): void {
  const { sessionId, sessionName } = options;
  const { activeTabId } = useSceneStore.getState();

  if (activeTabId === 'session') {
    const targetMode = getCachedTerminalPanelPosition() === 'bottom' ? 'bottom-terminal' : 'agent';
    createTerminalTab(sessionId, sessionName, targetMode);
    return;
  }

  openStandaloneShellSession(sessionId);
}
