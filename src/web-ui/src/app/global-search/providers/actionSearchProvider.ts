import { PRODUCT_ACTION_CATALOG } from '../productActionCatalog';
import { scoreTextMatch } from '../searchMatching';
import type { GlobalSearchProvider } from '../types';

const DEFAULT_ACTION_COUNT = 6;

export const actionSearchProvider: GlobalSearchProvider = {
  id: 'actions',
  groups: ['actions'],
  search: (request) => {
    if (request.scope === 'content') return { items: [] };

    const items = PRODUCT_ACTION_CATALOG
      .filter((action) => !action.requiresWorkspace || request.currentWorkspace !== null)
      .map((action) => {
        const title = request.tCommon(action.labelKey);
        const subtitle = request.tCommon(action.descriptionKey);
        const matchScore = scoreTextMatch(request.query, [title, subtitle, ...action.aliases]);
        return {
          id: `action:${action.id}`,
          providerId: 'actions',
          group: 'actions' as const,
          title,
          subtitle,
          score: request.query ? matchScore : action.defaultPriority,
          target: { kind: 'action' as const, actionId: action.id },
        };
      })
      .filter((item) => !request.query || item.score > 0)
      .sort((left, right) => right.score - left.score)
      .slice(0, request.query ? request.limitPerGroup : DEFAULT_ACTION_COUNT);

    return { items };
  },
};
