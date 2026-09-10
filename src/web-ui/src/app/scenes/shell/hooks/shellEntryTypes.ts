import type { ManualTerminalProfile } from '@/tools/terminal/services/manualTerminalProfileService';
import type { SessionResponse, TerminalSessionSource } from '@/tools/terminal/types/session';

export const MANUAL_SOURCE: TerminalSessionSource = 'manual';
export const AGENT_SOURCE: TerminalSessionSource = 'agent';

export type ShellEntryKind = 'manual-profile' | 'manual-session' | 'agent-session';

export interface ShellEntry {
  id: string;
  kind: ShellEntryKind;
  source: TerminalSessionSource;
  sessionId: string;
  name: string;
  isRunning: boolean;
  isPersisted: boolean;
  status?: string;
  profileId?: string;
  cwd?: string;
  workingDirectory?: string;
  startupCommand?: string;
  shellType?: string;
}

export interface SaveShellEntryInput {
  name: string;
  workingDirectory?: string;
  startupCommand?: string;
}

export function isSessionRunning(session: SessionResponse): boolean {
  const normalizedStatus = String(session.status).toLowerCase();
  return ['active', 'running', 'ready', 'orphaned', 'starting', 'restoring'].includes(normalizedStatus);
}

export function getShellEntryState(entry: ShellEntry): 'running' | 'starting' | 'stopping' | 'saved' | 'exited' | 'error' | 'unknown' {
  const status = entry.status?.toLowerCase() ?? '';
  if (status === 'saved') return 'saved';
  if (status === 'starting' || status === 'restoring') return 'starting';
  if (status === 'terminating') return 'stopping';
  if (status === 'error') return 'error';
  if (status === 'stopped' || status.startsWith('exited')) return 'exited';
  return entry.isRunning ? 'running' : 'unknown';
}

export function compareShellEntries(a: ShellEntry, b: ShellEntry): number {
  if (a.isPersisted !== b.isPersisted) {
    return a.isPersisted ? -1 : 1;
  }

  if (a.isRunning !== b.isRunning) {
    return a.isRunning ? -1 : 1;
  }

  return a.name.localeCompare(b.name, undefined, { sensitivity: 'base' });
}

export function createManualProfileEntry(
  profile: ManualTerminalProfile,
  session?: SessionResponse,
): ShellEntry {
  return {
    id: profile.id,
    kind: 'manual-profile',
    source: session?.source ?? MANUAL_SOURCE,
    sessionId: profile.sessionId,
    name: profile.name,
    isRunning: session ? isSessionRunning(session) : false,
    isPersisted: true,
    status: session?.status ?? 'saved',
    profileId: profile.id,
    cwd: session?.cwd,
    workingDirectory: profile.workingDirectory,
    startupCommand: profile.startupCommand,
    shellType: session?.shellType ?? profile.shellType,
  };
}

export function createSessionEntry(
  session: SessionResponse,
  kind: 'manual-session' | 'agent-session',
): ShellEntry {
  return {
    id: session.id,
    kind,
    source: kind === 'agent-session' ? AGENT_SOURCE : MANUAL_SOURCE,
    sessionId: session.id,
    name: session.name,
    isRunning: isSessionRunning(session),
    isPersisted: false,
    status: session.status,
    cwd: session.cwd,
    workingDirectory: session.cwd,
    shellType: session.shellType,
  };
}
