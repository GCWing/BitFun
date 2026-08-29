import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const markdownEditorAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'markdown-editor',
  parts: [
    { id: 'root' }, { id: 'loading' }, { id: 'error' },
    { id: 'toolbar' }, { id: 'actions' }, { id: 'body' },
  ],
  facets: [{ id: 'view', attribute: 'data-bf-view', values: ['preview', 'markdown', 'source'] }],
  states: [
    { id: 'loading', selector: { kind: 'self', suffix: '[data-bf-state~="loading"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-bf-state~="error"]' } },
  ],
};
