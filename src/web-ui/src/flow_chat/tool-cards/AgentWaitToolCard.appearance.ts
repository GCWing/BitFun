import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const agentWaitToolCardAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'agent-wait-tool-card',
  parts: [
    { id: 'root' },
  ],
  states: [
    { id: 'failed', selector: { kind: 'self', suffix: '[data-bf-state~="failed"]' } },
  ],
};
