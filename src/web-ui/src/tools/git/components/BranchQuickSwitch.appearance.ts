import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const branchQuickSwitchAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'branch-quick-switch',
  componentAttribute: 'data-openbitfun-product-component',
  parts: [
    { id: 'root' }, { id: 'search' }, { id: 'input' },
    { id: 'loading' }, { id: 'empty' },
  ],
};
