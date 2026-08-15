import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const fileMentionPickerAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'file-mention-picker',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'back' }, { id: 'content' },
    { id: 'loading' }, { id: 'empty' }, { id: 'list' }, { id: 'item' },
    { id: 'itemName' }, { id: 'itemDetail' }, { id: 'footer' },
  ],
  states: [
    { id: 'selected', selector: { kind: 'self', suffix: '[data-bf-state~="selected"]' } },
    { id: 'loading', selector: { kind: 'self', suffix: '[data-bf-state~="loading"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-bf-state~="error"]' } },
  ],
};
