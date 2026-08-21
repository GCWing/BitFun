import { describe, expect, it } from 'vitest';

import {
  canvasArtifactRefSessionId,
  isCanvasArtifactRef,
  parseCanvasArtifactRef,
} from './canvasArtifactRef';

// Fixtures mirror `canvas_contracts.rs` so the TS and Rust parsers stay aligned:
// a reference one side accepts and the other rejects is a dead Canvas link.
describe('parseCanvasArtifactRef', () => {
  it('decodes percent-encoded segments', () => {
    expect(parseCanvasArtifactRef('bitfun-canvas://session/session%201/canvas/canvas%201')).toEqual({
      sessionId: 'session 1',
      canvasId: 'canvas 1',
    });
  });

  it('parses plain segments', () => {
    expect(parseCanvasArtifactRef('bitfun-canvas://session/session_1/canvas/canvas_1')).toEqual({
      sessionId: 'session_1',
      canvasId: 'canvas_1',
    });
  });

  it.each([
    'bitfun-canvas://session/..%2Fother/canvas/canvas_1',
    'bitfun-canvas://session/../canvas/canvas_1',
    'bitfun-canvas://session/session_1/canvas/canvas%2Fwith%2Fslash',
    'bitfun-canvas://session/session%5C1/canvas/canvas_1',
  ])('rejects unsafe path segments: %s', (uri) => {
    expect(parseCanvasArtifactRef(uri)).toBeNull();
  });

  it.each([
    'file:///Users/user/project/canvas.tsx',
    'https://example.com/canvas',
    'bitfun-canvas://canvas/canvas_1',
    'bitfun-canvas://session/session_1/canvas/canvas_1/extra',
    'bitfun-canvas://session/session_1/artifact/canvas_1',
    'bitfun-canvas://session//canvas/canvas_1',
  ])('rejects malformed references: %s', (uri) => {
    expect(parseCanvasArtifactRef(uri)).toBeNull();
  });

  it('rejects invalid percent-encoding instead of throwing', () => {
    expect(parseCanvasArtifactRef('bitfun-canvas://session/%E0%A4%A/canvas/canvas_1')).toBeNull();
  });

  it('rejects non-string input', () => {
    expect(parseCanvasArtifactRef(undefined)).toBeNull();
    expect(parseCanvasArtifactRef(null)).toBeNull();
    expect(parseCanvasArtifactRef(42)).toBeNull();
  });
});

describe('isCanvasArtifactRef', () => {
  it('matches on the scheme only', () => {
    expect(isCanvasArtifactRef('bitfun-canvas://anything')).toBe(true);
    expect(isCanvasArtifactRef('bitfun-canvas:/session/a/canvas/b')).toBe(false);
    expect(isCanvasArtifactRef('https://example.com')).toBe(false);
  });
});

describe('canvasArtifactRefSessionId', () => {
  it('returns the decoded session id', () => {
    expect(canvasArtifactRefSessionId('bitfun-canvas://session/session%201/canvas/c'))
      .toBe('session 1');
  });

  it('returns null for an unusable reference', () => {
    expect(canvasArtifactRefSessionId('bitfun-canvas://session/../canvas/c')).toBeNull();
  });
});
