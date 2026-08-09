import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const toolCardAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'tool-card',
  parts: [
    { id: 'root' }, { id: 'surface' }, { id: 'header' }, { id: 'icon' },
    { id: 'action' }, { id: 'content' }, { id: 'extra' }, { id: 'status' },
    { id: 'expanded' }, { id: 'error' },
  ],
  facets: [{
    id: 'status',
    attribute: 'data-bf-status',
    values: [
      'pending', 'queued', 'waiting', 'preparing', 'streaming', 'receiving',
      'running', 'completed', 'error', 'cancelled', 'rejected', 'analyzing',
      'pending_confirmation', 'confirmed',
    ],
  }],
  states: [
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-bf-state~="expanded"]' } },
    { id: 'failed', selector: { kind: 'self', suffix: '[data-bf-state~="failed"]' } },
    { id: 'loading', selector: { kind: 'self', suffix: '[data-bf-state~="loading"]' } },
    { id: 'confirmation', selector: { kind: 'self', suffix: '[data-bf-state~="confirmation"]' } },
  ],
};
