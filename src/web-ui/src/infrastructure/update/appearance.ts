import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const updateAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'update',
  parts: [
    { id: 'availableRoot' },
    { id: 'lead' },
    { id: 'leadIcon' },
    { id: 'subtitle' },
    { id: 'versions' },
    { id: 'versionRow' },
    { id: 'versionLabel' },
    { id: 'versionValue' },
    { id: 'notes' },
    { id: 'notesLabel' },
    { id: 'notesBody' },
    { id: 'actions' },
    { id: 'progressRoot' },
    { id: 'alert' },
    { id: 'progressBar' },
    { id: 'progressFill' },
    { id: 'progressHint' },
    { id: 'restartHint' },
  ],
  facets: [
    { id: 'variant', attribute: 'data-bf-variant', values: ['daily', 'manual'] },
    { id: 'status', attribute: 'data-bf-status', values: ['downloading', 'error', 'installed'] },
  ],
  states: [
    { id: 'highlight', selector: { kind: 'self', suffix: '[data-bf-state~="highlight"]' } },
    { id: 'indeterminate', selector: { kind: 'self', suffix: '[data-bf-state~="indeterminate"]' } },
  ],
};
