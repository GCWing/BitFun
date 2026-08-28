import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const deviceOverviewAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'device-overview',
  componentAttribute: 'data-bf-product-component',
  parts: [{ id: 'root', propertyProfile: 'overlay', visualRole: 'popup' }],
  facets: [
    { id: 'placement', attribute: 'data-bf-placement', values: ['top', 'bottom'] },
  ],
  states: [
    { id: 'local', selector: { kind: 'self', suffix: '[data-bf-state~="local"]' } },
    { id: 'connected', selector: { kind: 'self', suffix: '[data-bf-state~="connected"]' } },
  ],
};
