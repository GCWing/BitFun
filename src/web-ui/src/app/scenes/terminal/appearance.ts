import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';

export const terminalAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'terminal',
  parts: [{ id: 'root' }, { id: 'empty' }],
  states: [{ id: 'inactive', selector: { kind: 'self', suffix: '[data-openbitfun-state~="inactive"]' } }],
};
