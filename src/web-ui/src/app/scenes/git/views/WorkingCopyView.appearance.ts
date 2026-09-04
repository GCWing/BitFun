import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const workingCopyViewAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'working-copy-view',
  parts: [
    { id: 'root' }, { id: 'placeholder' }, { id: 'commitBar' }, { id: 'statusRow' },
    { id: 'branch' }, { id: 'syncActions' }, { id: 'commitInput' }, { id: 'commitActions' },
    { id: 'main' }, { id: 'fileList' }, { id: 'search' }, { id: 'groupHeader' },
    { id: 'file' }, { id: 'fileCheck' }, { id: 'fileName' }, { id: 'fileStatus' },
    { id: 'resizer' }, { id: 'diffArea' }, { id: 'empty' },
  ],
  states: [
    { id: 'selected', selector: { kind: 'self', suffix: '[data-openbitfun-state~="selected"]' } },
    { id: 'loading', selector: { kind: 'self', suffix: '[data-openbitfun-state~="loading"]' } },
  ],
};
