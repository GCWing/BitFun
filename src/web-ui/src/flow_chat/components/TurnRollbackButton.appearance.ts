import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const turnRollbackButtonAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'turn-rollback-button',
  parts: [{ id: 'root' }],
  facets: [{ id: 'mode', attribute: 'data-bf-mode', values: ['current', 'action'] }],
  states: [{ id: 'loading', selector: { kind: 'self', suffix: '[data-bf-state~="loading"]' } }],
};
