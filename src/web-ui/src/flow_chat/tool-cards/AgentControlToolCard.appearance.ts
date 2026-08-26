import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const agentControlToolCardAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'agent-control-tool-card',
  parts: [
    { id: 'root' },
    { id: 'header' },
    { id: 'agentPill' },
    { id: 'avatar' },
    { id: 'name' },
    { id: 'type' },
    { id: 'status' },
    { id: 'expandIndicator' },
    { id: 'prompt' },
  ],
  facets: [{
    id: 'status',
    attribute: 'data-bf-status',
    values: ['running', 'finishing', 'waiting', 'completed', 'cancelled', 'error', 'idle'],
  }],
  states: [
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-bf-state~="expanded"]' } },
    { id: 'streaming', selector: { kind: 'self', suffix: '[data-bf-state~="streaming"]' } },
    { id: 'openable', selector: { kind: 'self', suffix: '[data-bf-state~="openable"]' } },
  ],
};
