import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const pendingQueuePanelAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'pending-queue-panel',
  componentAttribute: 'data-bf-product-component',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'title' }, { id: 'list' },
    { id: 'item' }, { id: 'content' }, { id: 'preview' },
    { id: 'status' }, { id: 'actions' }, { id: 'action' },
  ],
  states: [
    { id: 'sending', selector: { kind: 'self', suffix: '[data-bf-state~="sending"]' } },
    { id: 'failed', selector: { kind: 'self', suffix: '[data-bf-state~="failed"]' } },
  ],
};
