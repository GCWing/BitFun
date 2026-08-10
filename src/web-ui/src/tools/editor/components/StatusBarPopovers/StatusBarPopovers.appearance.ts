import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const statusBarPopoversAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'status-bar-popover',
  parts: [
    { id: 'root' }, { id: 'hint' }, { id: 'inputWrap' }, { id: 'input' },
    { id: 'list' }, { id: 'item' }, { id: 'itemIcon' },
  ],
  facets: [
    { id: 'kind', attribute: 'data-bf-popover', values: ['line', 'indent', 'encoding', 'language'] },
  ],
  states: [
    { id: 'selected', selector: { kind: 'self', suffix: '[data-bf-state~="selected"]' } },
  ],
};
