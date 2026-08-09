import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';

export const pagesAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'pages',
  parts: [{ id: 'root' }, { id: 'header' }, { id: 'content' }, { id: 'loading' }, { id: 'error' }, { id: 'empty' }, { id: 'list' }],
};
