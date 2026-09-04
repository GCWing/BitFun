import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const canvasTabOverflowAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'canvas-tab-overflow',
  parts: [
    { id: 'root' }, { id: 'trigger' }, { id: 'badge' }, { id: 'itemTitle' },
  ],
  states: [
    { id: 'open', selector: { kind: 'self', suffix: '[data-openbitfun-state~="open"]' } },
    { id: 'active', selector: { kind: 'self', suffix: '[data-openbitfun-state~="active"]' } },
    { id: 'dirty', selector: { kind: 'self', suffix: '[data-openbitfun-state~="dirty"]' } },
    { id: 'deleted', selector: { kind: 'self', suffix: '[data-openbitfun-state~="deleted"]' } },
  ],
};
