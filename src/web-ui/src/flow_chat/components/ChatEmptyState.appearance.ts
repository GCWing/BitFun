import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const chatEmptyStateAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'chat-empty-state',
  parts: [
    { id: 'root' }, { id: 'container' }, { id: 'greeting' },
    { id: 'divider' }, { id: 'prompt' }, { id: 'noWorkspace' },
  ],
  states: [{ id: 'loading', selector: { kind: 'self', suffix: '[data-openbitfun-state~="loading"]' } }],
};
