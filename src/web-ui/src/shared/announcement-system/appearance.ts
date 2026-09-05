import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const announcementAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'announcement',
  parts: [
    { id: 'stack' },
    { id: 'deck' },
    { id: 'ghost' },
    { id: 'modalBackdrop' },
    { id: 'modal' },
    { id: 'modalClose' },
    { id: 'modalPages' },
    { id: 'modalFooter' },
    { id: 'modalNavigation' },
  ],
  facets: [
    { id: 'depth', attribute: 'data-openbitfun-depth', values: ['1', '2'] },
  ],
};
