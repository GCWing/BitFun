/**
 * @vitest-environment jsdom
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { aiApi } from '@/infrastructure/api/service-api/AIApi';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { setRecentReasoningPreset } from './reasoningPresets';
import { resolveReasoningPresetForSessionCreation } from './modelResolution';

vi.mock('@/infrastructure/api/service-api/AIApi', () => ({
  aiApi: { getModelCatalog: vi.fn() },
}));

vi.mock('@/infrastructure/config/services/ConfigManager', () => ({
  configManager: { getConfigs: vi.fn() },
}));

describe('reasoning preset session creation resolution', () => {
  beforeEach(() => {
    const storage = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, String(value)),
      removeItem: (key: string) => storage.delete(key),
      clear: () => storage.clear(),
      key: (index: number) => [...storage.keys()][index] ?? null,
      get length() { return storage.size; },
    });
    vi.mocked(configManager.getConfigs).mockResolvedValue({
      'ai.agent_model_defaults': { mode: 'primary' },
    });
    vi.mocked(aiApi.getModelCatalog).mockResolvedValue({
      version: 1,
      default_models: { primary: 'model-primary', fast: 'model-fast' },
      models: [
        {
          id: 'model-primary',
          name: 'Primary',
          provider: 'responses',
          base_url: 'https://example.test',
          model_name: 'gpt-primary',
          enabled: true,
          capabilities: ['text_chat'],
          reasoning: {
            status: 'known',
            presets: [{
              id: 'high',
              label: 'High',
              order: 10,
              source: 'models_dev',
              actions: [{ type: 'effort', value: 'high' }],
            }],
          },
        },
        {
          id: 'model-fast',
          name: 'Fast',
          provider: 'responses',
          base_url: 'https://example.test',
          model_name: 'gpt-fast',
          enabled: true,
          capabilities: ['text_chat'],
          reasoning: { status: 'unsupported', presets: [] },
        },
      ],
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('resolves Primary through the concrete primary model and restores its recent preset', async () => {
    setRecentReasoningPreset('model-primary', 'high');
    await expect(resolveReasoningPresetForSessionCreation('primary')).resolves.toBe('high');
  });

  it('uses the configured mode model for a new session without an explicit model', async () => {
    vi.mocked(configManager.getConfigs).mockResolvedValue({
      'ai.agent_model_defaults': { mode: 'primary' },
    });
    setRecentReasoningPreset('model-primary', 'high');

    await expect(resolveReasoningPresetForSessionCreation()).resolves.toBe('high');
  });

  it('falls back to Primary when a stale selector is restored', async () => {
    setRecentReasoningPreset('model-primary', 'high');
    await expect(resolveReasoningPresetForSessionCreation('removed-model')).resolves.toBe('high');
  });

  it('fails closed when the concrete model does not expose a known preset', async () => {
    setRecentReasoningPreset('model-fast', 'high');
    await expect(resolveReasoningPresetForSessionCreation('fast')).resolves.toBeUndefined();
  });
});
