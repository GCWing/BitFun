import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const deviceOverviewAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'device-overview',
  componentAttribute: 'data-openbitfun-product-component',
  parts: [{ id: 'root', propertyProfile: 'overlay', visualRole: 'popup' }],
  facets: [
    { id: 'placement', attribute: 'data-openbitfun-placement', values: ['top', 'bottom'] },
  ],
  states: [
    { id: 'local', selector: { kind: 'self', suffix: '[data-openbitfun-state~="local"]' } },
    { id: 'connected', selector: { kind: 'self', suffix: '[data-openbitfun-state~="connected"]' } },
  ],
};
