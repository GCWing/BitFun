import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const markdownEditorAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'markdown-editor',
  parts: [
    { id: 'root' }, { id: 'loading' }, { id: 'error' },
    { id: 'toolbar' }, { id: 'actions' }, { id: 'body' },
  ],
  facets: [{ id: 'view', attribute: 'data-openbitfun-view', values: ['preview', 'markdown', 'source'] }],
  states: [
    { id: 'loading', selector: { kind: 'self', suffix: '[data-openbitfun-state~="loading"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-openbitfun-state~="error"]' } },
  ],
};
