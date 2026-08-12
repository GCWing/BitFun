import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const groupChatMemberPickerAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'group-chat-member-picker',
  parts: [
    { id: 'root' },
    { id: 'members' },
    { id: 'member' },
    { id: 'memberName' },
    { id: 'memberRole' },
    { id: 'leaveButton' },
    { id: 'addToggle' },
    { id: 'addList' },
    { id: 'addItem' },
  ],
  states: [
    { id: 'owner', selector: { kind: 'self', suffix: '[data-bf-state~="owner"]' } },
    { id: 'member', selector: { kind: 'self', suffix: '[data-bf-state~="member"]' } },
  ],
};
