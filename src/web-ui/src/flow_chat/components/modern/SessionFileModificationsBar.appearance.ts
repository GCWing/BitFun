import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const sessionFileModificationsBarAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'session-file-modifications-bar',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'summary' }, { id: 'toggle' },
    { id: 'list' }, { id: 'file' }, { id: 'fileIcon' }, { id: 'fileName' },
    { id: 'fileSource' }, { id: 'fileError' }, { id: 'fileStats' },
  ],
  facets: [
    { id: 'layout', attribute: 'data-openbitfun-layout', values: ['default', 'compact'] },
    { id: 'operation', attribute: 'data-openbitfun-operation', values: ['write', 'delete', 'edit'] },
  ],
  states: [
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-openbitfun-state~="expanded"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-openbitfun-state~="error"]' } },
  ],
};
