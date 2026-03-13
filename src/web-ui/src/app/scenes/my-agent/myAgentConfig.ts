import type { PanelType } from '@/app/types';

export type MyAgentView = 'profile' | 'agents' | 'skills' | 'insights';

export interface MyAgentNavItem {
  id: MyAgentView;
  panelTab: PanelType;
  labelKey: string;
}

export interface MyAgentNavCategory {
  id: string;
  nameKey: string;
  items: MyAgentNavItem[];
}

export const MY_AGENT_NAV_CATEGORIES: MyAgentNavCategory[] = [
  {
    id: 'identity',
    nameKey: 'nav.myAgent.categories.identity',
    items: [
      { id: 'profile', panelTab: 'profile', labelKey: 'nav.items.persona' },
    ],
  },
  {
    id: 'collaboration',
    nameKey: 'nav.myAgent.categories.collaboration',
    items: [
      { id: 'agents', panelTab: 'agents', labelKey: 'nav.items.agents' },
      { id: 'skills', panelTab: 'skills', labelKey: 'nav.items.skills' },
    ],
  },
  {
    id: 'analytics',
    nameKey: 'nav.myAgent.categories.analytics',
    items: [
      { id: 'insights', panelTab: 'sessions', labelKey: 'nav.items.insights' },
    ],
  },
];

export const DEFAULT_MY_AGENT_VIEW: MyAgentView = 'profile';
