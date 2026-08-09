import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const pendingQueuePanelAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'pending-queue-panel',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'title' }, { id: 'list' },
    { id: 'item' }, { id: 'content' }, { id: 'editor' }, { id: 'preview' },
    { id: 'status' }, { id: 'actions' }, { id: 'action' },
  ],
  states: [
    { id: 'editing', selector: { kind: 'self', suffix: '[data-bf-state~="editing"]' } },
    { id: 'sending', selector: { kind: 'self', suffix: '[data-bf-state~="sending"]' } },
    { id: 'failed', selector: { kind: 'self', suffix: '[data-bf-state~="failed"]' } },
  ],
};
