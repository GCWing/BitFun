import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const workspaceListSectionAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'workspace-list-section',
  parts: [{ id: 'root' }, { id: 'empty' }, { id: 'item' }, { id: 'dropLine' }],
  states: [
    { id: 'dragging', selector: { kind: 'self', suffix: '[data-bf-state~="dragging"]' } },
    { id: 'selected', selector: { kind: 'self', suffix: '[data-bf-state~="selected"]' } },
  ],
};
