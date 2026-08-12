import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const groupChatsSectionAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'group-chats-section',
  parts: [
    { id: 'root' },
    { id: 'empty' },
    { id: 'items' },
    { id: 'item' },
    { id: 'itemName' },
    { id: 'itemMeta' },
    { id: 'itemMode' },
  ],
  states: [
    { id: 'active', selector: { kind: 'self', suffix: '[data-bf-state~="active"]' } },
  ],
};
