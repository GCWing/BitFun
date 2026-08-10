import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const snapshotRollbackButtonAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'snapshot-rollback-button',
  parts: [{ id: 'root' }, { id: 'label' }],
  facets: [{ id: 'outcome', attribute: 'data-bf-outcome', values: ['idle', 'current', 'success', 'error'] }],
  states: [{ id: 'loading', selector: { kind: 'self', suffix: '[data-bf-state~="loading"]' } }],
};
