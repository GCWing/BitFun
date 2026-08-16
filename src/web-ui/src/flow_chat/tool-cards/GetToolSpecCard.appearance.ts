import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const getToolSpecCardAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'get-tool-spec-card',
  parts: [
    { id: 'root' },
  ],
  states: [
    { id: 'failed', selector: { kind: 'self', suffix: '[data-bf-state~="failed"]' } },
  ],
};
