import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';

export const configPageAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'config-page',
  parts: [{ id: 'loading' }, { id: 'message' }],
};
