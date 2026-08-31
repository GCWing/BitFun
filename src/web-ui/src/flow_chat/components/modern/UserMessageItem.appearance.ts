import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const userMessageItemAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'user-message-item',
  parts: [
    { id: 'root' }, { id: 'main' }, { id: 'content' },
    { id: 'steeringTag' }, { id: 'actions' }, { id: 'images' }, { id: 'image' },
    { id: 'lightbox' }, { id: 'loading' },
  ],
  states: [
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-bf-state~="expanded"]' } },
    { id: 'failed', selector: { kind: 'self', suffix: '[data-bf-state~="failed"]' } },
    { id: 'loading', selector: { kind: 'self', suffix: '[data-bf-state~="loading"]' } },
  ],
};
