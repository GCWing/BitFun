import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const copyableTextPreviewAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'copyable-text-preview',
  parts: [
    { id: 'root' },
    { id: 'empty' },
    { id: 'tooltipContent' },
    { id: 'tooltipText' },
    { id: 'copyAction' },
  ],
  states: [
    { id: 'copied', selector: { kind: 'self', suffix: '[data-openbitfun-state~="copied"]' } },
  ],
};
