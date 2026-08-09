import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const permissionRequestPanelAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'permission-request-panel',
  parts: [
    { id: 'root' }, { id: 'collapsedTrigger' }, { id: 'collapsedBadge' },
    { id: 'panel' }, { id: 'heading' }, { id: 'headingActions' },
    { id: 'requests' }, { id: 'request' }, { id: 'requestHeading' },
    { id: 'requestDetails' }, { id: 'risk' }, { id: 'error' },
    { id: 'feedback' }, { id: 'actions' }, { id: 'singleActions' }, { id: 'batchActions' },
  ],
  states: [
    { id: 'collapsed', selector: { kind: 'self', suffix: '[data-bf-state~="collapsed"]' } },
    { id: 'responding', selector: { kind: 'self', suffix: '[data-bf-state~="responding"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-bf-state~="error"]' } },
  ],
};
