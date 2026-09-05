import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const fileSystemAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'file-system',
  parts: [
    { id: 'searchResults' },
    { id: 'empty' },
    { id: 'header' },
    { id: 'list' },
    { id: 'match' },
  ],
  states: [
    { id: 'empty', selector: { kind: 'self', suffix: '[data-openbitfun-state~="empty"]' } },
  ],
};
