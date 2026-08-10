import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';

export const emptyAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'empty',
  parts: [{ id: 'root' }, { id: 'image' }, { id: 'description' }, { id: 'footer' }],
};
