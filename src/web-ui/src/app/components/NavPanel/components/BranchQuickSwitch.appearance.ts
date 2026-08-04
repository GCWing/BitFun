import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const branchQuickSwitchAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'branch-quick-switch',
  parts: [
    { id: 'root' }, { id: 'search' }, { id: 'input' }, { id: 'list' },
    { id: 'loading' }, { id: 'empty' }, { id: 'item' }, { id: 'itemName' },
  ],
  states: [
    { id: 'selected', selector: { kind: 'self', suffix: '[data-bf-state~="selected"]' } },
    { id: 'current', selector: { kind: 'self', suffix: '[data-bf-state~="current"]' } },
  ],
};
