/**
 * Classify account / relay errors for Online Devices UX.
 * Auth failures clear the local session; unreachable keeps it and shows retry.
 */

export function errorMessage(error: unknown): string {
  return (error instanceof Error ? error.message : String(error)).toLowerCase();
}

export function isAccountAuthFailure(error: unknown): boolean {
  const msg = errorMessage(error);
  return (
    msg.includes('401')
    || msg.includes('unauthorized')
    || msg.includes('invalid or expired token')
    || msg.includes('relay auth error')
  );
}

/** Network / transport failures talking to the relay (not auth rejection). */
export function isRelayUnreachable(error: unknown): boolean {
  if (isAccountAuthFailure(error)) {
    return false;
  }
  const msg = errorMessage(error);
  // WebSocket upgrades surface proxy/node instability as an HTTP handshake
  // status rather than a generic network error. Retry only transient statuses;
  // persistent 4xx responses (for example a missing /ws route) stay terminal.
  const hasTransientHttpStatus = /\b(?:408|425|429|500|502|503|504)\b/.test(msg);
  return (
    hasTransientHttpStatus
    || msg.includes('connection refused')
    || msg.includes('connect error')
    || msg.includes('connection reset')
    || msg.includes('timed out')
    || msg.includes('timeout')
    || msg.includes('failed to fetch')
    || msg.includes('error sending request')
    || msg.includes('network')
    || msg.includes('dns')
    || msg.includes('name or service not known')
    || msg.includes('no such host')
    || msg.includes('unreachable')
    || msg.includes('could not connect')
    || msg.includes('failed to connect')
    || msg.includes('empty reply')
    || msg.includes('connection closed')
    || msg.includes('bad gateway')
    || msg.includes('service unavailable')
    || msg.includes('gateway timeout')
    || msg.includes('temporarily unavailable')
    || msg.includes('unexpected eof')
    || msg.includes('unexpected end of file')
    || msg.includes('handshake not finished')
    || msg.includes('handshake incomplete')
    || msg.includes('broken pipe')
    || msg.includes('socket hang up')
  );
}
