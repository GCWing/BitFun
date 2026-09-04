import { describe, expect, it } from 'vitest';
import {
  BASE_DISPATCH_CAPABILITIES,
  DISPATCH_PROTOCOL_VERSION,
} from './dispatchPreflight';

describe('dispatch preflight', () => {
  it('requires protocol v6 product identity, reasoning, and Git worktree delivery', () => {
    expect(DISPATCH_PROTOCOL_VERSION).toBe(6);
    expect(BASE_DISPATCH_CAPABILITIES).toEqual(expect.arrayContaining([
      'product_identity',
      'workspace_git_worktree',
      'workspace_git_bundle_upload',
      'workspace_git_sync',
      'reasoning_presets',
    ]));
    expect(BASE_DISPATCH_CAPABILITIES).not.toEqual(expect.arrayContaining([
      'workspace_snapshot_exact',
      'workspace_snapshot_chunked',
      'workspace_snapshot_cache',
      'workspace_result_bundle',
    ]));
  });
});
