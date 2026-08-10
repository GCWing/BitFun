import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';

export const cubeLoadingAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'cube-loading',
  parts: [{ id: 'root' }, { id: 'text' }],
  facets: [{ id: 'size', attribute: 'data-bf-size', values: ['small', 'medium', 'large'] }],
};
