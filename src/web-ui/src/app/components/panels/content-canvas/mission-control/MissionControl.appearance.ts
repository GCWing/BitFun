import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const missionControlAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'mission-control',
  parts: [
    { id: 'root' }, { id: 'content' }, { id: 'header' }, { id: 'headerActions' },
    { id: 'filters' }, { id: 'search' }, { id: 'groupFilters' }, { id: 'filter' },
    { id: 'grid' }, { id: 'empty' }, { id: 'footer' }, { id: 'separator' },
  ],
  facets: [{ id: 'group', attribute: 'data-openbitfun-group', values: ['primary', 'secondary', 'tertiary'] }],
  states: [
    { id: 'open', selector: { kind: 'self', suffix: '[data-openbitfun-state~="open"]' } },
    { id: 'active', selector: { kind: 'self', suffix: '[data-openbitfun-state~="active"]' } },
  ],
};
