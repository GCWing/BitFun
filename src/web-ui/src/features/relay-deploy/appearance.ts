import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const relayDeployAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'relay-deploy',
  parts: [
    { id: 'root' },
    { id: 'steps' },
    { id: 'step' },
    { id: 'error' },
  ],
  states: [
    { id: 'active', selector: { kind: 'self', suffix: '[data-openbitfun-state~="active"]' } },
    { id: 'completed', selector: { kind: 'self', suffix: '[data-openbitfun-state~="completed"]' } },
  ],
};
