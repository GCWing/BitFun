import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const codePreviewAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'code-preview',
  parts: [
    { id: 'root' }, { id: 'placeholder' }, { id: 'content' }, { id: 'plain' },
    { id: 'line' }, { id: 'lineNumber' }, { id: 'lineContent' }, { id: 'cursor' },
  ],
  states: [
    { id: 'empty', selector: { kind: 'self', suffix: '[data-bf-state~="empty"]' } },
    { id: 'streaming', selector: { kind: 'self', suffix: '[data-bf-state~="streaming"]' } },
    { id: 'highlighted', selector: { kind: 'self', suffix: '[data-bf-state~="highlighted"]' } },
  ],
};
