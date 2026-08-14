import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

/**
 * The surface id stays `permission-request-panel` even though the component is
 * now a band inside the composer: published skins target this id, and renaming
 * it would silently drop their styling for the approval surface.
 */
export const chatInputApprovalBandAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'permission-request-panel',
  parts: [
    { id: 'root' }, { id: 'request' }, { id: 'risk' }, { id: 'error' },
    { id: 'actions' }, { id: 'scope' },
  ],
  states: [
    { id: 'responding', selector: { kind: 'self', suffix: '[data-bf-state~="responding"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-bf-state~="error"]' } },
  ],
};
