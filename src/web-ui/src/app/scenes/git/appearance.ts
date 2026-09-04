import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';

export const gitAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'git',
  parts: [{ id: 'root' }, { id: 'content' }, { id: 'empty' }, { id: 'loading' }],
  facets: [{ id: 'view', attribute: 'data-openbitfun-view', values: ['hidden', 'repository', 'not-repository', 'trust-required', 'loading'] }],
};
