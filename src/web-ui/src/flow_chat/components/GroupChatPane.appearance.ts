import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const groupChatPaneAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'group-chat-pane',
  parts: [
    { id: 'empty' },
    { id: 'root' },
    { id: 'header' },
    { id: 'memberCount' },
    { id: 'modeToggle' },
    { id: 'memberToggle' },
    { id: 'timeoutReminders' },
    { id: 'timeoutReminder' },
    { id: 'messages' },
    { id: 'message' },
    { id: 'messageAuthor' },
    { id: 'messageContent' },
    { id: 'input' },
    { id: 'textInput' },
  ],
};
