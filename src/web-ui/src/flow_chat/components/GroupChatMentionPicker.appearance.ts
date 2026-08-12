import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const groupChatMentionPickerAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'group-chat-mention-picker',
  parts: [
    { id: 'root' },
    { id: 'items' },
    { id: 'item' },
    { id: 'itemName' },
    { id: 'itemDetail' },
  ],
  states: [
    { id: 'selected', selector: { kind: 'self', suffix: '[data-bf-state~="selected"]' } },
  ],
};
