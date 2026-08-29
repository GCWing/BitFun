import { describe, expect, it } from 'vitest';

import {
  fallbackChangedPaths,
  mergeBranchSwitchFileStats,
  parseCheckoutOverwriteFailure,
  parseUnifiedDiffStats,
} from './branchSwitchFailure';

describe('parseCheckoutOverwriteFailure', () => {
  it('extracts only the files from a wrapped tracked-change checkout diagnostic', () => {
    const result = parseCheckoutOverwriteFailure([
      'Failed to checkout branch: Git command failed: error: Your local changes to the following files would be overwritten by checkout:',
      '\tSECURITY.md',
      '\tsrc/file with spaces.ts',
      'Please commit your changes or stash them before you switch branches.',
      'Aborting',
    ].join('\n'));

    expect(result).toEqual({
      kind: 'tracked',
      files: ['SECURITY.md', 'src/file with spaces.ts'],
    });
  });

  it('classifies untracked overwrite diagnostics for the same commit recovery', () => {
    const result = parseCheckoutOverwriteFailure([
      'error: The following untracked working tree files would be overwritten by checkout:',
      '\tgenerated/output.json',
      'Please move or remove them before you switch branches.',
      'Aborting',
    ].join('\n'));

    expect(result).toEqual({
      kind: 'untracked',
      files: ['generated/output.json'],
    });
  });

  it('does not turn unrelated checkout errors into a commit prompt', () => {
    expect(parseCheckoutOverwriteFailure(
      'error: you need to resolve your current index first',
    )).toBeNull();
  });
});

describe('parseUnifiedDiffStats', () => {
  it('counts staged and unstaged line changes per blocking file', () => {
    const unstaged = parseUnifiedDiffStats([
      'diff --git a/SECURITY.md b/SECURITY.md',
      'index 1..2 100644',
      '--- a/SECURITY.md',
      '+++ b/SECURITY.md',
      '@@ -1,2 +1,2 @@',
      '-old',
      '+new',
      ' keep',
      'diff --git a/src/added.ts b/src/added.ts',
      'new file mode 100644',
      '--- /dev/null',
      '+++ b/src/added.ts',
      '@@ -0,0 +1,2 @@',
      '+one',
      '+two',
    ].join('\n'));
    const staged = parseUnifiedDiffStats([
      'diff --git a/SECURITY.md b/SECURITY.md',
      'index 0..1 100644',
      '--- a/SECURITY.md',
      '+++ b/SECURITY.md',
      '@@ -1 +1 @@',
      '-earlier',
      '+old',
    ].join('\n'));

    const merged = mergeBranchSwitchFileStats(unstaged, staged);
    expect(merged.get('SECURITY.md')).toEqual({ additions: 2, deletions: 2 });
    expect(merged.get('src/added.ts')).toEqual({ additions: 2, deletions: 0 });
  });
});

describe('fallbackChangedPaths', () => {
  it('deduplicates paths across status buckets while preserving order', () => {
    expect(fallbackChangedPaths({
      staged: [{ path: 'a.ts' }],
      unstaged: [{ path: 'a.ts' }, { path: 'b.ts' }],
      untracked: ['c.ts'],
      conflicts: ['b.ts', 'd.ts'],
    })).toEqual(['a.ts', 'b.ts', 'c.ts', 'd.ts']);
  });
});
