import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';

export const popupCloseButtonAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'popup-close-button',
  parts: [{ id: 'root', propertyProfile: 'control', visualRole: 'control' }],
  facets: [
    { id: 'variant', attribute: 'data-bf-variant', values: ['ghost'] },
    { id: 'size', attribute: 'data-bf-size', values: ['medium'] },
    { id: 'shape', attribute: 'data-bf-shape', values: ['square'] },
  ],
  states: [
    { id: 'hover', selector: { kind: 'self', suffix: ':hover:not(:disabled)' } },
    { id: 'active', selector: { kind: 'self', suffix: ':active:not(:disabled)' } },
    { id: 'focusVisible', selector: { kind: 'self', suffix: ':focus-visible' } },
    { id: 'disabled', selector: { kind: 'self', suffix: ':disabled' } },
  ],
};
