import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const canvasToolAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'canvas-tool',
  parts: [
    { id: 'root' },
    { id: 'empty' },
    { id: 'message' },
    { id: 'diagnostics' },
    { id: 'source' },
    { id: 'toolbar' },
    { id: 'frame' },
    { id: 'sourceOverlay' },
    { id: 'sourceDialog' },
  ],
  states: [
    { id: 'empty', selector: { kind: 'self', suffix: '[data-bf-state~="empty"]' } },
  ],
};
