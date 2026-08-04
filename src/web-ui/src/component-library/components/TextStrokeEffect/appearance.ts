import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';

export const textStrokeEffectAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'text-stroke-effect',
  parts: [{ id: 'root' }, { id: 'outline' }, { id: 'animated' }, { id: 'gradient' }],
};
