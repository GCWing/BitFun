import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';

export const insightsAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'insights',
  parts: [{ id: 'root' }, { id: 'header' }, { id: 'error' }, { id: 'content' }, { id: 'loading' }, { id: 'empty' }],
  facets: [{ id: 'view', attribute: 'data-openbitfun-view', values: ['list', 'report'] }],
};
