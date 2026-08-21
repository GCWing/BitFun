import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const groupChatViewAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'group-chat-view',
  parts: [
    { id: 'root', propertyProfile: 'layout', visualRole: 'continuous-surface', continuityGroup: 'session-workspace' },
    { id: 'headerActions', propertyProfile: 'control', visualRole: 'control' },
    { id: 'body', propertyProfile: 'layout', visualRole: 'continuous-surface', continuityGroup: 'session-workspace' },
    { id: 'emptyState', visualRole: 'content' },
    { id: 'input', propertyProfile: 'layout', visualRole: 'toolbar' },
  ],
};
