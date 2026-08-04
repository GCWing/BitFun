import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const runtimeStatusSlotAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'runtime-status-slot',
  parts: [{ id: 'root' }, { id: 'content' }, { id: 'hint' }],
};
