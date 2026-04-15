import { useOverlayStore } from '@/app/stores/overlayStore';
import { useTerminalSceneStore } from '@/app/stores/terminalSceneStore';
import { createTerminalTab } from '@/shared/utils/tabUtils';

interface OpenShellSessionTargetOptions {
  sessionId: string;
  sessionName: string;
}

function openStandaloneShellSession(sessionId: string): void {
  const { openOverlay } = useOverlayStore.getState();
  const terminalState = useTerminalSceneStore.getState();

  openOverlay('shell');

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
 * - stay inside Agent right tabs when the base session is active (no overlay)
 * - otherwise open the standalone shell overlay
 */
export function openShellSessionTarget(options: OpenShellSessionTargetOptions): void {
  const { sessionId, sessionName } = options;
  const { activeOverlay } = useOverlayStore.getState();

  if (activeOverlay === null) {
    createTerminalTab(sessionId, sessionName, 'agent');
    return;
  }

  openStandaloneShellSession(sessionId);
}
