import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const settingsNavAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'settings-nav',
  parts: [
    { id: 'root', propertyProfile: 'layout', visualRole: 'panel', continuityGroup: 'settings-workspace' },
    { id: 'header', visualRole: 'toolbar', continuityGroup: 'settings-workspace' },
    { id: 'search', propertyProfile: 'control', visualRole: 'control' },
    { id: 'searchEmpty', visualRole: 'content' },
    { id: 'searchResults', visualRole: 'content' },
    { id: 'searchResult', propertyProfile: 'control', visualRole: 'control' },
    { id: 'category', visualRole: 'content' },
    { id: 'categoryHeader', visualRole: 'toolbar' },
    { id: 'items', visualRole: 'content' },
    { id: 'item', propertyProfile: 'control', visualRole: 'control' },
    { id: 'highlight', propertyProfile: 'paint', visualRole: 'decoration' },
  ],
  states: [
    { id: 'active', selector: { kind: 'self', suffix: '[data-bf-state~="active"]' } },
    { id: 'selected', selector: { kind: 'self', suffix: '[data-bf-state~="selected"]' } },
  ],
};
