import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const conversationModeSurfaceAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'conversation-mode-surface',
  parts: [
    { id: 'root' },
    { id: 'body' },
    { id: 'modeSwitch' },
    { id: 'modeSwitchButton' },
    { id: 'voiceModeIcon' },
  ],
  states: [
    { id: 'chat', selector: { kind: 'self', suffix: '[data-openbitfun-state~="chat"]' } },
    { id: 'voice', selector: { kind: 'self', suffix: '[data-openbitfun-state~="voice"]' } },
  ],
};
