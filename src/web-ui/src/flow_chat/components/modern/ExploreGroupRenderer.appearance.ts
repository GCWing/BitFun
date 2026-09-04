import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const exploreGroupAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'explore-group',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'summary' },
    { id: 'contentWrapper' }, { id: 'content' }, { id: 'item' },
  ],
  states: [{ id: 'expanded', selector: { kind: 'self', suffix: '[data-openbitfun-state~="expanded"]' } }],
};
