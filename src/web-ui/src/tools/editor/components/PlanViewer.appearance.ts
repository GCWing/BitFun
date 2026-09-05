import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const planViewerAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'plan-viewer',
  parts: [
    { id: 'root' }, { id: 'loading' }, { id: 'error' }, { id: 'header' },
    { id: 'headerMain' }, { id: 'headerActions' }, { id: 'content' }, { id: 'markdown' },
    { id: 'editorPanel' }, { id: 'editor' }, { id: 'todos' }, { id: 'todo' },
  ],
  states: [
    { id: 'loading', selector: { kind: 'self', suffix: '[data-openbitfun-state~="loading"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-openbitfun-state~="error"]' } },
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-openbitfun-state~="expanded"]' } },
  ],
};
