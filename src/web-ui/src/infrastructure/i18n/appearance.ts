import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const languageSelectorAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'language-selector',
  parts: [
    { id: 'root' },
    { id: 'trigger' },
    { id: 'icon' },
    { id: 'code' },
    { id: 'menu' },
    { id: 'option' },
    { id: 'optionLabel' },
    { id: 'check' },
    { id: 'loading' },
  ],
  facets: [
    { id: 'mode', attribute: 'data-openbitfun-mode', values: ['dropdown', 'inline', 'icon-only'] },
  ],
  states: [
    { id: 'active', selector: { kind: 'self', suffix: '[data-openbitfun-state~="active"]' } },
    { id: 'changing', selector: { kind: 'self', suffix: '[data-openbitfun-state~="changing"]' } },
  ],
};
