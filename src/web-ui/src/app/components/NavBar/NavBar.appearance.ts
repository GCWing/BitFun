import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const navBarAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'nav-bar',
  parts: [{ id: 'root' }, { id: 'panelToggle' }, { id: 'back' }, { id: 'forward' }],
  states: [{ id: 'collapsed', selector: { kind: 'self', suffix: '[data-openbitfun-state~="collapsed"]' } }],
};
