import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const tiptapEditorAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'tiptap-editor',
  parts: [
    { id: 'root' },
    { id: 'content' },
    { id: 'inlineAi' },
    { id: 'inlineAiSurface' },
    { id: 'inlineAiPanel' },
    { id: 'composer' },
    { id: 'quickActions' },
    { id: 'footer' },
  ],
};
