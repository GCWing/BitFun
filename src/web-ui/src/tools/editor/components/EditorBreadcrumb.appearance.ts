import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const editorBreadcrumbAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'editor-breadcrumb',
  parts: [
    { id: 'root' }, { id: 'separator' }, { id: 'item' }, { id: 'itemIcon' },
    { id: 'itemText' }, { id: 'menu' }, { id: 'menuHeader' }, { id: 'menuBack' },
    { id: 'menuTitle' }, { id: 'loading' }, { id: 'empty' }, { id: 'list' },
    { id: 'listItem' },
  ],
  states: [
    { id: 'active', selector: { kind: 'self', suffix: '[data-bf-state~="active"]' } },
    { id: 'selected', selector: { kind: 'self', suffix: '[data-bf-state~="selected"]' } },
  ],
};
