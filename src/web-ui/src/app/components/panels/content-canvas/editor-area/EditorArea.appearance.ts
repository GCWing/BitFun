import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const canvasEditorAreaAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'canvas-editor-area',
  parts: [
    { id: 'root' }, { id: 'primary' }, { id: 'secondary' },
    { id: 'tertiary' }, { id: 'topRow' },
  ],
  facets: [{ id: 'layout', attribute: 'data-openbitfun-layout', values: ['none', 'horizontal', 'vertical', 'grid'] }],
};
