import { describe, expect, it } from 'vitest';
import {
  buildGlobalSearchResultPresentation,
  DEFAULT_ENTITY_PREVIEW_LIMIT,
} from './globalSearchResultPresentation';
import type { GlobalSearchGroupId, GlobalSearchItem } from './types';

function item(group: GlobalSearchGroupId, index: number): GlobalSearchItem {
  const entityTarget = group === 'assistants'
    ? { kind: 'assistant' as const, workspaceId: `${group}-${index}` }
    : { kind: 'workspace' as const, workspaceId: `${group}-${index}` };

  return {
    id: `${group}:${index}`,
    providerId: 'test',
    group,
    title: `${group} ${index}`,
    score: 100 - index,
    target: entityTarget,
  };
}

const entityItems = [
  ...Array.from({ length: 4 }, (_, index) => item('workspaces', index)),
  ...Array.from({ length: 3 }, (_, index) => item('assistants', index)),
];

describe('global search result presentation', () => {
  it('shows one workspace and one assistant in the default overview', () => {
    const presentation = buildGlobalSearchResultPresentation(entityItems, {
      hasQuery: false,
      drilldownGroup: null,
    });

    expect(DEFAULT_ENTITY_PREVIEW_LIMIT).toBe(1);
    expect(presentation.groups.map((group) => ({
      id: group.id,
      visible: group.items.length,
      total: group.totalCount,
      canOpenDetails: group.canOpenDetails,
    }))).toEqual([
      { id: 'workspaces', visible: 1, total: 4, canOpenDetails: true },
      { id: 'assistants', visible: 1, total: 3, canOpenDetails: true },
    ]);
    expect(presentation.navigableItems).toHaveLength(2);
  });

  it('keeps all matching entities visible when the user searches', () => {
    const presentation = buildGlobalSearchResultPresentation(entityItems, {
      hasQuery: true,
      drilldownGroup: null,
    });

    expect(presentation.groups.map((group) => group.items.length)).toEqual([4, 3]);
    expect(presentation.groups.every((group) => !group.canOpenDetails)).toBe(true);
  });

  it('shows the complete selected group and excludes other groups on its detail page', () => {
    const presentation = buildGlobalSearchResultPresentation(entityItems, {
      hasQuery: false,
      drilldownGroup: 'assistants',
    });

    expect(presentation.groups).toHaveLength(1);
    expect(presentation.groups[0]).toMatchObject({
      id: 'assistants',
      totalCount: 3,
      canOpenDetails: false,
    });
    expect(presentation.navigableItems.map((entry) => entry.id)).toEqual([
      'assistants:0',
      'assistants:1',
      'assistants:2',
    ]);
  });
});
