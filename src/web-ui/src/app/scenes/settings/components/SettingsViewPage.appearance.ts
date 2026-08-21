import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const settingsViewPageAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'settings-view-page',
  parts: [
    { id: 'root', propertyProfile: 'layout', visualRole: 'content' },
    { id: 'loading', propertyProfile: 'overlay', visualRole: 'content' },
    { id: 'loadingLine', propertyProfile: 'paint', visualRole: 'decoration' },
    { id: 'loadingBlock', propertyProfile: 'paint', visualRole: 'decoration' },
  ],
  facets: [{
    id: 'view',
    attribute: 'data-bf-view',
    values: ['voice', 'shortcuts', 'editor', 'terminal', 'quick-actions', 'hooks'],
  }],
};
