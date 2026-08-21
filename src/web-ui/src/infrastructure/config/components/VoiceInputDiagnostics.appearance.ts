import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const voiceInputDiagnosticsAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'voice-input-diagnostics',
  parts: [
    { id: 'root' }, { id: 'deviceControl' }, { id: 'deviceSelect' },
    { id: 'diagnosticAction' }, { id: 'level' }, { id: 'levelValue' },
    { id: 'recognitionTest' }, { id: 'note' }, { id: 'result' }, { id: 'error' },
  ],
  facets: [
    {
      id: 'phase',
      attribute: 'data-bf-phase',
      values: ['idle', 'preparing', 'recording', 'transcribing'],
    },
    { id: 'volume', attribute: 'data-bf-volume', values: ['silent', 'low', 'normal'] },
  ],
  states: [
    { id: 'testingRecognition', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-bf-state~="testing-recognition"]' } },
    { id: 'error', selector: { kind: 'ancestorPart', part: 'root', suffix: '[data-bf-state~="error"]' } },
  ],
};
