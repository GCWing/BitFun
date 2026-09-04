import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const diffEditorAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'diff-editor',
  parts: [
    { id: 'root' }, { id: 'toolbar' }, { id: 'toolbarInfo' },
    { id: 'toolbarActions' }, { id: 'content' }, { id: 'loading' }, { id: 'error' },
  ],
  states: [
    { id: 'loading', selector: { kind: 'self', suffix: '[data-openbitfun-state~="loading"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-openbitfun-state~="error"]' } },
  ],
};
