import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  connect: vi.fn(),
  listSessions: vi.fn(),
  createSession: vi.fn(),
  getConfig: vi.fn(),
}));

vi.mock('@/tools/terminal/services/TerminalService', () => ({
  getTerminalService: () => ({
    connect: mocks.connect,
    listSessions: mocks.listSessions,
    createSession: mocks.createSession,
  }),
}));

vi.mock('@/infrastructure/config/services/ConfigManager', () => ({
  configManager: { getConfig: mocks.getConfig },
}));

import { createManualTerminalSession } from './createManualTerminalSession';

describe('createManualTerminalSession', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.connect.mockResolvedValue(undefined);
    mocks.getConfig.mockResolvedValue({ default_shell: 'PowerShell' });
    mocks.listSessions.mockResolvedValue([
      { id: 'manual-1', source: 'manual' },
      { id: 'agent-1', source: 'agent' },
    ]);
    mocks.createSession.mockResolvedValue({ id: 'manual-2', name: 'Shell 2' });
  });

  it('creates a terminal directly for the active workspace and connection', async () => {
    await expect(createManualTerminalSession({
      workspacePath: '/workspace/project',
      connectionId: 'ssh-1',
    })).resolves.toEqual({ id: 'manual-2', name: 'Shell 2' });

    expect(mocks.connect).toHaveBeenCalledOnce();
    expect(mocks.createSession).toHaveBeenCalledWith({
      workingDirectory: '/workspace/project',
      connectionId: 'ssh-1',
      name: 'Shell 2',
      shellType: 'PowerShell',
      source: 'manual',
    });
  });
});
