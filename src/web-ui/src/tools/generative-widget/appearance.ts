import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const generativeWidgetAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'generative-widget',
  parts: [
    { id: 'root' },
    { id: 'empty' },
    { id: 'toolbar' },
    { id: 'toolbarMeta' },
    { id: 'toolbarActions' },
    { id: 'workspace' },
    { id: 'editorPane' },
    { id: 'paneHeader' },
    { id: 'editor' },
    { id: 'error' },
    { id: 'frame' },
    { id: 'iframe' },
  ],
  states: [
    { id: 'empty', selector: { kind: 'self', suffix: '[data-openbitfun-state~="empty"]' } },
  ],
};
