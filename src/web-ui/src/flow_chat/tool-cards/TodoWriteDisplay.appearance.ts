import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const todoWriteDisplayAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'todo-write-display',
  parts: [
    { id: 'root' }, { id: 'compactIcon' }, { id: 'count' }, { id: 'progress' },
    { id: 'headerContent' }, { id: 'expanded' }, { id: 'list' },
    { id: 'item' }, { id: 'itemContent' },
  ],
  facets: [{ id: 'mode', attribute: 'data-bf-mode', values: ['compact', 'standard'] }],
  states: [
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-bf-state~="expanded"]' } },
    { id: 'loading', selector: { kind: 'self', suffix: '[data-bf-state~="loading"]' } },
    { id: 'completed', selector: { kind: 'self', suffix: '[data-bf-state~="completed"]' } },
  ],
};
