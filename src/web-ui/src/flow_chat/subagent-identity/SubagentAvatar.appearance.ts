import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const subagentAvatarAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'subagent-avatar',
  parts: [{ id: 'root' }],
  states: [
    { id: 'running', selector: { kind: 'self', suffix: '[data-openbitfun-state~="running"]' } },
    { id: 'finishing', selector: { kind: 'self', suffix: '[data-openbitfun-state~="finishing"]' } },
    { id: 'waiting', selector: { kind: 'self', suffix: '[data-openbitfun-state~="waiting"]' } },
    { id: 'completed', selector: { kind: 'self', suffix: '[data-openbitfun-state~="completed"]' } },
    { id: 'cancelled', selector: { kind: 'self', suffix: '[data-openbitfun-state~="cancelled"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-openbitfun-state~="error"]' } },
    { id: 'idle', selector: { kind: 'self', suffix: '[data-openbitfun-state~="idle"]' } },
  ],
};
