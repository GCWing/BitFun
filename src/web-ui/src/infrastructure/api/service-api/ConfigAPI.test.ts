import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ConfigAPI } from './ConfigAPI';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('./ApiClient', () => ({
  api: {
    invoke: invokeMock,
  },
}));

describe('ConfigAPI batch config reads', () => {
  let configAPI: ConfigAPI;

  beforeEach(() => {
    configAPI = new ConfigAPI();
    invokeMock.mockReset();
  });

  it('reads multiple config paths through one batch command', async () => {
    const configs = {
      'ai.models': [],
      'ai.default_models': { chat: 'gpt-5' },
    };
    invokeMock.mockResolvedValueOnce(configs);

    await expect(
      configAPI.getConfigs(['ai.models', 'ai.models', 'ai.default_models'])
    ).resolves.toEqual(configs);

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith('get_configs', {
      request: {
        paths: ['ai.models', 'ai.default_models'],
        skipRetryOnNotFound: false,
      },
    });
  });

  it('falls back to existing single-path reads when the batch command fails', async () => {
    invokeMock.mockImplementation((command: string, args?: any) => {
      if (command === 'get_configs') {
        return Promise.reject(new Error('unknown command get_configs'));
      }

      return Promise.resolve(`value:${args.request.path}`);
    });

    await expect(configAPI.getConfigs(['ai.models', 'ai.default_models'])).resolves.toEqual({
      'ai.models': 'value:ai.models',
      'ai.default_models': 'value:ai.default_models',
    });

    expect(invokeMock).toHaveBeenCalledTimes(3);
    expect(invokeMock).toHaveBeenNthCalledWith(1, 'get_configs', {
      request: {
        paths: ['ai.models', 'ai.default_models'],
        skipRetryOnNotFound: false,
      },
    });
    expect(invokeMock).toHaveBeenNthCalledWith(2, 'get_config', {
      request: {
        path: 'ai.models',
        skipRetryOnNotFound: false,
      },
    }, undefined);
    expect(invokeMock).toHaveBeenNthCalledWith(3, 'get_config', {
      request: {
        path: 'ai.default_models',
        skipRetryOnNotFound: false,
      },
    }, undefined);
  });
});
