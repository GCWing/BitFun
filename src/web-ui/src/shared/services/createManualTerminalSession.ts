import { configManager } from '@/infrastructure/config/services/ConfigManager';
import type { TerminalConfig } from '@/infrastructure/config/types';
import { getTerminalService } from '@/tools/terminal/services/TerminalService';
import type { SessionResponse } from '@/tools/terminal/types/session';

export interface CreateManualTerminalSessionOptions {
  workspacePath?: string;
  connectionId?: string | null;
}

async function getDefaultShellType(): Promise<string | undefined> {
  try {
    const config = await configManager.getConfig<TerminalConfig>('terminal');
    return config?.default_shell || undefined;
  } catch {
    return undefined;
  }
}

/** Create a regular user terminal without requiring the Shell navigation UI. */
export async function createManualTerminalSession(
  options: CreateManualTerminalSessionOptions,
): Promise<SessionResponse> {
  const service = getTerminalService();
  await service.connect();

  const sessions = await service.listSessions();
  const manualCount = sessions.filter((session) => session.source === 'manual').length;
  const shellType = await getDefaultShellType();

  return service.createSession({
    workingDirectory: options.workspacePath,
    connectionId: options.connectionId ?? undefined,
    name: `Shell ${manualCount + 1}`,
    shellType,
    source: 'manual',
  });
}
