import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const gitToolDisplayAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'git-tool-display',
  parts: [
    { id: 'root' }, { id: 'headerExtra' }, { id: 'info' }, { id: 'result' },
    { id: 'output' }, { id: 'stderr' }, { id: 'footer' }, { id: 'error' },
  ],
  states: [{ id: 'failed', selector: { kind: 'self', suffix: '[data-bf-state~="failed"]' } }],
};
