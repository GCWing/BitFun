import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const userMessageEditComposerAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'user-message-edit-composer',
  componentAttribute: 'data-bf-product-component',
  parts: [{ id: 'root' }, { id: 'input' }, { id: 'actions' }, { id: 'action' }, { id: 'spinner' }],
  facets: [
    { id: 'mode', attribute: 'data-bf-mode', values: ['rich', 'plain'] },
    { id: 'action', attribute: 'data-bf-action', values: ['cancel', 'submit'] },
  ],
  states: [
    { id: 'submitting', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-bf-state~="submitting"]' } },
  ],
};
