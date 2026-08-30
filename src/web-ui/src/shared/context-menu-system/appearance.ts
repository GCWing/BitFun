import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const contextMenuAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'context-menu',
  componentAttribute: 'data-bf-product-component',
  parts: [
    { id: 'root' },
    { id: 'item' },
    { id: 'separator' },
    { id: 'icon' },
    { id: 'label' },
    { id: 'shortcut' },
    { id: 'submenuArrow' },
    { id: 'submenu' },
  ],
  states: [
    { id: 'disabled', selector: { kind: 'self', suffix: '[data-bf-state~="disabled"]' } },
    { id: 'submenuActive', selector: { kind: 'self', suffix: '[data-bf-state~="submenu-active"]' } },
  ],
};
