import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const miniAppGalleryViewAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'miniapp-gallery-view',
  parts: [
    { id: 'root' }, { id: 'content' }, { id: 'categoryFilters' },
    { id: 'categoryFilter' },
  ],
};
