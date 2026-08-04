import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const userMessageAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'user-message',
  parts: [
    { id: 'root' }, { id: 'content' }, { id: 'inlineContent' },
    { id: 'footer' }, { id: 'timestamp' }, { id: 'snapshotAction' },
  ],
  states: [
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-bf-state~="expanded"]' } },
    { id: 'collapsed', selector: { kind: 'self', suffix: '[data-bf-state~="collapsed"]' } },
  ],
};
