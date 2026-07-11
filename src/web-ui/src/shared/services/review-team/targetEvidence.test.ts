import { describe, expect, it } from 'vitest';
import { classifyReviewTargetFromFiles } from '../reviewTargetClassifier';
import {
  allowsReviewLiveRepositoryContext,
  buildGitRangeReviewTargetEvidence,
  buildPullRequestReviewTargetEvidence,
  buildUnknownReviewTargetEvidence,
  buildWorkspaceReviewTargetEvidence,
  stableReviewFingerprint,
} from './targetEvidence';

const CLEAN_STATUS = {
  staged: [],
  unstaged: [],
  untracked: [],
  conflicts: [],
  current_branch: 'feature',
  ahead: 0,
  behind: 0,
};

describe('Review target evidence', () => {
  it('keeps large diffs explicitly partial instead of implying fully consumed evidence', () => {
    const target = classifyReviewTargetFromFiles(['src/lib.rs'], 'slash_command_git_ref');
    const evidence = buildGitRangeReviewTargetEvidence({
      target,
      changedFiles: [{ path: 'src/lib.rs', status: 'modified' }],
      baseRevision: '1'.repeat(40),
      headRevision: '2'.repeat(40),
      workspaceHeadRevision: '2'.repeat(40),
      status: CLEAN_STATUS,
      diff: '+'.repeat(80_001),
    });

    expect(evidence.completeness).toBe('partial');
    expect(evidence.limitations).toContain('target_diff_budget_exceeded');
  });

  it('keeps a two-page diff complete when it remains within the total budget', () => {
    const target = classifyReviewTargetFromFiles(['src/lib.rs'], 'slash_command_git_ref');
    const evidence = buildGitRangeReviewTargetEvidence({
      target,
      changedFiles: [{ path: 'src/lib.rs', status: 'modified' }],
      baseRevision: '1'.repeat(40),
      headRevision: '2'.repeat(40),
      workspaceHeadRevision: '2'.repeat(40),
      status: CLEAN_STATUS,
      diff: '+'.repeat(60_000),
    });

    expect(evidence.completeness).toBe('complete');
    expect(evidence.limitations).not.toContain('target_diff_budget_exceeded');
  });

  it('does not penalize several independently small file diffs for aggregate size', () => {
    const target = classifyReviewTargetFromFiles(
      ['src/a.rs', 'src/b.rs'],
      'slash_command_git_ref',
    );
    const section = (path: string) =>
      `diff --git a/${path} b/${path}\n${'+change\n'.repeat(3_500)}`;
    const evidence = buildGitRangeReviewTargetEvidence({
      target,
      changedFiles: [
        { path: 'src/a.rs', status: 'modified' },
        { path: 'src/b.rs', status: 'modified' },
      ],
      baseRevision: '1'.repeat(40),
      headRevision: '2'.repeat(40),
      workspaceHeadRevision: '2'.repeat(40),
      status: CLEAN_STATUS,
      diff: `${section('src/a.rs')}${section('src/b.rs')}`,
    });

    expect(evidence.completeness).toBe('complete');
    expect(evidence.limitations).not.toContain('target_diff_requires_paging');
  });

  it('enables Git inspection only for a complete matching clean range', () => {
    const target = classifyReviewTargetFromFiles(['src/lib.rs'], 'slash_command_git_ref');
    const evidence = buildGitRangeReviewTargetEvidence({
      target,
      changedFiles: [{ path: 'src/lib.rs', status: 'modified' }],
      baseRevision: '1'.repeat(40),
      headRevision: '2'.repeat(40),
      workspaceHeadRevision: '2'.repeat(40),
      status: CLEAN_STATUS,
      diff: '-old\n+new',
    });

    expect(evidence).toMatchObject({
      source: 'git_range',
      completeness: 'complete',
      workspaceBinding: 'matching_clean',
    });
    expect(allowsReviewLiveRepositoryContext(evidence)).toBe(true);
  });

  it('marks a matching head dirty when target files have local changes', () => {
    const target = classifyReviewTargetFromFiles(['src/lib.rs'], 'slash_command_git_ref');
    const evidence = buildGitRangeReviewTargetEvidence({
      target,
      changedFiles: [{ path: 'src/lib.rs', status: 'modified' }],
      baseRevision: '1'.repeat(40),
      headRevision: '2'.repeat(40),
      workspaceHeadRevision: '2'.repeat(40),
      status: {
        ...CLEAN_STATUS,
        unstaged: [{ path: 'src/lib.rs', status: 'modified' }],
      },
      diff: '-old\n+new',
    });

    expect(evidence.workspaceBinding).toBe('matching_dirty');
    expect(evidence.limitations).toContain('workspace_has_local_changes');
    expect(allowsReviewLiveRepositoryContext(evidence)).toBe(false);
  });

  it('keeps a Git range partial when refs are not immutable commit ids', () => {
    const target = classifyReviewTargetFromFiles(['src/lib.rs'], 'slash_command_git_ref');
    const evidence = buildGitRangeReviewTargetEvidence({
      target,
      changedFiles: [{ path: 'src/lib.rs', status: 'modified' }],
      baseRevision: 'main',
      headRevision: 'feature',
      workspaceHeadRevision: 'feature',
      status: CLEAN_STATUS,
      diff: '-old\n+new',
    });

    expect(evidence.completeness).toBe('partial');
    expect(evidence.limitations).toContain('git_revision_unresolved');
    expect(allowsReviewLiveRepositoryContext(evidence)).toBe(false);
  });

  it('keeps an unresolved requested Git range fail-closed as a Git source', () => {
    const target = classifyReviewTargetFromFiles([], 'slash_command_git_ref');
    const evidence = buildUnknownReviewTargetEvidence(
      target,
      'git_range_resolution_failed',
    );

    expect(evidence).toMatchObject({
      source: 'git_range',
      completeness: 'unknown',
      workspaceBinding: 'unavailable',
    });
  });

  it('changes workspace fingerprint when untracked content changes', () => {
    const target = classifyReviewTargetFromFiles(['src/new.ts'], 'workspace_diff');
    const first = buildWorkspaceReviewTargetEvidence({
      target,
      baseRevision: '1'.repeat(40),
      diff: '',
      status: { ...CLEAN_STATUS, untracked: ['src/new.ts'] },
      untrackedContentFingerprints: { 'src/new.ts': 'aaaa' },
    });
    const second = buildWorkspaceReviewTargetEvidence({
      target,
      baseRevision: '1'.repeat(40),
      diff: '',
      status: { ...CLEAN_STATUS, untracked: ['src/new.ts'] },
      untrackedContentFingerprints: { 'src/new.ts': 'bbbb' },
    });

    expect(first.fingerprint).not.toBe(second.fingerprint);
  });

  it('keeps workspace evidence partial when untracked content cannot be read', () => {
    const target = classifyReviewTargetFromFiles(['src/new.ts'], 'workspace_diff');
    const evidence = buildWorkspaceReviewTargetEvidence({
      target,
      baseRevision: '1'.repeat(40),
      diff: '',
      status: { ...CLEAN_STATUS, untracked: ['src/new.ts'] },
      untrackedContentFingerprints: { 'src/new.ts': 'unavailable' },
    });

    expect(evidence.completeness).toBe('partial');
    expect(evidence.limitations).toContain('untracked_content_unavailable');
  });

  it('normalizes real Git status codes and requires the whole workspace to be clean', () => {
    const target = classifyReviewTargetFromFiles(['src/lib.rs'], 'slash_command_git_ref');
    const evidence = buildGitRangeReviewTargetEvidence({
      target,
      changedFiles: [{ path: 'src/lib.rs', status: 'modified' }],
      baseRevision: '1'.repeat(40),
      headRevision: '2'.repeat(40),
      workspaceHeadRevision: '2'.repeat(40),
      status: {
        ...CLEAN_STATUS,
        unstaged: [{ path: 'README.md', status: 'M' }],
      },
      diff: '-old\n+new',
    });

    expect(evidence.files[0].status).toBe('modified');
    expect(evidence.workspaceBinding).toBe('matching_dirty');
    expect(allowsReviewLiveRepositoryContext(evidence)).toBe(false);
  });

  it('normalizes Rust GitStatus codes in workspace evidence', () => {
    const target = classifyReviewTargetFromFiles(['src/lib.rs'], 'workspace_diff');
    const evidence = buildWorkspaceReviewTargetEvidence({
      target,
      baseRevision: '1'.repeat(40),
      diff: 'diff --git a/src/lib.rs b/src/lib.rs\n-old\n+new',
      status: {
        ...CLEAN_STATUS,
        staged: [{ path: 'src/lib.rs', status: 'M' }],
      },
    });

    expect(evidence.completeness).toBe('complete');
    expect(evidence.files[0].status).toBe('modified');
  });

  it('keeps excluded target files out of reviewer evidence', () => {
    const target = classifyReviewTargetFromFiles(
      ['src/lib.rs', 'generated/output.js'],
      'slash_command_git_ref',
    );
    target.files[1].excluded = true;
    const evidence = buildGitRangeReviewTargetEvidence({
      target,
      changedFiles: [
        { path: 'src/lib.rs', status: 'modified' },
        { path: 'generated/output.js', status: 'modified' },
      ],
      baseRevision: '1'.repeat(40),
      headRevision: '2'.repeat(40),
      workspaceHeadRevision: '2'.repeat(40),
      status: CLEAN_STATUS,
      diff: '-old\n+new',
    });

    expect(evidence.files.map((file) => file.path)).toEqual(['src/lib.rs']);
  });

  it('marks binary Git range evidence unavailable', () => {
    const target = classifyReviewTargetFromFiles(['image.png'], 'slash_command_git_ref');
    const evidence = buildGitRangeReviewTargetEvidence({
      target,
      changedFiles: [{ path: 'image.png', status: 'modified' }],
      baseRevision: '1'.repeat(40),
      headRevision: '2'.repeat(40),
      workspaceHeadRevision: '2'.repeat(40),
      status: CLEAN_STATUS,
      diff: 'diff --git a/image.png b/image.png\nBinary files a/image.png and b/image.png differ',
    });

    expect(evidence.completeness).toBe('partial');
    expect(evidence.files[0].completeness).toBe('unavailable');
    expect(allowsReviewLiveRepositoryContext(evidence)).toBe(true);
  });

  it('fails closed when a quoted binary diff header cannot be assigned safely', () => {
    const target = classifyReviewTargetFromFiles(['image with space.png'], 'slash_command_git_ref');
    const evidence = buildGitRangeReviewTargetEvidence({
      target,
      changedFiles: [{ path: 'image with space.png', status: 'modified' }],
      baseRevision: '1'.repeat(40),
      headRevision: '2'.repeat(40),
      workspaceHeadRevision: '2'.repeat(40),
      status: CLEAN_STATUS,
      diff: 'diff --git "a/image with space.png" "b/image with space.png"\nBinary files "a/image with space.png" and "b/image with space.png" differ',
    });

    expect(evidence.completeness).toBe('partial');
    expect(evidence.limitations).toContain('binary_diff_unavailable');
  });

  it('uses a stronger target fingerprint than the former 32-bit collision pair', () => {
    const first = stableReviewFingerprint({ diff: 'diff-9dd34ab5-64378e17', untracked: {} });
    const second = stableReviewFingerprint({ diff: 'diff-6f31a893-1e5f55c2', untracked: {} });

    expect(first).toHaveLength(16);
    expect(first).not.toBe(second);
  });

  it('binds pull request evidence to provider identity without enabling live workspace reads', () => {
    const target = classifyReviewTargetFromFiles(['src/lib.rs'], 'pull_request');
    const evidence = buildPullRequestReviewTargetEvidence({
      target,
      baseRevision: '1'.repeat(40),
      headRevision: '2'.repeat(40),
      pullRequest: {
        remoteId: 'origin|https://github.com/example/repo.git',
        platform: 'github',
        host: 'github.com',
        projectPath: 'example/repo',
        pullRequestId: '42',
        number: 42,
        webUrl: 'https://github.com/example/repo/pull/42',
      },
      files: [{
        path: 'src/lib.rs',
        status: 'modified',
        diffAvailable: true,
      }],
    });

    expect(evidence.completeness).toBe('complete');
    expect(evidence.pullRequest?.pullRequestId).toBe('42');
    expect(allowsReviewLiveRepositoryContext(evidence)).toBe(false);
  });
});
