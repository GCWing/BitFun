import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const statusBarPopoversAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'status-bar-popover',
  parts: [
    { id: 'root' }, { id: 'hint' }, { id: 'inputWrap' }, { id: 'input' },
  ],
  facets: [
    { id: 'kind', attribute: 'data-openbitfun-popover', values: ['line', 'indent', 'encoding', 'language'] },
  ],
};
