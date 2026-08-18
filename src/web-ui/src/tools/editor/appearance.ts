import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const editorToolAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'editor-tool',
  parts: [
    { id: 'root' },
    { id: 'content' },
    { id: 'loading' },
    { id: 'error' },
    { id: 'saving' },
    { id: 'readOnlyCodeBlock' },
    { id: 'meditorPreview' },
    { id: 'meditorFrontmatter' },
    { id: 'meditorEditArea' },
  ],
  states: [
    { id: 'loading', selector: { kind: 'self', suffix: '[data-bf-state~="loading"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-bf-state~="error"]' } },
    { id: 'largeFile', selector: { kind: 'self', suffix: '[data-bf-state~="large-file"]' } },
    { id: 'fullscreen', selector: { kind: 'self', suffix: '[data-bf-state~="fullscreen"]' } },
  ],
};
