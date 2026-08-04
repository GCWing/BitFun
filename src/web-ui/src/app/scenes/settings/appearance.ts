import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';

export const settingsAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'settings',
  parts: [
    { id: 'root', propertyProfile: 'layout', visualRole: 'workspace', continuityGroup: 'settings-workspace' },
    { id: 'content', propertyProfile: 'layout', visualRole: 'continuous-surface', continuityGroup: 'settings-workspace' },
    { id: 'loading', propertyProfile: 'overlay', visualRole: 'content' },
  ],
  facets: [{
    id: 'tab',
    attribute: 'data-bf-tab',
    values: [
      'basics',
      'appearance',
      'models',
      'archived-sessions',
      'session-personalization',
      'session-permissions',
      'quick-actions',
      'review',
      'memories',
      'mcp-tools',
      'external-sources',
      'acp-agents',
      'editor',
      'keyboard',
    ],
  }],
};
