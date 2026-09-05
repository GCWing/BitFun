import { describe, expect, it } from 'vitest';
import { parseCanvasArtifactReference } from './canvasArtifactReference';

describe('parseCanvasArtifactReference', () => {
  it('decodes valid session and Canvas ids', () => {
    expect(parseCanvasArtifactReference(
      'openbitfun-canvas://session/session%201/canvas/canvas%201',
    )).toEqual({ sessionId: 'session 1', canvasId: 'canvas 1' });
  });

  it.each([
    'https://example.com/canvas/1',
    'openbitfun-canvas://session/../canvas/canvas_1',
    'openbitfun-canvas://session/session_1/canvas/canvas%2Fescape',
    'openbitfun-canvas://session/session%5Cescape/canvas/canvas_1',
    'openbitfun-canvas://session/%ZZ/canvas/canvas_1',
  ])('rejects invalid or unsafe references: %s', (reference) => {
    expect(parseCanvasArtifactReference(reference)).toBeNull();
  });
});
