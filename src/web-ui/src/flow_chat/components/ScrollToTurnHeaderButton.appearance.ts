import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const scrollToTurnHeaderButtonAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'scroll-to-turn-header-button',
  parts: [{ id: 'root' }, { id: 'gradient' }, { id: 'content' }, { id: 'button' }],
  states: [{ id: 'visible', selector: { kind: 'self', suffix: '[data-openbitfun-state~="visible"]' } }],
};
