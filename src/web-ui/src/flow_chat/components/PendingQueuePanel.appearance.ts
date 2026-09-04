import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const pendingQueuePanelAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'pending-queue-panel',
  componentAttribute: 'data-openbitfun-product-component',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'title' }, { id: 'list' },
    { id: 'item' }, { id: 'content' }, { id: 'preview' },
    { id: 'status' }, { id: 'actions' }, { id: 'action' },
  ],
  states: [
    { id: 'sending', selector: { kind: 'self', suffix: '[data-openbitfun-state~="sending"]' } },
    { id: 'failed', selector: { kind: 'self', suffix: '[data-openbitfun-state~="failed"]' } },
  ],
};
