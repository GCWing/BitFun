export interface CanvasArtifactReferenceParts {
  sessionId: string;
  canvasId: string;
}

function isSafeCanvasArtifactSegment(value: string): boolean {
  return value.length > 0
    && value !== '.'
    && value !== '..'
    && ![...value].some((character) => {
      const codePoint = character.codePointAt(0) ?? 0;
      return character === '/' || character === '\\' || codePoint <= 31 || codePoint === 127;
    });
}

export function parseCanvasArtifactReference(reference: string): CanvasArtifactReferenceParts | null {
  const match = /^openbitfun-canvas:\/\/session\/([^/?#]+)\/canvas\/([^/?#]+)$/.exec(reference.trim());
  if (!match) return null;

  try {
    const sessionId = decodeURIComponent(match[1]);
    const canvasId = decodeURIComponent(match[2]);
    return isSafeCanvasArtifactSegment(sessionId) && isSafeCanvasArtifactSegment(canvasId)
      ? { sessionId, canvasId }
      : null;
  } catch {
    return null;
  }
}
