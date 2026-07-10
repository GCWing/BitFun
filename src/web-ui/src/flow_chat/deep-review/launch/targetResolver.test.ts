import { describe, expect, it, vi, beforeEach } from 'vitest';
import {
  countChangedLinesFromUnifiedDiff,
  resolveCurrentFileReviewChangeStats,
  resolveSlashCommandReviewTarget,
} from './targetResolver';

const mockGitGetStatus = vi.fn();
const mockGitGetChangedFiles = vi.fn();
const mockGitGetDiff = vi.fn();
const mockWorkspaceReadFile = vi.fn();

vi.mock('@/infrastructure/api', () => ({
  gitAPI: {
    getStatus: (...args: any[]) => mockGitGetStatus(...args),
    getChangedFiles: (...args: any[]) => mockGitGetChangedFiles(...args),
    getDiff: (...args: any[]) => mockGitGetDiff(...args),
  },
  workspaceAPI: {
    readFileContent: (...args: any[]) => mockWorkspaceReadFile(...args),
  },
}));

describe('Deep Review target resolver', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGitGetStatus.mockResolvedValue({
      staged: [],
      unstaged: [],
      untracked: [],
      conflicts: [],
      current_branch: 'main',
      ahead: 0,
      behind: 0,
    });
    mockGitGetChangedFiles.mockResolvedValue([]);
    mockGitGetDiff.mockResolvedValue('');
    mockWorkspaceReadFile.mockResolvedValue('');
  });

  it('counts changed lines from unified diff without headers', () => {
    expect(countChangedLinesFromUnifiedDiff([
      'diff --git a/src/lib.rs b/src/lib.rs',
      '--- a/src/lib.rs',
      '+++ b/src/lib.rs',
      '@@ -1,2 +1,3 @@',
      '-old line',
      '+new line',
      '+another line',
    ].join('\n'))).toBe(3);
  });

  it('resolves explicit file targets before reading git state', async () => {
    const result = await resolveSlashCommandReviewTarget(
      'src/web-ui/src/App.tsx src/crates/assembly/core/src/lib.rs for regressions',
      'D:\\workspace\\repo',
    );

    expect(mockGitGetStatus).not.toHaveBeenCalled();
    expect(mockGitGetChangedFiles).not.toHaveBeenCalled();
    expect(result.target.source).toBe('slash_command_explicit_files');
    expect(result.changeStats).toEqual({
      fileCount: 2,
      lineCountSource: 'unknown',
    });
  });

  it('resolves commit targets using changed files and diff stats', async () => {
    mockGitGetChangedFiles.mockResolvedValueOnce([
      { path: 'src/new.ts', old_path: 'src/old.ts' },
    ]);
    mockGitGetDiff.mockResolvedValueOnce([
      'diff --git a/src/new.ts b/src/new.ts',
      '@@ -1 +1 @@',
      '-old',
      '+new',
    ].join('\n'));

    const result = await resolveSlashCommandReviewTarget(
      'review commit abc123',
      'D:\\workspace\\repo',
    );

    expect(mockGitGetChangedFiles).toHaveBeenCalledWith('D:\\workspace\\repo', {
      source: 'abc123^',
      target: 'abc123',
    });
    expect(result.target.source).toBe('slash_command_git_ref');
    expect(result.changeStats).toEqual({
      fileCount: 2,
      totalLinesChanged: 2,
      lineCountSource: 'diff_stat',
    });
  });

  it('resolves empty slash-command focus from workspace diff', async () => {
    mockGitGetStatus.mockResolvedValueOnce({
      staged: [{ path: 'src/staged.ts', status: 'modified' }],
      unstaged: [],
      untracked: ['src/new.ts'],
      conflicts: [],
      current_branch: 'main',
      ahead: 0,
      behind: 0,
    });

    const result = await resolveSlashCommandReviewTarget('', 'D:\\workspace\\repo');

    expect(mockGitGetStatus).toHaveBeenCalledWith('D:\\workspace\\repo', 'deep_review_target_resolver');
    expect(result.target.source).toBe('workspace_diff');
    expect(result.changeStats).toEqual({
      fileCount: 2,
      lineCountSource: 'unknown',
    });
  });

  it('counts untracked file content in current file-scoped change stats', async () => {
    mockGitGetStatus.mockResolvedValueOnce({
      staged: [],
      unstaged: [],
      untracked: ['src/new.ts'],
      conflicts: [],
      current_branch: 'main',
      ahead: 0,
      behind: 0,
    });
    mockWorkspaceReadFile.mockResolvedValueOnce('one\ntwo\nthree\n');
    const target = {
      source: 'session_files' as const,
      resolution: 'resolved' as const,
      files: [{
        path: 'src/new.ts',
        normalizedPath: 'src/new.ts',
        status: 'modified' as const,
        source: 'session_files' as const,
        tags: [],
      }],
      tags: [],
      warnings: [],
    };

    const stats = await resolveCurrentFileReviewChangeStats(
      'D:/workspace/project',
      target,
      undefined,
      'remote-connection-1',
    );

    expect(mockWorkspaceReadFile).toHaveBeenCalledWith(
      'D:/workspace/project/src/new.ts',
      undefined,
      'remote-connection-1',
    );
    expect(stats).toEqual({
      fileCount: 1,
      totalLinesChanged: 3,
      lineCountSource: 'diff_stat',
    });
  });

  it('treats free-form focus as guidance while still resolving workspace changes', async () => {
    mockGitGetStatus.mockResolvedValueOnce({
      staged: [{ path: 'src/crates/assembly/core/src/agentic/auth.rs', status: 'modified' }],
      unstaged: [],
      untracked: [],
      conflicts: [],
      current_branch: 'main',
      ahead: 0,
      behind: 0,
    });

    const result = await resolveSlashCommandReviewTarget(
      'focus on authentication and authorization risks',
      'D:\\workspace\\repo',
    );

    expect(mockGitGetStatus).toHaveBeenCalledWith('D:\\workspace\\repo', 'deep_review_target_resolver');
    expect(result.target.source).toBe('workspace_diff');
    expect(result.target.resolution).toBe('resolved');
    expect(result.target.files.map((file) => file.normalizedPath)).toContain(
      'src/crates/assembly/core/src/agentic/auth.rs',
    );
  });
});
