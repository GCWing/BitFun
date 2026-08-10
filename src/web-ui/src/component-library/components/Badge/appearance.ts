import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';
export const badgeAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'badge', parts: [{ id: 'root' }], facets: [{ id: 'variant', attribute: 'data-bf-variant', values: ['neutral', 'accent', 'purple', 'success', 'warning', 'error', 'info'] }],
};
