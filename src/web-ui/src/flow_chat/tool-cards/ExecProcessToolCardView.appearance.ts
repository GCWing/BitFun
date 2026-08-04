import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const execProcessToolCardAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'exec-process-tool-card',
  parts: [
    { id: 'root' }, { id: 'status' }, { id: 'actions' }, { id: 'output' },
    { id: 'result' }, { id: 'footer' }, { id: 'waiting' }, { id: 'error' },
  ],
  states: [
    { id: 'cancelled', selector: { kind: 'self', suffix: '[data-bf-state~="cancelled"]' } },
    { id: 'waiting', selector: { kind: 'self', suffix: '[data-bf-state~="waiting"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-bf-state~="error"]' } },
  ],
};
