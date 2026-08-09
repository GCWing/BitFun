import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const workspaceSessionBatchModalAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'workspace-session-batch-modal',
  parts: [
    { id: 'root' }, { id: 'hero' }, { id: 'toolbar' }, { id: 'toolbarActions' },
    { id: 'summary' }, { id: 'list' }, { id: 'state' }, { id: 'row' },
    { id: 'rowCheck' }, { id: 'rowContent' }, { id: 'footer' },
  ],
  states: [
    { id: 'selected', selector: { kind: 'self', suffix: '[data-bf-state~="selected"]' } },
    { id: 'loading', selector: { kind: 'self', suffix: '[data-bf-state~="loading"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-bf-state~="error"]' } },
    { id: 'child', selector: { kind: 'self', suffix: '[data-bf-state~="child"]' } },
  ],
};
