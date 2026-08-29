import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const workspaceItemAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'workspace-item',
  parts: [
    { id: 'root' }, { id: 'card' }, { id: 'collapse' }, { id: 'icon' },
    { id: 'name' }, { id: 'label' }, { id: 'badge' }, { id: 'action' },
    { id: 'menu' }, { id: 'menuTrigger' },
    { id: 'sessions' }, { id: 'remoteStatus' },
    { id: 'indexIndicator' }, { id: 'indexPanel' },
  ],
  facets: [{ id: 'variant', attribute: 'data-bf-variant', values: ['workspace', 'assistant'] }],
  states: [
    { id: 'active', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-bf-state~="active"]' } },
    { id: 'dragging', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-bf-state~="dragging"]' } },
    { id: 'collapsed', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-bf-state~="collapsed"]' } },
    { id: 'remote', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-bf-state~="remote"]' } },
    { id: 'open', selector: { kind: 'self', suffix: '[data-bf-state~="open"]' } },
  ],
};
