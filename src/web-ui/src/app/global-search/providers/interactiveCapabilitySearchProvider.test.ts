import { describe, expect, it } from 'vitest';
import { INTERACTIVE_CAPABILITY_CATALOG } from '../interactiveCapabilityCatalog';
import type { GlobalSearchRequest } from '../types';
import { interactiveCapabilitySearchProvider } from './interactiveCapabilitySearchProvider';

function request(query: string): GlobalSearchRequest {
  return {
    rawQuery: query,
    query,
    scope: 'all',
    workspaces: [],
    currentWorkspace: null,
    limitPerGroup: 20,
    tCommon: (key) => key,
    tSettings: (key) => key,
  };
}

describe('interactiveCapabilitySearchProvider', () => {
  it('uses the curated feature-and-settings contract', () => {
    expect(INTERACTIVE_CAPABILITY_CATALOG.capabilities).toHaveLength(
      INTERACTIVE_CAPABILITY_CATALOG.counts.userFacing,
    );
    expect(new Set(INTERACTIVE_CAPABILITY_CATALOG.capabilities.map(({ id }) => id)).size)
      .toBe(INTERACTIVE_CAPABILITY_CATALOG.capabilities.length);
    expect(INTERACTIVE_CAPABILITY_CATALOG.capabilities.every(({ kind }) =>
      kind === 'feature' || kind === 'setting')).toBe(true);
  });

  it('finds a capability by its English terms', async () => {
    const result = await interactiveCapabilitySearchProvider.search(
      request('terminal'),
      new AbortController().signal,
    );
    expect(result.items.some((item) => item.target.kind === 'capability'
      && item.target.capabilityId === 'feature.terminal')).toBe(true);
  });

  it('finds the same catalog by Chinese terms', async () => {
    const result = await interactiveCapabilitySearchProvider.search(
      request('终端'),
      new AbortController().signal,
    );
    expect(result.items.some((item) => item.target.kind === 'capability'
      && item.target.capabilityId === 'feature.terminal')).toBe(true);
  });

  it('returns settings from the same semantic provider in the settings group', async () => {
    const result = await interactiveCapabilitySearchProvider.search(
      request('MCP server'),
      new AbortController().signal,
    );
    const mcp = result.items.find((item) => item.target.kind === 'capability'
      && item.target.capabilityId === 'setting.tools.mcp');
    expect(mcp?.providerId).toBe('interactive-capabilities');
    expect(mcp?.group).toBe('settings');
  });

  it('discovers the dedicated terminal settings page', async () => {
    const result = await interactiveCapabilitySearchProvider.search(
      request('default shell settings'),
      new AbortController().signal,
    );
    expect(result.items.some((item) => item.target.kind === 'capability'
      && item.target.capabilityId === 'setting.application.terminal')).toBe(true);
  });

  it.each([
    ['快捷键', 'setting.application.shortcuts', 'shortcut-browser'],
    ['editor font', 'setting.application.development', 'editor-appearance'],
    ['Hooks 设置', 'setting.tools.automation', 'hooks-enabled'],
  ])('routes %s to the matching settings subview', async (query, capabilityId, itemId) => {
    const result = await interactiveCapabilitySearchProvider.search(
      request(query),
      new AbortController().signal,
    );
    const match = result.items.find((item) => item.target.kind === 'capability'
      && item.target.capabilityId === capabilityId);
    expect(match?.target).toEqual({ kind: 'capability', capabilityId, itemId });
  });

  it('finds the browser element picker from Chinese and English sub-capability text', async () => {
    for (const query of ['元素选择器', 'element picker']) {
      const result = await interactiveCapabilitySearchProvider.search(
        request(query),
        new AbortController().signal,
      );
      const browser = result.items.find((item) => item.target.kind === 'capability'
        && item.target.capabilityId === 'feature.browser');
      expect(browser).toBeDefined();
      expect(browser?.subtitle.toLowerCase()).toContain(
        query === '元素选择器' ? '元素选择器' : 'element picker',
      );
      expect(browser?.context).toContain('feature.browser:element-picker');
    }
  });

  it('keeps personal assistants separate from subagent management', async () => {
    for (const query of ['育苗场', 'persona documents']) {
      const result = await interactiveCapabilitySearchProvider.search(
        request(query),
        new AbortController().signal,
      );
      const assistant = result.items.find((item) => item.target.kind === 'capability'
        && item.target.capabilityId === 'feature.personal-assistants');
      expect(assistant).toBeDefined();
      expect(assistant?.target.kind === 'capability' && assistant.target.capabilityId)
        .not.toBe('feature.agents');
    }
  });

  it('satisfies the shared cross-surface search acceptance corpus', async () => {
    for (const acceptance of INTERACTIVE_CAPABILITY_CATALOG.searchAcceptance) {
      const result = await interactiveCapabilitySearchProvider.search(
        request(acceptance.query),
        new AbortController().signal,
      );
      const ranked = [...result.items].sort((left, right) => right.score - left.score);
      expect(
        ranked[0]?.target.kind === 'capability' ? ranked[0].target.capabilityId : undefined,
        acceptance.id,
      ).toBe(acceptance.expectedFirstCapabilityId);
      const capabilityIds = new Set(result.items.flatMap((item) =>
        item.target.kind === 'capability' ? [item.target.capabilityId] : []));
      for (const capabilityId of acceptance.expectedCapabilityIds) {
        expect(capabilityIds.has(capabilityId), `${acceptance.id} missed ${capabilityId}`).toBe(true);
      }
      if (acceptance.expectedItem) {
        const match = result.items.find((item) => item.target.kind === 'capability'
          && item.target.capabilityId === acceptance.expectedItem?.capabilityId);
        expect(match?.target, `${acceptance.id} item route`).toEqual({
          kind: 'capability',
          capabilityId: acceptance.expectedItem.capabilityId,
          itemId: acceptance.expectedItem.itemId,
        });
      }
    }
  });
});
