import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const branchQuickSwitchAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'branch-quick-switch',
  componentAttribute: 'data-bf-product-component',
  parts: [
    { id: 'root' }, { id: 'search' }, { id: 'input' },
    { id: 'loading' }, { id: 'empty' },
  ],
};
