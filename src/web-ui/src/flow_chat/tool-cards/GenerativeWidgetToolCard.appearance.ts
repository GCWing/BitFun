import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const generativeWidgetToolCardAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'generative-widget-tool-card',
  parts: [
    { id: 'root' }, { id: 'title' }, { id: 'extra' }, { id: 'status' },
    { id: 'exportAction' }, { id: 'placeholder' }, { id: 'preview' },
    { id: 'captureRoot' }, { id: 'exportStage' },
  ],
  states: [
    { id: 'failed', selector: { kind: 'self', suffix: '[data-bf-state~="failed"]' } },
    { id: 'exporting', selector: { kind: 'self', suffix: '[data-bf-state~="exporting"]' } },
  ],
};
