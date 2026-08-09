import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const lspAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'lsp',
  parts: [
    { id: 'pluginList' },
    { id: 'empty' },
    { id: 'items' },
    { id: 'plugin' },
    { id: 'pluginHeader' },
    { id: 'pluginDetails' },
  ],
  states: [
    { id: 'empty', selector: { kind: 'self', suffix: '[data-bf-state~="empty"]' } },
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-bf-state~="expanded"]' } },
  ],
};
