import { describe, expect, it } from 'vitest';
import type { GlobalSearchRequest } from '../types';
import { settingsSearchProvider } from './settingsSearchProvider';

function request(query: string): GlobalSearchRequest {
  const labels: Record<string, string> = {
    'navigation.categories.tools': 'Tools & Integrations',
    'navigation.pages.automation.label': 'Automation',
    'navigation.pages.automation.description': 'Quick actions and Agent lifecycle hooks.',
    'navigation.views.quick-actions': 'Quick Actions',
    'navigation.views.hooks': 'Hooks',
    'navigation.pages.mcp.label': 'MCP',
    'navigation.pages.mcp.description': 'Manage MCP servers, connections, and exposed tools.',
    'navigation.pages.acp.label': 'ACP Agents',
    'navigation.pages.acp.description': 'Configure external agents that connect through Agent Client Protocol.',
    'navigation.categories.data': 'Data & Maintenance',
    'navigation.pages.archivedSessions.label': 'Archived Sessions',
    'navigation.pages.archivedSessions.description': 'View, restore, or permanently delete archived sessions.',
  };
  return {
    rawQuery: query,
    query,
    scope: 'all',
    workspaces: [],
    currentWorkspace: null,
    limitPerGroup: 10,
    tCommon: (key) => key,
    tSettings: (key) => labels[key] ?? key,
  };
}

describe('settingsSearchProvider', () => {
  it('targets an internal settings view instead of its parent default view', async () => {
    const result = await settingsSearchProvider.search(
      request('hooks'),
      new AbortController().signal,
    );
    const hooks = result.items.find((item) => item.id === 'settings:tools.automation:hooks');

    expect(hooks?.target).toEqual({
      kind: 'settings',
      destination: { pageId: 'tools.automation', viewId: 'hooks' },
    });
    expect(hooks?.score).toBe(100);
  });

  it('targets archived sessions as a standalone settings page', async () => {
    const result = await settingsSearchProvider.search(
      request('archived sessions'),
      new AbortController().signal,
    );
    const archived = result.items.find((item) => item.id === 'settings:data.archived');

    expect(archived?.title).toBe('Archived Sessions');
    expect(archived?.target).toEqual({
      kind: 'settings',
      destination: { pageId: 'data.archived' },
    });
  });

  it.each([
    ['mcp', 'tools.mcp'],
    ['acp', 'tools.acp'],
  ] as const)('targets %s as a standalone settings page', async (query, pageId) => {
    const result = await settingsSearchProvider.search(
      request(query),
      new AbortController().signal,
    );
    const item = result.items.find((candidate) => candidate.id === `settings:${pageId}`);

    expect(item?.target).toEqual({
      kind: 'settings',
      destination: { pageId },
    });
  });
});
