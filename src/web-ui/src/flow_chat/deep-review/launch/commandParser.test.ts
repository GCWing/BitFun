import { describe, expect, it } from 'vitest';
import {
  DEEP_REVIEW_SLASH_COMMAND,
  collectChangedFilePaths,
  collectWorkspaceDiffFilePaths,
  extractExplicitReviewFilePaths,
  getDeepReviewCommandFocus,
  getReviewSlashCommandIntent,
  hasUnresolvedPathLikeReviewFocus,
  isDeepReviewSlashCommand,
  isReviewSlashCommand,
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

  it('recognizes the unified Review command and derives intent without exposing levels', () => {
    expect(isReviewSlashCommand('/review')).toBe(true);
    expect(isReviewSlashCommand('/review focus on auth')).toBe(true);
    expect(isReviewSlashCommand('/review strict focus on auth')).toBe(true);
    expect(isReviewSlashCommand('/DeepReview focus on auth')).toBe(true);
    expect(isReviewSlashCommand('/reviewer')).toBe(false);

    expect(getReviewSlashCommandIntent('/review')).toBe('adaptive');
    expect(getReviewSlashCommandIntent('/review focus on auth')).toBe('adaptive');
    expect(getReviewSlashCommandIntent('/review strict focus on auth')).toBe('strict');
    expect(getReviewSlashCommandIntent('/DeepReview focus on auth')).toBe('strict');
  });

  it('strips the canonical command before target parsing', () => {
    expect(getDeepReviewCommandFocus('/review strict commit abc123')).toBe('commit abc123');
    expect(getDeepReviewCommandFocus('/review commit abc123')).toBe('commit abc123');
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

  it('recognizes root files, quoted paths with spaces, and explicit directories', () => {
    expect(
      extractExplicitReviewFilePaths(
        'README.md "docs/review notes.md" ./src/ and prose',
      ),
    ).toEqual([
      'README.md',
      'docs/review notes.md',
      './src/',
    ]);
  });

  it('treats untrailed directories, extensionless files, and file locations as explicit scope', () => {
    expect(
      extractExplicitReviewFilePaths(
        'src/web-ui Dockerfile README src/schema.proto:42:3',
      ),
    ).toEqual([
      'src/web-ui',
      'Dockerfile',
      'README',
      'src/schema.proto',
    ]);
  });

  it('recognizes common root build files and dotfiles as explicit scope', () => {
    expect(
      extractExplicitReviewFilePaths(
        '.gitignore BUILD WORKSPACE go.mod go.sum BUILD.bazel',
      ),
    ).toEqual([
      '.gitignore',
      'BUILD',
      'WORKSPACE',
      'go.mod',
      'go.sum',
      'BUILD.bazel',
    ]);
  });

  it('flags unresolved path-like focus instead of allowing workspace fallback', () => {
    expect(hasUnresolvedPathLikeReviewFocus('UNKNOWN_BUILD_FILE')).toBe(true);
    expect(hasUnresolvedPathLikeReviewFocus('config.custom')).toBe(true);
    expect(hasUnresolvedPathLikeReviewFocus('"config.custom"')).toBe(true);
    expect(hasUnresolvedPathLikeReviewFocus('"custombuild"')).toBe(true);
    expect(hasUnresolvedPathLikeReviewFocus('API security')).toBe(false);
    expect(hasUnresolvedPathLikeReviewFocus('focus on API v2.0 behavior')).toBe(false);
    expect(hasUnresolvedPathLikeReviewFocus('focus on authentication risks')).toBe(false);
  });

  it('does not misclassify commit refs or branch ranges containing slashes as files', () => {
    expect(
      extractExplicitReviewFilePaths(
        'commit feature/review main..feature/review',
      ),
    ).toEqual([]);
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

  it('preserves permissive legacy commit parsing independently of composition', () => {
    expect(parseSlashCommandGitTarget('please inspect commit abc123')).toEqual({
      source: 'abc123^',
      target: 'abc123',
    });
  });

  it('collects each renamed change once using its current path', () => {
    expect(
      collectChangedFilePaths([
        { path: 'src/new.ts', old_path: 'src/old.ts' },
        { path: 'src/new.ts' },
      ] as any),
    ).toEqual(['src/new.ts']);
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
