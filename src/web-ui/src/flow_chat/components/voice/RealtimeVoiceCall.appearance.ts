import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const realtimeVoiceCallAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'realtime-voice-call',
  componentAttribute: 'data-openbitfun-product-component',
  parts: [
    { id: 'root' },
  ],
  facets: [
    {
      id: 'phase',
      attribute: 'data-openbitfun-phase',
      values: ['connecting', 'live', 'ending', 'error'],
    },
  ],
  states: [
    { id: 'idle', selector: { kind: 'self', suffix: '[data-openbitfun-state~="idle"]' } },
    { id: 'connecting', selector: { kind: 'self', suffix: '[data-openbitfun-state~="connecting"]' } },
    { id: 'live', selector: { kind: 'self', suffix: '[data-openbitfun-state~="live"]' } },
    { id: 'ending', selector: { kind: 'self', suffix: '[data-openbitfun-state~="ending"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-openbitfun-state~="error"]' } },
    { id: 'starting', selector: { kind: 'self', suffix: '[data-openbitfun-state~="starting"]' } },
    { id: 'working', selector: { kind: 'self', suffix: '[data-openbitfun-state~="working"]' } },
    { id: 'usingTools', selector: { kind: 'self', suffix: '[data-openbitfun-state~="using-tools"]' } },
    { id: 'waitingApproval', selector: { kind: 'self', suffix: '[data-openbitfun-state~="waiting-approval"]' } },
    { id: 'needsInput', selector: { kind: 'self', suffix: '[data-openbitfun-state~="needs-input"]' } },
    { id: 'finishing', selector: { kind: 'self', suffix: '[data-openbitfun-state~="finishing"]' } },
    { id: 'stopping', selector: { kind: 'self', suffix: '[data-openbitfun-state~="stopping"]' } },
  ],
};
