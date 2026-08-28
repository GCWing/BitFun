import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const imageViewerAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'image-viewer',
  parts: [
    { id: 'root' }, { id: 'info' },
    { id: 'container' }, { id: 'loading' }, { id: 'error' },
    { id: 'imageWrapper' }, { id: 'image' },
  ],
  states: [{ id: 'fullscreen', selector: { kind: 'self', suffix: '[data-bf-state~="fullscreen"]' } }],
};
