import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const runCodeToolCardAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'run-code-tool-card',
  parts: [
    { id: 'root' }, { id: 'expanded' }, { id: 'section' },
    { id: 'label' }, { id: 'output' }, { id: 'error' },
  ],
  states: [
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-bf-state~="expanded"]' } },
    { id: 'failed', selector: { kind: 'self', suffix: '[data-bf-state~="failed"]' } },
  ],
};
