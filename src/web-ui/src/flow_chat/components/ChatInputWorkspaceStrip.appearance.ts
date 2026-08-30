import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const chatInputWorkspaceStripAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'chat-input-workspace-strip',
  // The strip is two rails: `context` is the situation the next turn starts
  // from, `next` is what that turn is configured with. The old `main` /
  // `harness` / `runtime` / `actions` grouping went with the conditional grid.
  parts: [
    { id: 'root' }, { id: 'context' }, { id: 'next' },
    { id: 'workspace' }, { id: 'workspaceMenu' },
    { id: 'workspaceOption' }, { id: 'branch' }, { id: 'divider' },
    { id: 'permission' }, { id: 'permissionMenu' },
    { id: 'permissionOptions' }, { id: 'usageAction' },
  ],
  states: [
    { id: 'open', selector: { kind: 'self', suffix: '[data-bf-state~="open"]' } },
    { id: 'selected', selector: { kind: 'self', suffix: '[data-bf-state~="selected"]' } },
    { id: 'active', selector: { kind: 'self', suffix: '[data-bf-state~="active"]' } },
    { id: 'armed', selector: { kind: 'self', suffix: '[data-bf-state~="armed"]' } },
  ],
};
