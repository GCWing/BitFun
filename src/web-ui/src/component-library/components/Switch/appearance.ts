import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';
export const switchAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'switch', parts: [{ id: 'root' }, { id: 'wrapper' }, { id: 'input' }, { id: 'track' }, { id: 'text' }, { id: 'thumb' }, { id: 'loading' }, { id: 'content' }, { id: 'label' }, { id: 'description' }],
  facets: [{ id: 'size', attribute: 'data-bf-size', values: ['small', 'medium', 'large'] }],
  states: [{ id: 'hover', selector: { kind: 'self', suffix: ':hover' } }, { id: 'focusWithin', selector: { kind: 'self', suffix: ':focus-within' } }, { id: 'checked', selector: { kind: 'self', suffix: '[data-bf-state~="checked"]' } }, { id: 'disabled', selector: { kind: 'self', suffix: '[data-bf-state~="disabled"]' } }, { id: 'loading', selector: { kind: 'self', suffix: '[data-bf-state~="loading"]' } }],
};
