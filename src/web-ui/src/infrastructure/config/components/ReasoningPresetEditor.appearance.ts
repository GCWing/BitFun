import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const reasoningPresetEditorAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'reasoning-preset-editor',
  parts: [
    { id: 'root' },
    { id: 'section' },
    { id: 'primarySettings' },
    { id: 'binding' },
    { id: 'generated' },
    { id: 'header' },
    { id: 'empty' },
    { id: 'list' },
    { id: 'preset' },
    { id: 'presetSummary' },
    { id: 'presetEditor' },
    { id: 'actions' },
    { id: 'action' },
    { id: 'actionControls' },
  ],
  states: [
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-bf-state~="expanded"]' } },
  ],
};
