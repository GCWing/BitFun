import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const viewImageToolCardAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'view-image-tool-card',
  parts: [
    { id: 'root' }, { id: 'content' }, { id: 'error' }, { id: 'preview' },
    { id: 'image' }, { id: 'lightbox' },
  ],
  states: [
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-bf-state~="expanded"]' } },
    { id: 'failed', selector: { kind: 'self', suffix: '[data-bf-state~="failed"]' } },
  ],
};
