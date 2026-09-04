import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const fileMentionPickerAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'file-mention-picker',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'content' },
    { id: 'currentDirectoryName' },
    { id: 'parentDirectoryPath' }, { id: 'footer' },
  ],
  states: [
    { id: 'loading', selector: { kind: 'self', suffix: '[data-openbitfun-state~="loading"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-openbitfun-state~="error"]' } },
  ],
};
