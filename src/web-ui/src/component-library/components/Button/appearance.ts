import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';

export const buttonAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'button',
  parts: [
    { id: 'root', propertyProfile: 'control', visualRole: 'control' },
    { id: 'loadingIcon', propertyProfile: 'control', visualRole: 'decoration' },
    { id: 'loadingText', propertyProfile: 'paint', visualRole: 'content' },
  ],
  facets: [
    {
      id: 'variant',
      attribute: 'data-bf-variant',
      values: ['primary', 'secondary', 'ghost', 'dashed', 'danger', 'success', 'accent', 'ai'],
    },
    {
      id: 'size',
      attribute: 'data-bf-size',
      values: ['small', 'medium', 'large'],
    },
    {
      id: 'iconOnly',
      attribute: 'data-bf-icon-only',
      values: ['true'],
    },
  ],
  states: [
    { id: 'hover', selector: { kind: 'self', suffix: ':hover:not(:disabled)' } },
    { id: 'active', selector: { kind: 'self', suffix: ':active:not(:disabled)' } },
    { id: 'focusVisible', selector: { kind: 'self', suffix: ':focus-visible' } },
    { id: 'disabled', selector: { kind: 'self', suffix: ':disabled' } },
    { id: 'loading', selector: { kind: 'self', suffix: '[data-bf-state~="loading"]' } },
  ],
};
