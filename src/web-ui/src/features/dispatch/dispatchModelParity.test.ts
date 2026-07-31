import { describe, expect, it } from 'vitest';
import {
  compareDispatchModels,
  syncableLocalModelIds,
} from './dispatchModelParity';
import type { AIModelConfig } from '@/infrastructure/config/types';

function model(overrides: Partial<AIModelConfig> & { id: string }): AIModelConfig {
  return {
    name: 'Anthropic',
    provider: 'anthropic',
    base_url: 'https://example.test',
    model_name: 'claude',
    api_key: 'secret',
    enabled: true,
    category: 'chat',
    capabilities: [],
    ...overrides,
  } as AIModelConfig;
}

describe('syncableLocalModelIds', () => {
  it('keeps only what a target could construct a client for', () => {
    expect(
      syncableLocalModelIds([
        model({ id: 'ready' }),
        model({ id: 'disabled', enabled: false }),
        model({ id: 'no-key', api_key: '  ' }),
        model({ id: '   ' }),
      ]),
    ).toEqual(['ready']);
  });

  it('keeps a subscription model without an inline key', () => {
    expect(
      syncableLocalModelIds([
        model({ id: 'oauth', api_key: '', auth: { type: 'subscription', provider: 'codex' } }),
      ]),
    ).toEqual(['oauth']);
  });

  it('reports an unreadable catalog as unknown rather than empty', () => {
    expect(syncableLocalModelIds(null)).toBeNull();
    expect(syncableLocalModelIds(undefined)).toBeNull();
    expect(syncableLocalModelIds([])).toEqual([]);
  });
});

describe('compareDispatchModels', () => {
  it('matches on the same id set regardless of order', () => {
    expect(compareDispatchModels(['a', 'b'], ['b', 'a'])).toBe('match');
  });

  it('diverges when the target is missing or carries an extra model', () => {
    expect(compareDispatchModels(['a', 'b'], ['a'])).toBe('diverged');
    expect(compareDispatchModels(['a'], ['a', 'b'])).toBe('diverged');
    expect(compareDispatchModels(['a'], ['b'])).toBe('diverged');
  });

  it('never claims parity without both sides', () => {
    expect(compareDispatchModels(null, ['a'])).toBe('unknown');
    expect(compareDispatchModels(['a'], undefined)).toBe('unknown');
  });

  it('treats two empty catalogs as matching', () => {
    expect(compareDispatchModels([], [])).toBe('match');
  });
});
