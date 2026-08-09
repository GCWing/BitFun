import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';

export const dotMatrixLoaderAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'dot-matrix-loader',
  parts: [{ id: 'root' }, { id: 'dot' }],
  facets: [{ id: 'size', attribute: 'data-bf-size', values: ['small', 'medium', 'large'] }],
};
