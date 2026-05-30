import { describe, expect, it } from 'vitest';

import type { AcpAvailableCommand } from '@/infrastructure/api/service-api/ACPClientAPI';
import { filterSlashCommands } from './useAcpSlashCommands';
import { acpSessionRef, acpSlashCommandText } from '../utils/acpSession';

const commands: AcpAvailableCommand[] = [
  { name: 'compact', description: 'Compact the conversation context' },
  { name: 'init', description: 'Initialize the project' },
  { name: 'create_plan', description: 'Draft an execution plan', inputHint: 'what to plan' },
];

describe('filterSlashCommands', () => {
  it('returns all commands for an empty query', () => {
    expect(filterSlashCommands(commands, '')).toHaveLength(3);
    expect(filterSlashCommands(commands, '   ')).toHaveLength(3);
  });

  it('tolerates a leading slash', () => {
    expect(filterSlashCommands(commands, '/comp').map((c) => c.name)).toEqual(['compact']);
  });

  it('matches on name (case-insensitive)', () => {
    expect(filterSlashCommands(commands, 'INIT').map((c) => c.name)).toEqual(['init']);
  });

  it('matches on description', () => {
    expect(filterSlashCommands(commands, 'plan').map((c) => c.name)).toEqual(['create_plan']);
  });

  it('returns empty when nothing matches', () => {
    expect(filterSlashCommands(commands, 'zzz')).toEqual([]);
  });
});

describe('acpSlashCommandText', () => {
  it('formats a command name as invokable prompt text', () => {
    expect(acpSlashCommandText('create_plan')).toBe('/create_plan ');
  });
});

describe('acpSessionRef', () => {
  it('returns null for a non-ACP session', () => {
    expect(acpSessionRef({ sessionId: 's1', config: { agentType: 'agentic' }, mode: 'agentic' } as never)).toBeNull();
  });

  it('returns null when there is no session', () => {
    expect(acpSessionRef(null)).toBeNull();
    expect(acpSessionRef(undefined)).toBeNull();
  });

  it('derives the client id from an acp: agentType', () => {
    const ref = acpSessionRef({
      sessionId: 's1',
      config: { agentType: 'acp:omp', workspacePath: '/ws' },
      workspacePath: '/ws',
      remoteConnectionId: 'conn-1',
      remoteSshHost: 'host-1',
    } as never);
    expect(ref).toEqual({
      sessionId: 's1',
      clientId: 'omp',
      workspacePath: '/ws',
      remoteConnectionId: 'conn-1',
      remoteSshHost: 'host-1',
    });
  });

  it('falls back to mode when config.agentType is not acp', () => {
    const ref = acpSessionRef({ sessionId: 's2', config: {}, mode: 'acp:claude-code' } as never);
    expect(ref?.clientId).toBe('claude-code');
    expect(ref?.sessionId).toBe('s2');
  });
});
