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

  it('saves cloud speech configuration through one domain command', async () => {
    invokeMock.mockResolvedValueOnce({ modelId: 'speech-1', created: true });

    await expect(configAPI.saveCloudSpeechConfig({
      preset: 'qwen',
      name: 'Qwen ASR',
      baseUrl: 'https://example.com/v1',
      requestUrl: 'https://example.com/v1/audio/transcriptions',
      modelName: 'qwen-asr',
      apiKey: 'secret',
    })).resolves.toEqual({ modelId: 'speech-1', created: true });

    expect(invokeMock).toHaveBeenCalledWith('save_cloud_speech_config', {
      request: {
        preset: 'qwen',
        name: 'Qwen ASR',
        baseUrl: 'https://example.com/v1',
        requestUrl: 'https://example.com/v1/audio/transcriptions',
        modelName: 'qwen-asr',
        apiKey: 'secret',
      },
    });
  });

  it('keeps WebSearch credentials on the dedicated secret commands', async () => {
    invokeMock
      .mockResolvedValueOnce({ provider: 'tavily', configured: false })
      .mockResolvedValueOnce({ provider: 'tavily', configured: true })
      .mockResolvedValueOnce({ provider: 'tavily', configured: false });

    await configAPI.getWebSearchCredentialStatus('tavily');
    await configAPI.saveWebSearchCredential('tavily', 'tvly-secret');
    await configAPI.clearWebSearchCredential('tavily');

    expect(invokeMock).toHaveBeenNthCalledWith(1, 'get_web_search_credential_status', {
      request: { provider: 'tavily' },
    });
    expect(invokeMock).toHaveBeenNthCalledWith(2, 'save_web_search_credential', {
      request: { provider: 'tavily', secret: 'tvly-secret' },
    });
    expect(invokeMock).toHaveBeenNthCalledWith(3, 'clear_web_search_credential', {
      request: { provider: 'tavily' },
    });
    expect(invokeMock).not.toHaveBeenCalledWith(
      'set_config',
      expect.objectContaining({ request: expect.objectContaining({ value: 'tvly-secret' }) }),
    );
  });

  it('bounds Skill catalog requests at sixty seconds', async () => {
    invokeMock.mockResolvedValue([]);

    await configAPI.getSkillConfigs({ workspacePath: '/remote/project' });
    await configAPI.getModeSkillConfigs({
      modeId: 'agentic',
      workspacePath: '/remote/project',
    });

    expect(invokeMock).toHaveBeenNthCalledWith(
      1,
      'get_skill_configs',
      { forceRefresh: undefined, workspacePath: '/remote/project' },
      { timeout: 60_000 },
    );
    expect(invokeMock).toHaveBeenNthCalledWith(
      2,
      'get_mode_skill_configs',
      {
        modeId: 'agentic',
        forceRefresh: undefined,
        workspacePath: '/remote/project',
      },
      { timeout: 60_000 },
    );
  });

  it('rejects unsuccessful imports without attaching credential-bearing documents to errors', async () => {
    invokeMock.mockResolvedValueOnce({ success: false, errors: ['Invalid config'], warnings: [] });
    const document = { config: { app: { voice_call: { api_key: 'fixture-import-key' } } } };

    const error = await configAPI.importConfig(document).catch(error => error);

    expect(error).toBeInstanceOf(Error);
    expect(error.message).toContain('Invalid config');
    expect(error.context.request).toBeUndefined();
    expect(JSON.stringify(error)).not.toContain('fixture-import-key');
  });

  it('accepts a confirmed import and rejects an ambiguous response', async () => {
    invokeMock.mockResolvedValueOnce({ success: true, errors: [], warnings: [] });
    await expect(configAPI.importConfig({})).resolves.toBeUndefined();

    invokeMock.mockResolvedValueOnce(undefined);
    await expect(configAPI.importConfig({})).rejects.toThrow('Configuration import was not confirmed');
  });
});
