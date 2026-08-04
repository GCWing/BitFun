import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const defaultToolCardAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'default-tool-card',
  parts: [
    { id: 'root' }, { id: 'expanded' }, { id: 'meta' }, { id: 'metaLabel' },
    { id: 'metaDescription' }, { id: 'section' }, { id: 'sectionLabel' },
    { id: 'code' }, { id: 'error' },
  ],
  states: [
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-bf-state~="expanded"]' } },
    { id: 'confirmation', selector: { kind: 'self', suffix: '[data-bf-state~="confirmation"]' } },
    { id: 'failed', selector: { kind: 'self', suffix: '[data-bf-state~="failed"]' } },
  ],
};
