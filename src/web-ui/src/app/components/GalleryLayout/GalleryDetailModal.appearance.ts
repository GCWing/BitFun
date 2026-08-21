import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const galleryDetailModalAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'gallery-detail-modal',
  parts: [
    { id: 'root' },
    { id: 'hero' },
    { id: 'icon' },
    { id: 'summary' },
    { id: 'title' },
    { id: 'heroActions' },
    { id: 'badges' },
    { id: 'description' },
    { id: 'meta' },
    { id: 'content' },
    { id: 'actions' },
  ],
  states: [
    {
      id: 'heroTitle',
      selector: { kind: 'self', suffix: '[data-bf-state~="heroTitle"]' },
    },
    {
      id: 'stableHeight',
      selector: { kind: 'self', suffix: '[data-bf-state~="stableHeight"]' },
    },
  ],
};
