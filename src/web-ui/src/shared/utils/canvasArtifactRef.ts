/**
 * Canvas artifact reference parsing.
 *
 * Mirrors `parse_canvas_artifact_ref` in
 * `src/crates/contracts/product-domains/src/canvas/reference.rs`. Keep both
 * implementations in sync: a reference this parser accepts but the host
 * rejects (or vice versa) surfaces to the user as a dead Canvas link.
 */

export const CANVAS_ARTIFACT_REF_SCHEME = 'bitfun-canvas';
export const CANVAS_ARTIFACT_REF_PREFIX = `${CANVAS_ARTIFACT_REF_SCHEME}://`;

export interface CanvasArtifactRef {
  sessionId: string;
  canvasId: string;
}

/** Matches `is_safe_canvas_ref_segment`: no separators, no Unicode control chars. */
function isSafeCanvasRefSegment(value: string): boolean {
  if (!value || value === '.' || value === '..') {
    return false;
  }
  // eslint-disable-next-line no-control-regex
  return !/[/\\\u0000-\u001f\u007f-\u009f]/.test(value);
}

function decodeSegment(value: string): string | null {
  try {
    return decodeURIComponent(value);
  } catch {
    return null;
  }
}

export function isCanvasArtifactRef(value: unknown): value is string {
  return typeof value === 'string' && value.startsWith(CANVAS_ARTIFACT_REF_PREFIX);
}

/**
 * Parses `bitfun-canvas://session/<sessionId>/canvas/<canvasId>`.
 * Returns null for anything the host would reject.
 */
export function parseCanvasArtifactRef(uri: unknown): CanvasArtifactRef | null {
  if (!isCanvasArtifactRef(uri)) {
    return null;
  }

  const parts = uri.slice(CANVAS_ARTIFACT_REF_PREFIX.length).split('/');
  if (parts.length !== 4 || parts[0] !== 'session' || parts[2] !== 'canvas') {
    return null;
  }

  const sessionId = decodeSegment(parts[1]);
  const canvasId = decodeSegment(parts[3]);
  if (sessionId === null || canvasId === null) {
    return null;
  }
  if (!isSafeCanvasRefSegment(sessionId) || !isSafeCanvasRefSegment(canvasId)) {
    return null;
  }

  return { sessionId, canvasId };
}

/** Session id owning the canvas, or null when the reference is unusable. */
export function canvasArtifactRefSessionId(uri: unknown): string | null {
  return parseCanvasArtifactRef(uri)?.sessionId ?? null;
}
