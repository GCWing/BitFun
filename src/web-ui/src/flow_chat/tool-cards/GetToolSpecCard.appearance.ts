import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const getToolSpecCardAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'get-tool-spec-card',
  parts: [
    { id: 'root' }, { id: 'content' }, { id: 'expanded' },
    { id: 'section' }, { id: 'label' }, { id: 'description' },
  ],
  states: [
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-bf-state~="expanded"]' } },
    { id: 'failed', selector: { kind: 'self', suffix: '[data-bf-state~="failed"]' } },
  ],
};
