import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';

export const cardAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'card',
  parts: [
    { id: 'root' },
    { id: 'header' },
    { id: 'headerContent' },
    { id: 'title' },
    { id: 'subtitle' },
    { id: 'extra' },
    { id: 'body' },
    { id: 'footer' },
  ],
  facets: [
    {
      id: 'variant',
      attribute: 'data-bf-variant',
      values: ['default', 'elevated', 'subtle', 'accent', 'purple'],
    },
    {
      id: 'padding',
      attribute: 'data-bf-padding',
      values: ['none', 'small', 'medium', 'large'],
    },
    {
      id: 'radius',
      attribute: 'data-bf-radius',
      values: ['small', 'medium', 'large'],
    },
    {
      id: 'align',
      attribute: 'data-bf-align',
      values: ['left', 'center', 'right', 'between'],
    },
  ],
  states: [
    { id: 'hover', selector: { kind: 'self', suffix: ':hover' } },
    { id: 'active', selector: { kind: 'self', suffix: ':active' } },
    { id: 'interactive', selector: { kind: 'self', suffix: '[data-bf-state~="interactive"]' } },
    { id: 'fullWidth', selector: { kind: 'self', suffix: '[data-bf-state~="fullWidth"]' } },
  ],
};
