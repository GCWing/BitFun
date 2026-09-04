import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const canvasTabBarAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'canvas-tab-bar',
  parts: [
    { id: 'root', visualRole: 'toolbar', continuityGroup: 'canvas-tabs' },
    { id: 'list', visualRole: 'continuous-surface', continuityGroup: 'canvas-tabs' },
    { id: 'tabWrapper', visualRole: 'continuous-surface', continuityGroup: 'canvas-tabs' },
    { id: 'dropIndicator', propertyProfile: 'overlay', visualRole: 'divider' },
    { id: 'actions', visualRole: 'toolbar' },
    { id: 'action', propertyProfile: 'control', visualRole: 'control' },
  ],
  facets: [{ id: 'group', attribute: 'data-openbitfun-group', values: ['primary', 'secondary', 'tertiary'] }],
  states: [{ id: 'active', selector: { kind: 'self', suffix: '[data-openbitfun-state~="active"]' } }],
};
