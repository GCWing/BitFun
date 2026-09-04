import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const welcomePanelAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'welcome-panel',
  parts: [
    { id: 'root', propertyProfile: 'layout', visualRole: 'workspace' },
    { id: 'content', visualRole: 'card' },
    { id: 'greeting', visualRole: 'content' },
    { id: 'heading', propertyProfile: 'paint', visualRole: 'content' },
    { id: 'tagline', propertyProfile: 'paint', visualRole: 'content' },
    { id: 'divider', propertyProfile: 'paint', visualRole: 'divider' },
    { id: 'narrative', propertyProfile: 'paint', visualRole: 'content' },
    { id: 'openWorkspaceAction', propertyProfile: 'control', visualRole: 'control' },
    { id: 'workspaceAction', propertyProfile: 'control', visualRole: 'control' },
    { id: 'gitAction', propertyProfile: 'control', visualRole: 'control' },
    { id: 'workspaceMenu', propertyProfile: 'overlay', visualRole: 'popup' },
    { id: 'workspaceItem', propertyProfile: 'control', visualRole: 'control' },
    { id: 'cowork', visualRole: 'content' },
  ],
  states: [
    { id: 'hover', selector: { kind: 'self', suffix: ':hover:not(:disabled)' } },
    { id: 'focusVisible', selector: { kind: 'self', suffix: ':focus-visible' } },
    { id: 'disabled', selector: { kind: 'self', suffix: ':disabled' } },
    { id: 'open', selector: { kind: 'self', suffix: '[data-openbitfun-state~="open"]' } },
  ],
};
