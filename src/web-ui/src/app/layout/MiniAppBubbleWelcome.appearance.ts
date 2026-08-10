import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const miniAppBubbleWelcomeAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'miniapp-bubble-welcome',
  parts: [
    { id: 'root' }, { id: 'icon' }, { id: 'eyebrow' }, { id: 'title' },
    { id: 'description' }, { id: 'workspace' }, { id: 'suggestions' },
    { id: 'suggestionsLabel' }, { id: 'suggestionsList' }, { id: 'suggestion' },
  ],
};
