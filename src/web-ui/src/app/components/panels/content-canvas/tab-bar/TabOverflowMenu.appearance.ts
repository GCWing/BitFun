import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const canvasTabOverflowAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'canvas-tab-overflow',
  parts: [
    { id: 'root' }, { id: 'trigger' }, { id: 'badge' }, { id: 'itemTitle' },
  ],
  states: [
    { id: 'open', selector: { kind: 'self', suffix: '[data-bf-state~="open"]' } },
    { id: 'active', selector: { kind: 'self', suffix: '[data-bf-state~="active"]' } },
    { id: 'dirty', selector: { kind: 'self', suffix: '[data-bf-state~="dirty"]' } },
    { id: 'deleted', selector: { kind: 'self', suffix: '[data-bf-state~="deleted"]' } },
  ],
};
