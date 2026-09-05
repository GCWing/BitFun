import { describe, expect, it } from 'vitest';
import type { AIModelConfig, ReasoningConfig } from '../types';
import {
  canonicalReasoningConfig,
  validateReasoningConfig,
} from './reasoningPresets';

function model(overrides: Partial<AIModelConfig> = {}): AIModelConfig {
  return {
    name: 'Test provider',
    provider: 'anthropic',
    base_url: 'https://example.test',
    model_name: 'test-model',
    enabled: true,
    category: 'general_chat',
    capabilities: ['text_chat'],
    ...overrides,
  };
}

describe('canonical reasoning presets', () => {
  it('uses Auto when canonical reasoning is absent', () => {
    expect(canonicalReasoningConfig(model())).toEqual({
      catalog: { source: 'auto' },
      presets: [],
    });
  });

  it('keeps canonical reasoning authoritative', () => {
    const canonical: ReasoningConfig = {
      catalog: { source: 'disabled' },
      default_preset: 'custom',
      presets: [{ id: 'custom', actions: [{ type: 'toggle', enabled: false }] }],
    };
    expect(canonicalReasoningConfig(model({ reasoning: canonical }))).toEqual(canonical);
  });

  it('rejects duplicate IDs, invalid explicit catalog bindings, and missing defaults', () => {
    expect(validateReasoningConfig({
      catalog: { source: 'models_dev', provider: '', model: 'gpt-5' },
      presets: [],
    })).toBe('catalog_binding');
    expect(validateReasoningConfig({
      catalog: { source: 'auto' },
      presets: [
        { id: 'high', actions: [{ type: 'effort', value: 'high' }] },
        { id: 'high', actions: [{ type: 'effort', value: 'xhigh' }] },
      ],
    })).toBe('duplicate_preset_id');
    expect(validateReasoningConfig({
      catalog: { source: 'auto' },
      default_preset: 'missing',
      presets: [],
    })).toBe('default_preset');
  });

  it('accepts a generated catalog preset as the model default', () => {
    expect(validateReasoningConfig({
      catalog: { source: 'auto' },
      default_preset: 'high',
      presets: [],
    }, ['low', 'high'])).toBeNull();
  });

  it('rejects duplicate singleton actions', () => {
    expect(validateReasoningConfig({
      catalog: { source: 'auto' },
      presets: [{
        id: 'custom',
        actions: [
          { type: 'effort', value: 'medium' },
          { type: 'effort', value: 'high' },
        ],
      }],
    })).toBe('preset_setting');
  });

  it('accepts multiple request patches and distinct typed actions', () => {
    expect(validateReasoningConfig({
      catalog: { source: 'auto' },
      presets: [{
        id: 'custom',
        actions: [
          { type: 'effort', value: 'high' },
          { type: 'toggle', enabled: true },
          { type: 'budget_tokens', value: 8192 },
          { type: 'request_patch', body: { reasoning: { summary: 'detailed' } } },
          { type: 'request_patch', body: { reasoning: { summary: 'concise' } } },
        ],
      }],
    })).toBeNull();
  });

});
