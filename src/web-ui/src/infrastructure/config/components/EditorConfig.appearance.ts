import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const editorConfigAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'editor-config',
  parts: [
    { id: 'root' }, { id: 'content' }, { id: 'actions' }, { id: 'saving' },
  ],
  states: [
    { id: 'loading', selector: { kind: 'self', suffix: '[data-bf-state~="loading"]' } },
    { id: 'saving', selector: { kind: 'self', suffix: '[data-bf-state~="saving"]' } },
  ],
};
