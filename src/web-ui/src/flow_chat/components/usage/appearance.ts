import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const sessionUsagePanelAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'session-usage-panel',
  parts: [
    { id: 'root' },
    { id: 'header' },
    { id: 'tabs' },
    { id: 'tab' },
    { id: 'body' },
    { id: 'metaRow' },
    { id: 'section' },
    { id: 'metric' },
    { id: 'table' },
    { id: 'empty' },
  ],
  facets: [
    { id: 'tab', attribute: 'data-bf-tab', values: ['overview', 'models', 'tools', 'files', 'errors', 'slowest'] },
  ],
  states: [
    { id: 'active', selector: { kind: 'self', suffix: '[data-bf-state~="active"]' } },
    { id: 'fallback', selector: { kind: 'self', suffix: '[data-bf-state~="fallback"]' } },
  ],
};
