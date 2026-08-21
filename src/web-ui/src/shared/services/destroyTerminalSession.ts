import { getTerminalService } from '@/tools/terminal/services/TerminalService';

/** Destroy a terminal PTY and notify every mounted terminal surface. */
export async function destroyTerminalSession(sessionId: string): Promise<void> {
  const terminalService = getTerminalService();
  await terminalService.connect();

  const sessions = await terminalService.listSessions();
  if (sessions.some((session) => session.id === sessionId)) {
    await terminalService.closeSession(sessionId);
  }

  window.dispatchEvent(new CustomEvent('terminal-session-destroyed', { detail: { sessionId } }));
}
