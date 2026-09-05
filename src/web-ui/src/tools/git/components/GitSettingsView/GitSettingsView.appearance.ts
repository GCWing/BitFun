import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const gitSettingsViewAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'git-settings-view',
  parts: [
    { id: 'root' }, { id: 'loading' }, { id: 'error' }, { id: 'header' },
    { id: 'headerLeft' }, { id: 'headerRight' }, { id: 'status' }, { id: 'tabs' },
    { id: 'content' }, { id: 'section' }, { id: 'formGroup' }, { id: 'configItem' },
  ],
  states: [
    { id: 'loading', selector: { kind: 'self', suffix: '[data-openbitfun-state~="loading"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-openbitfun-state~="error"]' } },
  ],
};
