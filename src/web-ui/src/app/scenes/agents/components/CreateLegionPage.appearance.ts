import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const createLegionPageAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'create-legion-page',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'section' }, { id: 'actions' },
  ],
};
