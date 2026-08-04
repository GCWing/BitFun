import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const keyboardShortcutsAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'keyboard-shortcuts',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'content' }, { id: 'toolbar' },
    { id: 'search' }, { id: 'actions' }, { id: 'error' }, { id: 'list' },
    { id: 'item' }, { id: 'label' }, { id: 'key' }, { id: 'keyBadge' },
    { id: 'revert' }, { id: 'empty' },
  ],
  states: [
    { id: 'recording', selector: { kind: 'self', suffix: '[data-bf-state~="recording"]' } },
    { id: 'conflict', selector: { kind: 'self', suffix: '[data-bf-state~="conflict"]' } },
    { id: 'modified', selector: { kind: 'self', suffix: '[data-bf-state~="modified"]' } },
    { id: 'readonly', selector: { kind: 'self', suffix: '[data-bf-state~="readonly"]' } },
  ],
};
