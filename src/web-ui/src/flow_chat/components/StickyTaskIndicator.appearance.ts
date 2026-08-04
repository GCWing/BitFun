import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const stickyTaskIndicatorAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'sticky-task-indicator',
  parts: [
    { id: 'root' }, { id: 'gradient' }, { id: 'content' }, { id: 'button' },
    { id: 'icon' }, { id: 'label' }, { id: 'arrow' },
  ],
  states: [{ id: 'visible', selector: { kind: 'self', suffix: '[data-bf-state~="visible"]' } }],
};
