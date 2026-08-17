import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const flowChatHeaderAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'flow-chat-header',
  parts: [
    { id: 'root' },
    { id: 'leftActions' },
    { id: 'message' },
    { id: 'turnBadge' },
    { id: 'actions' },
    { id: 'backgroundActivity' },
    { id: 'activityPanel' },
    { id: 'activitySection' },
    { id: 'commandMenu' },
    { id: 'commandItem' },
    { id: 'search' },
    { id: 'searchControls' },
    { id: 'sessionTree' },
    { id: 'sessionTreeTrigger' },
    { id: 'sessionTreePanel' },
    { id: 'sessionTreeHeader' },
    { id: 'sessionTreeBody' },
    { id: 'sessionTreeNode' },
    { id: 'sessionTreeNodeTitle' },
    { id: 'sessionTreeMenu' },
    { id: 'sessionTreeMenuItem' },
    { id: 'sessionTreeState' },
  ],
  facets: [
    {
      id: 'status',
      attribute: 'data-bf-status',
      values: ['running', 'finishing', 'waiting', 'completed', 'cancelled', 'error', 'idle'],
    },
  ],
  states: [
    { id: 'open', selector: { kind: 'self', suffix: '[data-bf-state~="open"]' } },
    { id: 'active', selector: { kind: 'self', suffix: '[data-bf-state~="active"]' } },
    { id: 'cancelling', selector: { kind: 'self', suffix: '[data-bf-state~="cancelling"]' } },
    { id: 'loading', selector: { kind: 'self', suffix: '[data-bf-state~="loading"]' } },
    { id: 'empty', selector: { kind: 'self', suffix: '[data-bf-state~="empty"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-bf-state~="error"]' } },
  ],
};
