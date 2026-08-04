import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const chatInputPixelPetAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'chat-input-pixel-pet',
  parts: [
    { id: 'root' }, { id: 'stage' }, { id: 'svg' }, { id: 'silhouette' },
    { id: 'face' }, { id: 'rest' }, { id: 'analyze' }, { id: 'wait' },
    { id: 'work' }, { id: 'hover' }, { id: 'drag' }, { id: 'petdex' },
  ],
  facets: [
    { id: 'mood', attribute: 'data-bf-mood', values: ['rest', 'analyzing', 'waiting', 'working', 'hover', 'dragging'] },
    { id: 'layout', attribute: 'data-bf-layout', values: ['default', 'center', 'stopRight', 'petdex'] },
  ],
};
