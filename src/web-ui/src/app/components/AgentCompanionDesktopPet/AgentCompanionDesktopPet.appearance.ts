import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const agentCompanionDesktopPetAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'agent-companion-desktop-pet',
  parts: [
    { id: 'root' }, { id: 'dock' }, { id: 'bubbles' }, { id: 'bubble' },
    { id: 'bubbleTitle' }, { id: 'bubbleStatus' }, { id: 'bubbleOutput' },
    { id: 'hitbox' }, { id: 'pet' },
  ],
  states: [
    { id: 'attention', selector: { kind: 'self', suffix: '[data-openbitfun-state~="attention"]' } },
    { id: 'typing', selector: { kind: 'self', suffix: '[data-openbitfun-state~="typing"]' } },
  ],
};
