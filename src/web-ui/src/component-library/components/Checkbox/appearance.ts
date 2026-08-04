import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';
export const checkboxAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'checkbox', parts: [{ id: 'root' }, { id: 'wrapper' }, { id: 'input' }, { id: 'box' }, { id: 'icon' }, { id: 'content' }, { id: 'label' }, { id: 'description' }],
  facets: [{ id: 'size', attribute: 'data-bf-size', values: ['small', 'medium', 'large'] }],
  states: [
    { id: 'hover', selector: { kind: 'ancestorPart', part: 'root', suffix: ':hover' } },
    { id: 'focusWithin', selector: { kind: 'ancestorPart', part: 'root', suffix: ':focus-within' } },
    { id: 'checked', selector: { kind: 'ancestorPart', part: 'root', suffix: ':has(input:checked)' } },
    { id: 'indeterminate', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-bf-state~="indeterminate"]' } },
    { id: 'disabled', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-bf-state~="disabled"]' } },
    { id: 'error', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-bf-state~="error"]' } },
  ],
};
