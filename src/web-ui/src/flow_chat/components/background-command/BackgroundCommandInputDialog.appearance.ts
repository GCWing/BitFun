import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const backgroundCommandInputDialogAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'background-command-input-dialog',
  parts: [
    { id: 'root' }, { id: 'summary' }, { id: 'summaryLabel' }, { id: 'command' },
    { id: 'options' }, { id: 'note' }, { id: 'actions' },
  ],
  states: [
    { id: 'sending', selector: { kind: 'self', suffix: '[data-openbitfun-state~="sending"]' } },
    { id: 'masked', selector: { kind: 'self', suffix: '[data-openbitfun-state~="masked"]' } },
  ],
};
