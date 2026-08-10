import { describe, expect, it } from 'vitest';
import {
  isRemoteSessionScope,
  normalizeRemoteSessionScope,
} from './remoteSessionScope';

describe('remoteSessionScope', () => {
  it.each(['localhost', 'localhost:22', '127.0.0.1', '127.0.0.1:22', '::1', '[::1]:22'])(
    'drops local host %s when no remote connection exists',
    (remoteSshHost) => {
      expect(normalizeRemoteSessionScope(undefined, remoteSshHost)).toEqual({});
      expect(isRemoteSessionScope(undefined, remoteSshHost)).toBe(false);
    },
  );

  it('preserves localhost when a remote connection disambiguates the scope', () => {
    expect(normalizeRemoteSessionScope(' connection-1 ', ' localhost ')).toEqual({
      remoteConnectionId: 'connection-1',
      remoteSshHost: 'localhost',
    });
    expect(isRemoteSessionScope('connection-1', 'localhost')).toBe(true);
  });

  it('preserves a legacy non-local host without a connection id', () => {
    expect(normalizeRemoteSessionScope(undefined, ' legacy.example ')).toEqual({
      remoteSshHost: 'legacy.example',
    });
    expect(isRemoteSessionScope(undefined, 'legacy.example')).toBe(true);
  });
});
