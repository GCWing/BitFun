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
});
