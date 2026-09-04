import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';
export const shellAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'shell', parts: [{ id: 'root' }, { id: 'loading' }], states: [{ id: 'active', selector: { kind: 'self', suffix: '[data-openbitfun-state~="active"]' } }],
};
