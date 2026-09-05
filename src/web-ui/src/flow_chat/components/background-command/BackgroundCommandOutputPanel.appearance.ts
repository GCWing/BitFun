import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const backgroundCommandOutputPanelAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'background-command-output-panel',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'title' }, { id: 'headerActions' },
    { id: 'meta' }, { id: 'notice' }, { id: 'error' }, { id: 'output' },
    { id: 'terminal' }, { id: 'empty' }, { id: 'inputEditor' },
    { id: 'inputOptions' }, { id: 'inputActions' },
  ],
  states: [
    { id: 'loading', selector: { kind: 'self', suffix: '[data-openbitfun-state~="loading"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-openbitfun-state~="error"]' } },
  ],
};
