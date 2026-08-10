import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';

export const codeEditorAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'code-editor',
  parts: [{ id: 'root' }, { id: 'wrapper' }, { id: 'fullscreenButton' }, { id: 'editor' }],
  states: [
    { id: 'fullscreen', selector: { kind: 'self', suffix: '[data-bf-state~="fullscreen"]' } },
    { id: 'readOnly', selector: { kind: 'self', suffix: '[data-bf-state~="readOnly"]' } },
  ],
};
