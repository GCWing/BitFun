import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const splashScreenAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'splash-screen',
  parts: [{ id: 'root' }, { id: 'center' }, { id: 'logo' }, { id: 'message' }],
  states: [{ id: 'exiting', selector: { kind: 'self', suffix: '[data-openbitfun-state~="exiting"]' } }],
};
