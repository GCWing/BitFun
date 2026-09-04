import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const gitGraphViewAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'git-graph-view',
  parts: [{ id: 'root' }],
  states: [{ id: 'empty', selector: { kind: 'self', suffix: '[data-openbitfun-state~="empty"]' } }],
};
