// @vitest-environment jsdom

import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  connect: vi.fn(),
  listSessions: vi.fn(),
  closeSession: vi.fn(),
}));

vi.mock('@/tools/terminal/services/TerminalService', () => ({
  getTerminalService: () => ({
    connect: mocks.connect,
    listSessions: mocks.listSessions,
    closeSession: mocks.closeSession,
  }),
}));

import { destroyTerminalSession } from './destroyTerminalSession';

describe('destroyTerminalSession', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.connect.mockResolvedValue(undefined);
    mocks.listSessions.mockResolvedValue([{ id: 'terminal-1' }]);
    mocks.closeSession.mockResolvedValue(undefined);
  });

  it('closes the PTY before announcing that the session was destroyed', async () => {
    const destroyed = vi.fn();
    window.addEventListener('terminal-session-destroyed', destroyed);

    await destroyTerminalSession('terminal-1');

    expect(mocks.closeSession).toHaveBeenCalledWith('terminal-1');
    expect(destroyed).toHaveBeenCalledOnce();
    expect((destroyed.mock.calls[0][0] as CustomEvent).detail).toEqual({ sessionId: 'terminal-1' });

    window.removeEventListener('terminal-session-destroyed', destroyed);
  });
});
