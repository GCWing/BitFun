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
    { id: 'canvasAttachments' },
    { id: 'footer' },
    { id: 'meta' },
    { id: 'metaItem' },
    { id: 'action' },
  ],
  facets: [
    {
      id: 'status',
      attribute: 'data-openbitfun-status',
      values: ['pending', 'queued', 'waiting', 'preparing', 'streaming', 'receiving', 'running', 'completed', 'error', 'cancelled', 'rejected', 'analyzing'],
    },
  ],
  states: [
    { id: 'streaming', selector: { kind: 'self', suffix: '[data-openbitfun-state~="streaming"]' } },
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-openbitfun-state~="expanded"]' } },
    { id: 'pending', selector: { kind: 'self', suffix: '[data-openbitfun-state~="pending"]' } },
    { id: 'copied', selector: { kind: 'self', suffix: '[data-openbitfun-state~="copied"]' } },
  ],
};
