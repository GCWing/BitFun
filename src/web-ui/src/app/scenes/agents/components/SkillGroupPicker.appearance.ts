import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const skillGroupPickerAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'skill-group-picker',
  parts: [
    { id: 'root' }, { id: 'head' }, { id: 'sections' }, { id: 'section' },
    { id: 'group' }, { id: 'groupHeader' }, { id: 'groupActions' },
    { id: 'tokenGrid' }, { id: 'token' }, { id: 'manager' },
    { id: 'managerEditor' }, { id: 'managerList' }, { id: 'managerGroup' },
    { id: 'summary' }, { id: 'summaryGroup' }, { id: 'empty' },
  ],
  states: [
    { id: 'selected', selector: { kind: 'self', suffix: '[data-openbitfun-state~="selected"]' } },
  ],
};
