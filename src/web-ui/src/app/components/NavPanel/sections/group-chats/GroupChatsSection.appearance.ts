import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

/**
 * R-WF-12: Group Chats nav section. Renders its own collapsible section
 * header, the two create entries, and the group-chat-only session list
 * (reusing SessionsSection). Registered so the `data-bf-component="group-chats"`
 * markers (empty state) resolve against the appearance contract audit.
 */
export const groupChatsSectionAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'group-chats',
  parts: [
    { id: 'empty' },
  ],
  states: [],
};
