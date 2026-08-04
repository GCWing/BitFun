import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';

export const avatarAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'avatar',
  parts: [
    { id: 'root' }, { id: 'image' }, { id: 'icon' }, { id: 'text' },
    { id: 'group' }, { id: 'overflow' },
  ],
  facets: [
    { id: 'size', attribute: 'data-bf-size', values: ['small', 'medium', 'large', 'custom'] },
    { id: 'shape', attribute: 'data-bf-shape', values: ['circle', 'square'] },
  ],
};
