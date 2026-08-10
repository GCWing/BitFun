import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const composerVoiceInputAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'composer-voice-input',
  parts: [
    { id: 'root' }, { id: 'control' }, { id: 'pill' }, { id: 'status' },
    { id: 'time' }, { id: 'timeline' }, { id: 'timelineBar' }, { id: 'divider' },
    { id: 'action' },
  ],
  facets: [
    { id: 'phase', attribute: 'data-bf-phase', values: ['idle', 'preparing', 'recording', 'transcribing'] },
    { id: 'action', attribute: 'data-bf-action', values: ['cancel', 'transcribe', 'send'] },
  ],
  states: [
    { id: 'active', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-bf-state~="active"]' } },
    { id: 'lowVolume', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-bf-state~="low-volume"]' } },
    { id: 'disabled', selector: { kind: 'self', suffix: '[data-bf-state~="disabled"]' } },
  ],
};
