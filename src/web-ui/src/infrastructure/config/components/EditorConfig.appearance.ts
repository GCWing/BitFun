import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const editorConfigAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'editor-config',
  parts: [
    { id: 'root' }, { id: 'content' }, { id: 'actions' },
  ],
  states: [
    { id: 'loading', selector: { kind: 'self', suffix: '[data-bf-state~="loading"]' } },
  ],
};
