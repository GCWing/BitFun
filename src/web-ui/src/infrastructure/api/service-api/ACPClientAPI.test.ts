import { beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('./ApiClient', () => ({
  api: {
    invoke: invokeMock,
  },
}));

async function importApi() {
  vi.resetModules();
  return (await import('./ACPClientAPI')).default;
}

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe('ACPClientAPI client list startup cache', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    vi.useRealTimers();
    vi.stubGlobal('window', { dispatchEvent: vi.fn() });
  });

  it('deduplicates concurrent client list requests', async () => {
    const ACPClientAPI = await importApi();
    const clients = [
      {
        id: 'claude',
        name: 'Claude',
        command: 'claude',
        args: [],
        enabled: true,
        readonly: false,
        permissionMode: 'ask',
        status: 'configured',
        toolName: 'claude',
        sessionCount: 0,
      },
    ];
    const deferred = createDeferred<typeof clients>();
    invokeMock.mockReturnValueOnce(deferred.promise);

    const first = ACPClientAPI.getClients();
    const second = ACPClientAPI.getClients();

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith('get_acp_clients');

    deferred.resolve(clients);
    await expect(Promise.all([first, second])).resolves.toEqual([clients, clients]);
  });

  it('serves a recently resolved client list from memory until clients change', async () => {
    const ACPClientAPI = await importApi();
    invokeMock
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce([
        {
          id: 'codex',
          name: 'Codex',
          command: 'codex',
          args: [],
          enabled: true,
          readonly: false,
          permissionMode: 'ask',
          status: 'configured',
          toolName: 'codex',
          sessionCount: 0,
        },
      ]);

    await expect(ACPClientAPI.getClients()).resolves.toEqual([]);
    await expect(ACPClientAPI.getClients()).resolves.toEqual([]);
    expect(invokeMock).toHaveBeenCalledTimes(1);

    await ACPClientAPI.initializeClients();
    await ACPClientAPI.getClients();
    expect(invokeMock).toHaveBeenCalledTimes(3);
    expect(invokeMock).toHaveBeenNthCalledWith(2, 'initialize_acp_clients');
    expect(invokeMock).toHaveBeenNthCalledWith(3, 'get_acp_clients');
  });

  it('patches one client subagent profile without discarding the ACP registry', async () => {
    const ACPClientAPI = await importApi();
    invokeMock
      .mockResolvedValueOnce(JSON.stringify({
        acpClients: {
          codex: {
            name: 'Codex',
            command: 'codex',
            enabled: true,
          },
          opencode: {
            name: 'OpenCode',
            command: 'opencode',
            enabled: true,
          },
        },
      }))
      .mockResolvedValueOnce(undefined);

    await ACPClientAPI.updateClientSubagentConfig({
      clientId: 'codex',
      enabled: true,
      description: '  Implements complex code changes  ',
      bestFor: ' Cross-file refactors ',
    });

    expect(invokeMock).toHaveBeenNthCalledWith(1, 'load_acp_json_config');
    expect(invokeMock).toHaveBeenNthCalledWith(
      2,
      'save_acp_json_config',
      expect.objectContaining({ jsonConfig: expect.any(String) })
    );
    const saved = JSON.parse(invokeMock.mock.calls[1][1].jsonConfig);
    expect(saved.acpClients.codex.subagent).toEqual({
      enabled: true,
      description: 'Implements complex code changes',
      bestFor: 'Cross-file refactors',
    });
    expect(saved.acpClients.opencode.command).toBe('opencode');
  });
});
