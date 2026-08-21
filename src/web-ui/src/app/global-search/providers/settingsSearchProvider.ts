import { SETTINGS_CATEGORIES } from '@/app/scenes/settings/settingsRegistry';
import { scoreTextMatch } from '../searchMatching';
import type { GlobalSearchProvider } from '../types';

export const settingsSearchProvider: GlobalSearchProvider = {
  id: 'settings',
  groups: ['settings'],
  search: (request) => {
    if (request.scope !== 'all' || !request.query) return { items: [] };

    const items = SETTINGS_CATEGORIES.flatMap((category) => category.pages.flatMap((page) => {
      const categoryTitle = request.tSettings(category.labelKey);
      const pageTitle = request.tSettings(page.labelKey);
      const description = request.tSettings(page.descriptionKey);
      const baseFields = [pageTitle, description, categoryTitle, ...page.keywords];

      if (!page.views?.length) {
        return [{
          id: `settings:${page.id}`,
          providerId: 'settings',
          group: 'settings' as const,
          title: pageTitle,
          subtitle: description,
          score: scoreTextMatch(request.query, baseFields),
          target: { kind: 'settings' as const, destination: { pageId: page.id } },
        }];
      }

      return page.views.map((view) => {
        const viewTitle = request.tSettings(view.labelKey);
        return {
          id: `settings:${page.id}:${view.id}`,
          providerId: 'settings',
          group: 'settings' as const,
          title: viewTitle,
          subtitle: `${pageTitle} · ${description}`,
          score: scoreTextMatch(request.query, [
            viewTitle,
            view.id,
            ...view.keywords,
            ...baseFields,
          ]),
          target: {
            kind: 'settings' as const,
            destination: { pageId: page.id, viewId: view.id },
          },
        };
      });
    })).filter((item) => item.score > 0);

    return { items };
  },
};
