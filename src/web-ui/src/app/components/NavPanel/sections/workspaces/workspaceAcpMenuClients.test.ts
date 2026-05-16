import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ACPClientAPI } from '@/infrastructure/api/service-api/ACPClientAPI';
import { loadWorkspaceAcpMenuClients } from './workspaceAcpMenuClients';

vi.mock('@/infrastructure/api/service-api/ACPClientAPI', () => ({
  ACPClientAPI: {
    getClients: vi.fn(),
    probeClientRequirements: vi.fn(),
  },
}));

function client(id: string, enabled: boolean) {
  return {
    id,
    name: id,
    command: id,
    args: [],
    enabled,
    readonly: false,
    permissionMode: 'ask' as const,
    status: 'configured' as const,
    toolName: `acp__${id}__prompt`,
    sessionCount: 0,
  };
}

describe('loadWorkspaceAcpMenuClients', () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it('does not probe external ACP executables while loading workspace menu clients', async () => {
    vi.mocked(ACPClientAPI.getClients).mockResolvedValue([
      client('opencode', true),
      client('disabled-client', false),
    ]);

    const clients = await loadWorkspaceAcpMenuClients();

    expect(ACPClientAPI.getClients).toHaveBeenCalledTimes(1);
    expect(ACPClientAPI.probeClientRequirements).not.toHaveBeenCalled();
    expect(clients.map(item => item.id)).toEqual(['opencode']);
  });

  it('uses built-in ACP presets for remote workspaces without requiring local config', async () => {
    vi.mocked(ACPClientAPI.getClients).mockResolvedValue([
      client('claude-code', false),
      client('custom-remote-only', true),
    ]);
    vi.mocked(ACPClientAPI.probeClientRequirements).mockResolvedValue([
      {
        id: 'opencode',
        tool: { name: 'opencode', installed: false },
        runnable: false,
        notes: ['opencode is not available on remote PATH'],
      },
      {
        id: 'claude-code',
        tool: { name: 'claude', installed: false },
        adapter: { name: '@zed-industries/claude-code-acp', installed: true },
        runnable: false,
        notes: ['claude is not available on remote PATH'],
      },
      {
        id: 'codex',
        tool: { name: 'codex', installed: true },
        adapter: { name: '@zed-industries/codex-acp', installed: true },
        runnable: true,
        notes: [],
      },
    ]);

    const clients = await loadWorkspaceAcpMenuClients({
      remoteWorkspace: true,
      remoteConnectionId: 'ssh-1',
    });

    expect(ACPClientAPI.probeClientRequirements).toHaveBeenCalledWith({
      remoteConnectionId: 'ssh-1',
    });
    expect(clients.map(item => item.id)).toEqual(['codex']);
    expect(clients[0]?.enabled).toBe(true);
  });
});
