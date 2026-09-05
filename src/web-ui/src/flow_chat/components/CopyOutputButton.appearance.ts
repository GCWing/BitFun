import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const copyOutputButtonAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'copy-output-button',
  parts: [{ id: 'root' }],
  states: [{ id: 'copied', selector: { kind: 'self', suffix: '[data-openbitfun-state~="copied"]' } }],
};
