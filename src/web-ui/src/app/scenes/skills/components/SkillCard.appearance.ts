import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const skillCardAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'skill-card',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'iconArea' }, { id: 'icon' },
    { id: 'badges' }, { id: 'body' }, { id: 'titleRow' }, { id: 'name' },
    { id: 'meta' }, { id: 'description' }, { id: 'footer' }, { id: 'actions' },
    { id: 'action' },
  ],
  facets: [
    { id: 'variant', attribute: 'data-openbitfun-variant', values: ['skill', 'market'] },
    { id: 'tone', attribute: 'data-openbitfun-tone', values: ['primary', 'danger', 'success', 'muted'] },
  ],
  states: [
    { id: 'hover', selector: { kind: 'self', suffix: ':hover' } },
    { id: 'focusVisible', selector: { kind: 'self', suffix: ':focus-visible' } },
    { id: 'disabled', selector: { kind: 'self', suffix: '[data-openbitfun-state~="disabled"]' } },
  ],
};
