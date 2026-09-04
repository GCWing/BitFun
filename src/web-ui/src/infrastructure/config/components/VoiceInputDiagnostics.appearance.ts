import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const voiceInputDiagnosticsAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'voice-input-diagnostics',
  parts: [
    { id: 'root' }, { id: 'deviceControl' }, { id: 'deviceSelect' },
    { id: 'diagnosticAction' }, { id: 'feedback' }, { id: 'result' }, { id: 'error' },
  ],
  facets: [
    {
      id: 'phase',
      attribute: 'data-openbitfun-phase',
      values: ['idle', 'preparing', 'recording', 'transcribing'],
    },
  ],
  states: [
    { id: 'testingRecognition', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-openbitfun-state~="testing-recognition"]' } },
    { id: 'error', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-openbitfun-state~="error"]' } },
  ],
};
