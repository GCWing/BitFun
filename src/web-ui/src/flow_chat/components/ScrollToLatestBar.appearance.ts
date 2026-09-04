import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const scrollToLatestBarAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'scroll-to-latest-bar',
  parts: [{ id: 'root' }, { id: 'gradient' }, { id: 'content' }, { id: 'button' }],
  facets: [{ id: 'input', attribute: 'data-openbitfun-input', values: ['active', 'expanded', 'collapsed'] }],
};
