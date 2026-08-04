import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const modelRoundItemAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'model-round-item',
  parts: [
    { id: 'root' },
    { id: 'retryHistory' },
    { id: 'retryToggle' },
    { id: 'retryAttempt' },
    { id: 'attemptLabel' },
    { id: 'diagnosticToggle' },
    { id: 'diagnosticDetails' },
    { id: 'diagnosticSection' },
    { id: 'subagent' },
    { id: 'toolItem' },
    { id: 'footer' },
    { id: 'meta' },
    { id: 'metaItem' },
    { id: 'action' },
  ],
  facets: [
    {
      id: 'status',
      attribute: 'data-bf-status',
      values: ['pending', 'queued', 'waiting', 'preparing', 'streaming', 'receiving', 'running', 'completed', 'error', 'cancelled', 'rejected', 'analyzing'],
    },
  ],
  states: [
    { id: 'streaming', selector: { kind: 'self', suffix: '[data-bf-state~="streaming"]' } },
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-bf-state~="expanded"]' } },
    { id: 'pending', selector: { kind: 'self', suffix: '[data-bf-state~="pending"]' } },
    { id: 'copied', selector: { kind: 'self', suffix: '[data-bf-state~="copied"]' } },
  ],
};
