import { configManager } from '@/infrastructure/config/services/ConfigManager';
import type { TerminalConfig } from '@/infrastructure/config/types';
import { getTerminalService } from '@/tools/terminal/services/TerminalService';
import type { CreateSessionRequest, SessionResponse } from '@/tools/terminal/types/session';
import { getActiveSurfaceScope } from '@/infrastructure/peer-device/deviceSurface';

export interface CreateManualTerminalSessionOptions {
  workspacePath?: string;
  connectionId?: string | null;
  shellType?: string;
  shellId?: string;
  name?: string;
  sessionId?: string;
}

async function getDefaultShellPreference(): Promise<string | undefined> {
  try {
    const config = await configManager.getConfig<TerminalConfig>('terminal');
    return config?.default_shell || undefined;
  } catch {
    return undefined;
  }
}

async function resolveDefaultShellSelection(
  service: ReturnType<typeof getTerminalService>,
): Promise<Pick<CreateSessionRequest, 'shellId' | 'shellType'>> {
  const preference = await getDefaultShellPreference();
  if (!preference) {
    return {};
  }

  try {
    const shell = (await service.getAvailableShells()).find(
      (candidate) => candidate.path === preference,
    );
    return shell ? { shellId: shell.id, shellType: shell.shellType } : {};
  } catch {
    return {};
  }
}

/** Create a regular user terminal without requiring the Shell navigation UI. */
export async function createManualTerminalSession(
  options: CreateManualTerminalSessionOptions,
): Promise<SessionResponse> {
  const service = getTerminalService();
  const scope = getActiveSurfaceScope();
  await service.connect();
  scope.assertCurrent('create manual terminal');

  const [sessions, shellSelection] = await Promise.all([
    service.listSessions(),
    options.connectionId ? Promise.resolve({}) : resolveDefaultShellSelection(service),
  ]);
  scope.assertCurrent('create manual terminal');
  const manualCount = sessions.filter((session) => session.source === 'manual').length;

  return service.createSession({
    workingDirectory: options.workspacePath,
    connectionId: options.connectionId ?? undefined,
    name: options.name ?? `Shell ${manualCount + 1}`,
    sessionId: options.sessionId,
    ...(options.shellId ? { shellId: options.shellId, shellType: options.shellType }
      : options.shellType && options.shellType !== 'Remote' ? { shellType: options.shellType } : shellSelection),
    source: 'manual',
  });
}
