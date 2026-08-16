import { SETTINGS_CATEGORIES } from '@/app/scenes/settings/settingsConfig';
import { scoreTextMatch } from '../searchMatching';
import type { GlobalSearchProvider } from '../types';

export const settingsSearchProvider: GlobalSearchProvider = {
  id: 'settings',
  groups: ['settings'],
  search: (request) => {
    if (request.scope !== 'all' || !request.query) return { items: [] };

    const items = SETTINGS_CATEGORIES.flatMap((category) => category.tabs.map((tab) => {
      const title = request.tSettings(tab.labelKey);
      const subtitle = tab.descriptionKey
        ? request.tSettings(tab.descriptionKey)
        : request.tSettings(category.nameKey);
      const score = scoreTextMatch(request.query, [
        title,
        subtitle,
        request.tSettings(category.nameKey),
        ...(tab.keywords ?? []),
      ]);
      return {
        id: `settings:${tab.id}`,
        providerId: 'settings',
        group: 'settings' as const,
        title,
        subtitle,
        score,
        target: { kind: 'settings' as const, tab: tab.id },
      };
    })).filter((item) => item.score > 0);

    return { items };
  },
};
