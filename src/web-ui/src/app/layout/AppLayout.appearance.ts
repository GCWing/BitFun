import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const appLayoutAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'app-layout',
  parts: [
    { id: 'root' }, { id: 'main' }, { id: 'windowModeHint' },
    { id: 'windowModeTitle' }, { id: 'windowModeDetail' },
  ],
  states: [
    { id: 'fullscreen', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-openbitfun-state~="fullscreen"]' } },
    { id: 'toolbar', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-openbitfun-state~="toolbar"]' } },
  ],
};
