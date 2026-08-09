import { describe, expect, it } from 'vitest';
import type { AIModelConfig, ReasoningConfig } from '../types';
import {
  availableReasoningActionTypes,
  canonicalReasoningConfig,
  nextReasoningActionType,
  resolveDefaultReasoningEffortValue,
  resolveReasoningEffortValues,
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
  it('uses model-specific effort values when the catalog provides them', () => {
    expect(resolveReasoningEffortValues({
      status: 'known',
      presets: [
        {
          id: 'low',
          label: 'Low',
          order: 10,
          actions: [{ type: 'effort', value: 'low' }],
          source: 'models_dev',
        },
        {
          id: 'high',
          label: 'High',
          order: 20,
          actions: [{ type: 'effort', value: 'high' }],
          source: 'adapter_fallback',
        },
        {
          id: 'custom',
          label: 'Custom',
          order: 30,
          actions: [{ type: 'effort', value: 'vendor-level' }],
          source: 'model_config',
        },
      ],
    })).toEqual(['low', 'high']);
  });

  it('falls back to common effort candidates without catalog values', () => {
    expect(resolveReasoningEffortValues(undefined)).toEqual([
      'none', 'minimal', 'low', 'medium', 'high', 'xhigh', 'max',
    ]);
  });

  it('defaults effort from the model projection instead of assuming medium', () => {
    const projection = {
      status: 'known' as const,
      default_preset: 'high',
      presets: [
        {
          id: 'low',
          label: 'Low',
          order: 10,
          actions: [{ type: 'effort' as const, value: 'low' }],
          source: 'models_dev' as const,
        },
        {
          id: 'high',
          label: 'High',
          order: 20,
          actions: [{ type: 'effort' as const, value: 'high' }],
          source: 'models_dev' as const,
        },
      ],
    };

    expect(resolveDefaultReasoningEffortValue(projection)).toBe('high');
    expect(resolveDefaultReasoningEffortValue({
      ...projection,
      default_preset: undefined,
    })).toBe('low');
    expect(resolveDefaultReasoningEffortValue(undefined)).toBe('medium');
  });

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

  it('offers only unused singleton action types while keeping request patches repeatable', () => {
    const actions = [
      { type: 'effort', value: 'medium' },
      { type: 'request_patch', body: {} },
    ] satisfies ReasoningConfig['presets'][number]['actions'];

    expect(availableReasoningActionTypes(actions)).toEqual([
      'toggle',
      'budget_tokens',
      'request_patch',
    ]);
    expect(availableReasoningActionTypes(actions, 0)).toEqual([
      'effort',
      'toggle',
      'budget_tokens',
      'request_patch',
    ]);
    expect(nextReasoningActionType(actions)).toBe('toggle');
  });

  it('adds request patches after every singleton action type is used', () => {
    expect(nextReasoningActionType([
      { type: 'effort', value: 'medium' },
      { type: 'toggle', enabled: true },
      { type: 'budget_tokens', value: 8192 },
      { type: 'request_patch', body: {} },
    ])).toBe('request_patch');
  });
});
