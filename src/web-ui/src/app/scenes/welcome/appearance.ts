import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';
export const welcomeAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'welcome', parts: [
    { id: 'root', propertyProfile: 'layout', visualRole: 'workspace' },
    { id: 'content', visualRole: 'card' },
    { id: 'greeting', visualRole: 'content' },
    { id: 'title', propertyProfile: 'paint', visualRole: 'content' },
    { id: 'subtitle', propertyProfile: 'paint', visualRole: 'content' },
    { id: 'divider', propertyProfile: 'paint', visualRole: 'divider' },
    { id: 'recentSection', visualRole: 'content' },
    { id: 'sectionHeader', visualRole: 'toolbar' },
    { id: 'sectionLabel', propertyProfile: 'paint', visualRole: 'content' },
    { id: 'actions', visualRole: 'toolbar' },
    { id: 'action', propertyProfile: 'control', visualRole: 'control' },
    { id: 'recentList', visualRole: 'content' },
    { id: 'recentRow', visualRole: 'content' },
    { id: 'recentItem', propertyProfile: 'control', visualRole: 'control' },
    { id: 'recentName', propertyProfile: 'paint', visualRole: 'content' },
    { id: 'recentMeta', propertyProfile: 'paint', visualRole: 'content' },
    { id: 'empty', visualRole: 'content' },
  ],
  states: [{ id: 'hover', selector: { kind: 'self', suffix: ':hover' } }, { id: 'focusVisible', selector: { kind: 'self', suffix: ':focus-visible' } }, { id: 'disabled', selector: { kind: 'self', suffix: ':disabled' } }],
};
