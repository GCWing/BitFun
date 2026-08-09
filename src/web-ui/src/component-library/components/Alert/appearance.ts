import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';
export const alertAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'alert', parts: [{ id: 'root' }, { id: 'icon' }, { id: 'content' }, { id: 'title' }, { id: 'message' }, { id: 'description' }, { id: 'close' }],
  facets: [{ id: 'variant', attribute: 'data-bf-variant', values: ['success', 'error', 'warning', 'info'] }],
  states: [{ id: 'hover', selector: { kind: 'self', suffix: ':hover' } }, { id: 'focusVisible', selector: { kind: 'self', suffix: ':focus-visible' } }],
};
