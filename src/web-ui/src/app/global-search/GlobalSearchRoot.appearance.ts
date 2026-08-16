import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const globalSearchAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'global-search',
  parts: [
    { id: 'root' },
    { id: 'query' },
    { id: 'scopeBar' },
    { id: 'results' },
    { id: 'group' },
    { id: 'result' },
    { id: 'footer' },
  ],
  states: [
    { id: 'selected', selector: { kind: 'self', suffix: '[data-bf-state~="selected"]' } },
  ],
};
