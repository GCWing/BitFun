import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const skillGroupPickerAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'skill-group-picker',
  componentAttribute: 'data-openbitfun-product-component',
  parts: [
    { id: 'root' }, { id: 'head' }, { id: 'sections' }, { id: 'section' },
    { id: 'group' }, { id: 'groupHeader' }, { id: 'groupActions' },
    { id: 'tokenGrid' }, { id: 'token' },
    { id: 'summary' }, { id: 'summaryGroup' }, { id: 'empty' },
  ],
  states: [
    { id: 'selected', selector: { kind: 'self', suffix: '[data-openbitfun-state~="selected"]' } },
  ],
};
