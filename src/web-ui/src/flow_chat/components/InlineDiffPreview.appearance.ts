import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const inlineDiffPreviewAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'inline-diff-preview',
  parts: [
    { id: 'root' }, { id: 'placeholder' }, { id: 'notice' }, { id: 'content' },
    { id: 'line' }, { id: 'gutter' }, { id: 'prefix' }, { id: 'lineContent' },
  ],
  states: [
    { id: 'empty', selector: { kind: 'self', suffix: '[data-openbitfun-state~="empty"]' } },
  ],
};
