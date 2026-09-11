import { getActiveSurfaceScope, getActiveSurfaceId } from '@/infrastructure/peer-device/deviceSurface';
import { getTerminalService } from '@/tools/terminal/services/TerminalService';

/** Destroy a terminal PTY and notify every mounted terminal surface. */
export async function destroyTerminalSession(sessionId: string): Promise<void> {
  const scope = getActiveSurfaceScope();
  const surfaceId = getActiveSurfaceId();
  const terminalService = getTerminalService();
  await terminalService.connect();

  scope.assertCurrent('destroy terminal');
  const sessions = await terminalService.listSessions();
  scope.assertCurrent('destroy terminal');
  if (sessions.some((session) => session.id === sessionId)) {
    await terminalService.closeSession(sessionId);
  }

  window.dispatchEvent(new CustomEvent('terminal-session-destroyed', { detail: { sessionId, surfaceId } }));
}
