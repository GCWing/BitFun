import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const shellNavAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'shell-nav',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'title' }, { id: 'headerActions' },
    { id: 'splitButton' }, { id: 'menu' }, { id: 'menuItem' },
    { id: 'empty' }, { id: 'list' },
  ],
  states: [
    { id: 'active', selector: { kind: 'self', suffix: '[data-bf-state~="active"]' } },
    { id: 'selected', selector: { kind: 'self', suffix: '[data-bf-state~="selected"]' } },
  ],
};
