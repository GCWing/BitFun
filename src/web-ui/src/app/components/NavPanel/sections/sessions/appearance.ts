import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const sessionsSectionAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'sessions-section',
  parts: [
    { id: 'root' },
    { id: 'loading' },
    { id: 'aggregateLoading' },
    { id: 'row' },
    { id: 'rowMain' },
    { id: 'assistantAvatar' },
    { id: 'edit' },
    { id: 'actions' },
    { id: 'menu' },
    { id: 'toggle' },
  ],
  states: [
    { id: 'active', selector: { kind: 'self', suffix: '[data-bf-state~="active"]' } },
    { id: 'editing', selector: { kind: 'self', suffix: '[data-bf-state~="editing"]' } },
    { id: 'menuOpen', selector: { kind: 'self', suffix: '[data-bf-state~="menuOpen"]' } },
    { id: 'loading', selector: { kind: 'self', suffix: '[data-bf-state~="loading"]' } },
    { id: 'partial', selector: { kind: 'self', suffix: '[data-bf-state~="partial"]' } },
  ],
};
