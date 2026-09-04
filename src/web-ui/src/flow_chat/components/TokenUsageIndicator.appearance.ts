import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const tokenUsageIndicatorAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'token-usage-indicator',
  parts: [{ id: 'root' }, { id: 'track' }, { id: 'fill' }, { id: 'hover' }, { id: 'percentage' }],
  facets: [{ id: 'level', attribute: 'data-openbitfun-level', values: ['normal', 'warning', 'critical'] }],
};
