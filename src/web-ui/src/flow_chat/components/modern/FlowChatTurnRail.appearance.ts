import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const flowChatTurnRailAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'flow-chat-turn-rail',
  parts: [
    { id: 'root' },
    { id: 'list' },
    { id: 'item' },
    { id: 'bar' },
    { id: 'tooltipContent' },
    { id: 'tooltipTurn' },
    { id: 'tooltipMessage' },
  ],
  states: [
    { id: 'current', selector: { kind: 'self', suffix: '[data-openbitfun-state~="current"]' } },
    { id: 'visible', selector: { kind: 'self', suffix: '[data-openbitfun-state~="visible"]' } },
  ],
};
