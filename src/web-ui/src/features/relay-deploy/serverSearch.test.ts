import { describe, expect, it } from 'vitest';

import type { SavedConnection, SSHConfigEntry } from '../ssh-remote/types';
import { buildRelayServerSearchState, getRelayConnectionHost } from './serverSearch';

const savedAliyun: SavedConnection = {
  id: 'ssh-root@47.99.85.183',
  name: 'Aliyun',
  host: '47.99.85.183',
  port: 22,
  username: 'root',
  authType: { type: 'Agent' },
};

const aliyunAlias: SSHConfigEntry = {
  host: 'aliyun_wgq',
  hostname: '47.99.85.183',
  port: 22,
  user: 'root',
  agent: true,
};

describe('buildRelayServerSearchState', () => {
  it('uses HostName for SSH config connections while retaining the alias for display', () => {
    expect(getRelayConnectionHost(aliyunAlias)).toBe('47.99.85.183');
  });

  it('keeps an SSH config alias even when a saved connection uses the same endpoint', () => {
    const state = buildRelayServerSearchState(
      [savedAliyun],
      [aliyunAlias],
      '',
      'aliyun_wgq',
    );

    expect(state.hasSSHConfigHosts).toBe(true);
    expect(state.filteredSSHConfigHosts).toEqual([aliyunAlias]);
  });

  it('keeps the SSH config section visible when a search has no matches', () => {
    const state = buildRelayServerSearchState(
      [savedAliyun],
      [aliyunAlias],
      '',
      'missing-server',
    );

    expect(state.hasSSHConfigHosts).toBe(true);
    expect(state.filteredSSHConfigHosts).toEqual([]);
  });
});
