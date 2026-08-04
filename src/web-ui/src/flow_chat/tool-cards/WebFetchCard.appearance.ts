import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const webFetchCardAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'web-fetch-card',
  parts: [
    { id: 'root' }, { id: 'headerUrl' }, { id: 'content' }, { id: 'expanded' },
    { id: 'meta' }, { id: 'detailsRow' }, { id: 'details' }, { id: 'detail' },
    { id: 'actions' }, { id: 'result' },
  ],
  states: [
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-bf-state~="expanded"]' } },
    { id: 'failed', selector: { kind: 'self', suffix: '[data-bf-state~="failed"]' } },
  ],
};
