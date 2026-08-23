import {
  INTERACTIVE_CAPABILITY_CATALOG,
  type InteractiveCapability,
} from '../interactiveCapabilityCatalog';
import { scoreTextMatch } from '../searchMatching';
import type { GlobalSearchProvider } from '../types';

const CJK_QUERY = /[\u3400-\u9fff]/u;

function resultTitle(capability: InteractiveCapability, query: string) {
  return CJK_QUERY.test(query) ? capability.titleZh : capability.titleEn;
}

function matchingItem(capability: InteractiveCapability, query: string) {
  const queryTokens = query.trim().split(/[\s._/\\:-]+/u).filter(Boolean);
  return capability.items
    .map((item) => {
      const directScore = scoreTextMatch(query, [item.titleZh, item.titleEn]);
      const partialScore = Math.max(0, ...queryTokens.map((token) =>
        scoreTextMatch(token, [item.titleZh, item.titleEn])));
      return {
        item,
        score: directScore > 0 ? 100 + directScore : Math.floor(partialScore / 2),
      };
    })
    .filter(({ score }) => score > 0)
    .sort((left, right) => right.score - left.score)[0]?.item;
}

function resultSubtitle(
  capability: InteractiveCapability,
  query: string,
  item: ReturnType<typeof matchingItem>,
) {
  const category = INTERACTIVE_CAPABILITY_CATALOG.categories[capability.categoryId];
  const summary = item
    ? (CJK_QUERY.test(query) ? item.titleZh : item.titleEn)
    : (CJK_QUERY.test(query) ? capability.summaryZh : capability.summaryEn);
  const categoryTitle = CJK_QUERY.test(query) ? category?.titleZh : category?.titleEn;
  return [categoryTitle, summary].filter(Boolean).join(' · ');
}

export const interactiveCapabilitySearchProvider: GlobalSearchProvider = {
  id: 'interactive-capabilities',
  groups: ['capabilities', 'settings'],
  search: (request) => {
    if (request.scope === 'content' || !request.query) return { items: [] };

    const items = INTERACTIVE_CAPABILITY_CATALOG.capabilities
      .map((capability) => {
        const item = matchingItem(capability, request.query);
        const textScore = scoreTextMatch(request.query, [
          capability.id,
          capability.titleZh,
          capability.titleEn,
          ...capability.searchTerms,
        ]);
        const directControlBonus = Math.min(
          8,
          capability.operations.length + capability.options.length,
        );
        const delegatedControlBonus = capability.items.some(({ control }) =>
          control.kind === 'delegate') ? 4 : 0;
        return {
          id: `capability:${capability.id}`,
          providerId: 'interactive-capabilities',
          group: capability.kind === 'setting' ? 'settings' as const : 'capabilities' as const,
          title: resultTitle(capability, request.query),
          subtitle: resultSubtitle(capability, request.query, item),
          context: item ? `${capability.id}:${item.id}` : capability.id,
          badge: capability.kind,
          score: textScore > 0 ? textScore + directControlBonus + delegatedControlBonus : 0,
          target: {
            kind: 'capability' as const,
            capabilityId: capability.id,
            itemId: item?.id,
          },
        };
      })
      .filter((item) => item.score > 0);

    return { items };
  },
};
