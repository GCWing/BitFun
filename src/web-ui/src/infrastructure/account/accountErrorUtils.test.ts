import { describe, expect, it } from 'vitest';
import { isAccountAuthFailure, isRelayUnreachable } from './accountErrorUtils';

describe('isAccountAuthFailure', () => {
  it('detects HTTP 401 and token errors', () => {
    expect(isAccountAuthFailure(new Error('HTTP 401 Unauthorized'))).toBe(true);
    expect(isAccountAuthFailure(new Error('invalid or expired token'))).toBe(true);
    expect(isAccountAuthFailure(new Error('relay auth error: bad token'))).toBe(true);
  });

  it('rejects network failures', () => {
    expect(isAccountAuthFailure(new Error('error sending request: connection refused'))).toBe(false);
  });
});

describe('isRelayUnreachable', () => {
  it('detects common transport failures', () => {
    expect(isRelayUnreachable(new Error('error sending request: connection refused'))).toBe(true);
    expect(isRelayUnreachable(new Error('request timed out'))).toBe(true);
    expect(isRelayUnreachable(new Error('Failed to fetch'))).toBe(true);
    expect(isRelayUnreachable(new Error('dns resolution failed'))).toBe(true);
    expect(isRelayUnreachable(new Error('failed to connect to relay'))).toBe(true);
  });

  it('detects transient WebSocket upgrade and handshake failures', () => {
    expect(isRelayUnreachable(new Error('dial wss://relay/ws: HTTP error: 502 Bad Gateway')))
      .toBe(true);
    expect(isRelayUnreachable(new Error('HTTP 503 Service Unavailable'))).toBe(true);
    expect(isRelayUnreachable(new Error('WebSocket protocol error: Handshake not finished')))
      .toBe(true);
    expect(isRelayUnreachable(new Error('TLS stream ended with unexpected EOF'))).toBe(true);
  });

  it('does not treat auth failures as unreachable', () => {
    expect(isRelayUnreachable(new Error('HTTP 401 Unauthorized'))).toBe(false);
    expect(isRelayUnreachable(new Error('invalid or expired token'))).toBe(false);
  });

  it('does not treat unrelated application errors as unreachable', () => {
    expect(isRelayUnreachable(new Error('not logged in'))).toBe(false);
    expect(isRelayUnreachable(new Error('remote connect service not initialized'))).toBe(false);
    expect(isRelayUnreachable(new Error('dial wss://relay/ws: HTTP error: 404 Not Found')))
      .toBe(false);
    expect(isRelayUnreachable(new Error('invalid certificate: UnknownIssuer'))).toBe(false);
  });
});
