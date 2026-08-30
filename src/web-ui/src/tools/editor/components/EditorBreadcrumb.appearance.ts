import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const editorBreadcrumbAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'editor-breadcrumb',
  componentAttribute: 'data-bf-product-component',
  parts: [
    { id: 'root' }, { id: 'separator' }, { id: 'item' }, { id: 'itemIcon' },
    { id: 'itemText' }, { id: 'menu' }, { id: 'loading' }, { id: 'empty' },
  ],
  states: [
    { id: 'active', selector: { kind: 'self', suffix: '[data-bf-state~="active"]' } },
  ],
};
