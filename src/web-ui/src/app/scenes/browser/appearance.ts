import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';

export const browserAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'browser',
  parts: [{ id: 'root' }, { id: 'toolbar' }, { id: 'error' }, { id: 'content' }],
  states: [{ id: 'loading', selector: { kind: 'self', suffix: '[data-openbitfun-state~="loading"]' } }],
};
