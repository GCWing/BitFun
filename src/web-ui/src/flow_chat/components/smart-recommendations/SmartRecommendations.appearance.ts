import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const smartRecommendationsAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'smart-recommendations',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'title' }, { id: 'close' },
    { id: 'actions' },
  ],
  states: [{ id: 'loading', selector: { kind: 'self', suffix: '[data-bf-state~="loading"]' } }],
};
