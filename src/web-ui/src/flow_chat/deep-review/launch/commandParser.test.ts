import { describe, expect, it } from 'vitest';
import {
  DEEP_REVIEW_SLASH_COMMAND,
  collectChangedFilePaths,
  collectWorkspaceDiffFilePaths,
  extractExplicitReviewFilePaths,
  getDeepReviewCommandFocus,
  isDeepReviewSlashCommand,
  parseSlashCommandGitTarget,
} from './commandParser';

describe('Deep Review launch command parser', () => {
  it('recognizes strict Review typed commands and compatibility aliases', () => {
    expect(DEEP_REVIEW_SLASH_COMMAND).toBe('/review strict');
    expect(isDeepReviewSlashCommand('/review strict')).toBe(true);
    expect(isDeepReviewSlashCommand('/review strict commit abc123')).toBe(true);
    expect(isDeepReviewSlashCommand('/review deep commit abc123')).toBe(false);
    expect(isDeepReviewSlashCommand('/DeepReview')).toBe(true);
    expect(isDeepReviewSlashCommand('/DeepReview review commit abc123')).toBe(true);
    expect(isDeepReviewSlashCommand('/deepreview review commit abc123')).toBe(true);
    expect(isDeepReviewSlashCommand('/review')).toBe(false);
    expect(isDeepReviewSlashCommand('/DeepReviewer review commit abc123')).toBe(false);
  });

  it('strips the canonical command before target parsing', () => {
    expect(getDeepReviewCommandFocus('/review strict commit abc123')).toBe('commit abc123');
    expect(getDeepReviewCommandFocus('/DeepReview review commit abc123')).toBe('review commit abc123');
    expect(getDeepReviewCommandFocus('/deepreview review commit abc123')).toBe('review commit abc123');
    expect(getDeepReviewCommandFocus('/DeepReview')).toBe('');
  });

  it('extracts explicit review file paths once and ignores prose tokens', () => {
    expect(
      extractExplicitReviewFilePaths(
        'please inspect `src/web-ui/src/App.tsx`, src/web-ui/src/App.tsx and src/crates/assembly/core/src/lib.rs for risk',
      ),
    ).toEqual([
      'src/web-ui/src/App.tsx',
      'src/crates/assembly/core/src/lib.rs',
    ]);
  });

  it('parses commit and range targets', () => {
    expect(parseSlashCommandGitTarget('review commit abc123 for regressions')).toEqual({
      source: 'abc123^',
      target: 'abc123',
    });
    expect(parseSlashCommandGitTarget('review main..feature/deep-review')).toEqual({
      source: 'main',
      target: 'feature/deep-review',
    });
    expect(parseSlashCommandGitTarget('review --flag docs only')).toBeNull();
  });

  it('collects unique changed paths including renamed sources', () => {
    expect(
      collectChangedFilePaths([
        { path: 'src/new.ts', old_path: 'src/old.ts' },
        { path: 'src/new.ts' },
      ] as any),
    ).toEqual(['src/new.ts', 'src/old.ts']);
  });

  it('collects workspace diff paths from all status buckets', () => {
    expect(
      collectWorkspaceDiffFilePaths({
        staged: [{ path: 'src/staged.ts', status: 'modified' }],
        unstaged: [{ path: 'src/unstaged.ts', status: 'modified' }],
        untracked: ['src/new.ts'],
        conflicts: ['src/conflict.ts'],
      } as any),
    ).toEqual([
      'src/staged.ts',
      'src/unstaged.ts',
      'src/new.ts',
      'src/conflict.ts',
    ]);
  });
});
