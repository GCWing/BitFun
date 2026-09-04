import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const miniAppGalleryViewAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'miniapp-gallery-view',
  parts: [
    { id: 'root' }, { id: 'content' }, { id: 'categoryFilters' },
    { id: 'categoryFilter' }, { id: 'tools' }, { id: 'item' },
    { id: 'showcase' }, { id: 'summary' }, { id: 'title' },
    { id: 'meta' }, { id: 'status' }, { id: 'actions' },
    { id: 'error' }, { id: 'detail' },
  ],
};
