import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const composerVoiceInputAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'composer-voice-input',
  parts: [
    { id: 'root' }, { id: 'control' }, { id: 'pill' }, { id: 'setupPill' },
    { id: 'setupMessage' }, { id: 'status' },
    { id: 'time' }, { id: 'timeline' }, { id: 'timelineBar' }, { id: 'divider' },
    { id: 'action' },
  ],
  facets: [
    { id: 'phase', attribute: 'data-openbitfun-phase', values: ['idle', 'setup', 'downloading', 'preparing', 'recording', 'transcribing'] },
    { id: 'action', attribute: 'data-openbitfun-action', values: ['install', 'dismiss', 'cancel', 'transcribe', 'send'] },
  ],
  states: [
    { id: 'active', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-openbitfun-state~="active"]' } },
    { id: 'lowVolume', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-openbitfun-state~="low-volume"]' } },
    { id: 'disabled', selector: { kind: 'self', suffix: '[data-openbitfun-state~="disabled"]' } },
  ],
};
