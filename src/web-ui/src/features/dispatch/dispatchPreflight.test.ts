import { describe, expect, it } from 'vitest';
import { isDispatchWorkspaceReady } from './dispatchPreflight';

describe('dispatch preflight', () => {
  it('accepts only the exact probed target workspace', () => {
    const workspace = {
      path: '/srv/app',
      exists: true,
      isDirectory: true,
      isGitRepository: true,
    };
    expect(isDispatchWorkspaceReady('/srv/app', workspace)).toBe(true);
    expect(isDispatchWorkspaceReady('/srv/other', workspace)).toBe(false);
  });
});
