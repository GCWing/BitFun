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
  return capability.items
    .map((item) => ({
      item,
      score: scoreTextMatch(query, [item.titleZh, item.titleEn]),
    }))
    .filter(({ score }) => score > 0)
    .sort((left, right) => right.score - left.score)[0]?.item;
}

function resultSubtitle(capability: InteractiveCapability, query: string) {
  const category = INTERACTIVE_CAPABILITY_CATALOG.categories[capability.categoryId];
  const item = matchingItem(capability, query);
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
      .map((capability) => ({
        id: `capability:${capability.id}`,
        providerId: 'interactive-capabilities',
        group: capability.kind === 'setting' ? 'settings' as const : 'capabilities' as const,
        title: resultTitle(capability, request.query),
        subtitle: resultSubtitle(capability, request.query),
        context: matchingItem(capability, request.query)?.id
          ? `${capability.id}:${matchingItem(capability, request.query)?.id}`
          : capability.id,
        badge: capability.kind,
        score: scoreTextMatch(request.query, [
          capability.id,
          capability.titleZh,
          capability.titleEn,
          ...capability.searchTerms,
        ]),
        target: { kind: 'capability' as const, capabilityId: capability.id },
      }))
      .filter((item) => item.score > 0);

    return { items };
  },
};
