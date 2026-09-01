import {
  GLOBAL_SEARCH_GROUP_ORDER,
  type GlobalSearchGroupId,
  type GlobalSearchItem,
} from './types';

export const DEFAULT_ENTITY_PREVIEW_LIMIT = 1;

export type GlobalSearchDrilldownGroupId = Extract<
  GlobalSearchGroupId,
  'workspaces' | 'assistants'
>;

export interface GlobalSearchGroupView {
  id: GlobalSearchGroupId;
  items: GlobalSearchItem[];
  totalCount: number;
  canOpenDetails: boolean;
}

export interface GlobalSearchResultPresentation {
  groups: GlobalSearchGroupView[];
  navigableItems: GlobalSearchItem[];
}

export function isGlobalSearchDrilldownGroup(
  groupId: GlobalSearchGroupId,
): groupId is GlobalSearchDrilldownGroupId {
  return groupId === 'workspaces' || groupId === 'assistants';
}

export function buildGlobalSearchResultPresentation(
  items: readonly GlobalSearchItem[],
  options: {
    hasQuery: boolean;
    drilldownGroup: GlobalSearchDrilldownGroupId | null;
  },
): GlobalSearchResultPresentation {
  const grouped = new Map<GlobalSearchGroupId, GlobalSearchItem[]>();
  items.forEach((item) => {
    const group = grouped.get(item.group) ?? [];
    group.push(item);
    grouped.set(item.group, group);
  });

  const groupOrder = options.drilldownGroup
    ? [options.drilldownGroup]
    : GLOBAL_SEARCH_GROUP_ORDER;

  const groups = groupOrder.flatMap<GlobalSearchGroupView>((groupId) => {
    const groupItems = grouped.get(groupId) ?? [];
    if (groupItems.length === 0) return [];

    const useDefaultEntityPreview =
      !options.hasQuery
      && !options.drilldownGroup
      && isGlobalSearchDrilldownGroup(groupId);
    const visibleItems = useDefaultEntityPreview
      ? groupItems.slice(0, DEFAULT_ENTITY_PREVIEW_LIMIT)
      : groupItems;

    return [{
      id: groupId,
      items: visibleItems,
      totalCount: groupItems.length,
      canOpenDetails: useDefaultEntityPreview
        && groupItems.length > DEFAULT_ENTITY_PREVIEW_LIMIT,
    }];
  });

  return {
    groups,
    navigableItems: groups.flatMap((group) => group.items),
  };
}
