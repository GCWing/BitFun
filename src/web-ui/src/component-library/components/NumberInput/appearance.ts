import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';
export const numberInputAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'number-input', parts: [{ id: 'root' }, { id: 'label' }, { id: 'container' }, { id: 'glow' }, { id: 'valueArea' }, { id: 'input' }, { id: 'unit' }, { id: 'buttons' }, { id: 'decrement' }, { id: 'increment' }, { id: 'progress' }, { id: 'progressBar' }],
  facets: [{ id: 'variant', attribute: 'data-bf-variant', values: ['default', 'compact', 'stepper'] }, { id: 'size', attribute: 'data-bf-size', values: ['small', 'medium', 'large'] }],
  states: [{ id: 'hover', selector: { kind: 'self', suffix: ':hover' } }, { id: 'focusWithin', selector: { kind: 'self', suffix: ':focus-within' } }, { id: 'disabled', selector: { kind: 'self', suffix: '[data-bf-state~="disabled"]' } }, { id: 'editing', selector: { kind: 'self', suffix: '[data-bf-state~="editing"]' } }, { id: 'dragging', selector: { kind: 'self', suffix: '[data-bf-state~="dragging"]' } }],
};
