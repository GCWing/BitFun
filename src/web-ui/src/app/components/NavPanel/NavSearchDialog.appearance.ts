import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const navSearchDialogAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'nav-search-dialog',
  parts: [
    { id: 'root' }, { id: 'card' }, { id: 'inputRow' }, { id: 'search' },
    { id: 'results' }, { id: 'empty' }, { id: 'group' }, { id: 'groupLabel' },
    { id: 'item' }, { id: 'itemIcon' }, { id: 'itemContent' }, { id: 'itemLabel' },
    { id: 'itemSublabel' }, { id: 'sessionHint' },
  ],
  states: [{ id: 'active', selector: { kind: 'self', suffix: '[data-bf-state~="active"]' } }],
};
