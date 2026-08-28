import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const realtimeVoiceCallAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'realtime-voice-call',
  parts: [
    { id: 'trigger' },
    { id: 'root', propertyProfile: 'overlay', visualRole: 'popup' },
    { id: 'header' },
    { id: 'avatar' },
    { id: 'heading' },
    { id: 'liveIndicator' },
    { id: 'conversation' },
    { id: 'utterance' },
    { id: 'empty' },
    { id: 'task' },
    { id: 'meter' },
    { id: 'controls' },
    { id: 'control' },
  ],
  facets: [
    {
      id: 'phase',
      attribute: 'data-bf-phase',
      values: ['connecting', 'live', 'ending', 'error'],
    },
  ],
  states: [
    { id: 'idle', selector: { kind: 'self', suffix: '[data-bf-state~="idle"]' } },
    { id: 'connecting', selector: { kind: 'self', suffix: '[data-bf-state~="connecting"]' } },
    { id: 'live', selector: { kind: 'self', suffix: '[data-bf-state~="live"]' } },
    { id: 'ending', selector: { kind: 'self', suffix: '[data-bf-state~="ending"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-bf-state~="error"]' } },
    { id: 'starting', selector: { kind: 'self', suffix: '[data-bf-state~="starting"]' } },
    { id: 'working', selector: { kind: 'self', suffix: '[data-bf-state~="working"]' } },
    { id: 'usingTools', selector: { kind: 'self', suffix: '[data-bf-state~="using-tools"]' } },
    { id: 'waitingApproval', selector: { kind: 'self', suffix: '[data-bf-state~="waiting-approval"]' } },
    { id: 'needsInput', selector: { kind: 'self', suffix: '[data-bf-state~="needs-input"]' } },
    { id: 'finishing', selector: { kind: 'self', suffix: '[data-bf-state~="finishing"]' } },
    { id: 'stopping', selector: { kind: 'self', suffix: '[data-bf-state~="stopping"]' } },
  ],
};
