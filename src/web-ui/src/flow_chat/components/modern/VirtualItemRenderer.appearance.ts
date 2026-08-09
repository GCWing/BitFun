import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const virtualItemAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'virtual-item',
  parts: [{ id: 'root' }, { id: 'imageAnalyzing' }, { id: 'placeholder' }],
  states: [
    { id: 'searchMatch', selector: { kind: 'self', suffix: '[data-bf-state~="searchMatch"]' } },
    { id: 'searchCurrent', selector: { kind: 'self', suffix: '[data-bf-state~="searchCurrent"]' } },
  ],
};
