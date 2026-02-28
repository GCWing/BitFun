/**
 * skillsConfig — static shape of skills scene navigation views.
 *
 * Shared by SkillsNav (left sidebar) and SkillsScene (content renderer).
 */

import type { SkillsView } from './skillsSceneStore';

export interface SkillsNavItem {
  id: SkillsView;
  labelKey: string;
}

export interface SkillsNavCategory {
  id: string;
  nameKey: string;
  items: SkillsNavItem[];
}

export const SKILLS_NAV_CATEGORIES: SkillsNavCategory[] = [
  {
    id: 'discover',
    nameKey: 'nav.categories.discover',
    items: [
      { id: 'market', labelKey: 'nav.items.market' },
    ],
  },
  {
    id: 'installed',
    nameKey: 'nav.categories.installed',
    items: [
      { id: 'installed-all',     labelKey: 'nav.items.installedAll'     },
      { id: 'installed-user',    labelKey: 'nav.items.installedUser'    },
      { id: 'installed-project', labelKey: 'nav.items.installedProject' },
    ],
  },
];

export const DEFAULT_SKILLS_VIEW: SkillsView = 'market';
