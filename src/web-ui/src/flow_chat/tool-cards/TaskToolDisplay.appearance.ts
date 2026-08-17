import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const taskToolDisplayAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'task-tool-display',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'body' }, { id: 'main' },
    { id: 'action' }, { id: 'subagentName' }, { id: 'meta' }, { id: 'rail' }, { id: 'expanded' },
    { id: 'interruption' }, { id: 'reviewer' }, { id: 'responsibilities' },
    { id: 'prompt' }, { id: 'cancel' },
  ],
  states: [
    { id: 'failed', selector: { kind: 'self', suffix: '[data-bf-state~="failed"]' } },
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-bf-state~="expanded"]' } },
  ],
};
