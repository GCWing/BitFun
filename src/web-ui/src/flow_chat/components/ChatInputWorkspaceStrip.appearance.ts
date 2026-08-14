import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const chatInputWorkspaceStripAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'chat-input-workspace-strip',
  parts: [
    { id: 'root' }, { id: 'main' }, { id: 'workspace' }, { id: 'branch' },
    { id: 'harness' }, { id: 'runtime' },
    { id: 'actions' }, { id: 'permission' }, { id: 'permissionMenu' },
    { id: 'permissionOptions' }, { id: 'permissionOptionRow' },
    { id: 'permissionOption' }, { id: 'permissionOptionTrailing' },
    { id: 'permissionOptionNextTurn' }, { id: 'usageAction' },
  ],
  states: [
    { id: 'open', selector: { kind: 'self', suffix: '[data-bf-state~="open"]' } },
    { id: 'selected', selector: { kind: 'self', suffix: '[data-bf-state~="selected"]' } },
    { id: 'armed', selector: { kind: 'self', suffix: '[data-bf-state~="armed"]' } },
  ],
};
