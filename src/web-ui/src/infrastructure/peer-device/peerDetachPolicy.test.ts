import { describe, expect, it } from 'vitest';
import { shouldSurfacePeerDetachFailure } from './peerDetachPolicy';

describe('peer detach failure policy', () => {
  it('surfaces explicit exits and keeps automatic recovery non-throwing', () => {
    expect(shouldSurfacePeerDetachFailure()).toBe(true);
    expect(shouldSurfacePeerDetachFailure('manual')).toBe(true);
    expect(shouldSurfacePeerDetachFailure('switch')).toBe(true);
    expect(shouldSurfacePeerDetachFailure('peer_offline')).toBe(false);
    expect(shouldSurfacePeerDetachFailure('rpc_failures')).toBe(false);
  });
});
