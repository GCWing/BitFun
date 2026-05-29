import { describe, expect, it } from 'vitest';
import { enrichCapabilities, getAgentDescription } from './utils';
import type { AgentWithCapabilities } from './agentsStore';

function makeAgent(overrides: Partial<AgentWithCapabilities> = {}): AgentWithCapabilities {
  return {
    key: overrides.id ?? 'IntentCoding',
    id: 'IntentCoding',
    name: 'Intent Coding',
    description: 'backend fallback',
    isReadonly: false,
    isReview: false,
    toolCount: 1,
    defaultTools: [],
    defaultEnabled: true,
    effectiveEnabled: true,
    capabilities: [],
    agentKind: 'mode',
    ...overrides,
  };
}

describe('agents utils', () => {
  it('resolves IntentCoding mode description from the canonical locale key', () => {
    const t = ((key: string) => {
      if (key === 'agentDescriptions.IntentCoding') {
        return 'Intent Coding translated description';
      }
      return '';
    }) as any;

    expect(getAgentDescription(t, makeAgent())).toBe('Intent Coding translated description');
  });

  it('adds coding and testing capabilities for IntentCoding mode', () => {
    const enriched = enrichCapabilities(makeAgent());

    expect(enriched.capabilities).toEqual([
      { category: 'coding', level: 5 },
      { category: 'testing', level: 4 },
    ]);
  });
});
