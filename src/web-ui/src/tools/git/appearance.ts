import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const gitToolAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'git-tool',
  parts: [
    { id: 'graphRoot' },
    { id: 'graphHeader' },
    { id: 'graphContent' },
    { id: 'graphCanvas' },
    { id: 'graphStatus' },
    { id: 'diffEditor' },
    { id: 'createBranchDialog' },
  ],
  states: [
    { id: 'loading', selector: { kind: 'self', suffix: '[data-bf-state~="loading"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-bf-state~="error"]' } },
    { id: 'empty', selector: { kind: 'self', suffix: '[data-bf-state~="empty"]' } },
  ],
};
