import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const subagentAvatarAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'subagent-avatar',
  parts: [{ id: 'root' }],
  states: [
    { id: 'running', selector: { kind: 'self', suffix: '[data-bf-state~="running"]' } },
    { id: 'finishing', selector: { kind: 'self', suffix: '[data-bf-state~="finishing"]' } },
    { id: 'waiting', selector: { kind: 'self', suffix: '[data-bf-state~="waiting"]' } },
    { id: 'completed', selector: { kind: 'self', suffix: '[data-bf-state~="completed"]' } },
    { id: 'cancelled', selector: { kind: 'self', suffix: '[data-bf-state~="cancelled"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-bf-state~="error"]' } },
    { id: 'idle', selector: { kind: 'self', suffix: '[data-bf-state~="idle"]' } },
  ],
};
