import { describe, expect, it } from 'vitest';
import {
  BASE_DISPATCH_CAPABILITIES,
  DISPATCH_PROTOCOL_VERSION,
} from './dispatchPreflight';

describe('dispatch preflight', () => {
  it('requires protocol v4 Git worktree delivery without a snapshot fallback', () => {
    expect(DISPATCH_PROTOCOL_VERSION).toBe(4);
    expect(BASE_DISPATCH_CAPABILITIES).toEqual(expect.arrayContaining([
      'workspace_git_worktree',
      'workspace_git_bundle_upload',
      'workspace_git_sync',
    ]));
    expect(BASE_DISPATCH_CAPABILITIES).not.toEqual(expect.arrayContaining([
      'workspace_snapshot_exact',
      'workspace_snapshot_chunked',
      'workspace_snapshot_cache',
      'workspace_result_bundle',
    ]));
  });
});
