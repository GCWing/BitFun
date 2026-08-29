/**
 * @vitest-environment jsdom
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { aiApi } from '@/infrastructure/api/service-api/AIApi';
import { setRecentReasoningPreset } from './reasoningPresets';
import { resolveReasoningPresetForSessionCreation } from './modelResolution';

vi.mock('@/infrastructure/api/service-api/AIApi', () => ({
  aiApi: { getModelCatalog: vi.fn() },
}));

describe('reasoning preset session creation resolution', () => {
  beforeEach(() => {
    window.localStorage.clear();
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

  it('resolves Primary through the concrete primary model and restores its recent preset', async () => {
    setRecentReasoningPreset('model-primary', 'high');
    await expect(resolveReasoningPresetForSessionCreation('primary')).resolves.toBe('high');
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
