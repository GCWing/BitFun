import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const groupChatCreateDialogAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'group-chat-create-dialog',
  parts: [
    { id: 'backdrop' },
    { id: 'title' },
    { id: 'close' },
    { id: 'nameInput' },
    { id: 'memberList' },
    { id: 'memberOption' },
    { id: 'cancel' },
    { id: 'create' },
  ],
  states: [
    { id: 'selected', selector: { kind: 'self', suffix: '[data-bf-state~="selected"]' } },
  ],
};
