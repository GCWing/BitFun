import { describe, expect, it, vi, beforeEach } from 'vitest';
import {
  countChangedLinesFromUnifiedDiff,
  resolveCurrentFileReviewChangeStats,
  resolveCurrentFileReviewSnapshot,
  resolveSlashCommandReviewTarget,
} from './targetResolver';
import { classifyReviewTargetFromFiles } from '@/shared/services/reviewTargetClassifier';

const mockGitGetStatus = vi.fn();
const mockGitGetChangedFiles = vi.fn();
const mockGitGetDiff = vi.fn();
const mockGitResolveRevision = vi.fn();
const mockWorkspaceReadFile = vi.fn();
const mockWorkspaceGetFileMetadata = vi.fn();

vi.mock('@/infrastructure/api', () => ({
  gitAPI: {
    getStatus: (...args: any[]) => mockGitGetStatus(...args),
    getChangedFiles: (...args: any[]) => mockGitGetChangedFiles(...args),
    getDiff: (...args: any[]) => mockGitGetDiff(...args),
    resolveRevision: (...args: any[]) => mockGitResolveRevision(...args),
  },
  workspaceAPI: {
    readFileContent: (...args: any[]) => mockWorkspaceReadFile(...args),
    getFileMetadata: (...args: any[]) => mockWorkspaceGetFileMetadata(...args),
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
    mockGitResolveRevision.mockImplementation(async (_workspacePath: string, revision: string) => {
      if (revision.endsWith('^')) {
        return '1111111111111111111111111111111111111111';
      }
      return '2222222222222222222222222222222222222222';
    });
    mockWorkspaceReadFile.mockResolvedValue('');
    mockWorkspaceGetFileMetadata.mockResolvedValue({
      isFile: true,
      size: 1024,
    });
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

  it('binds explicit file targets to the current workspace evidence', async () => {
    mockGitGetStatus.mockResolvedValueOnce({
      staged: [{ path: 'src/web-ui/src/App.tsx', status: 'modified' }],
      unstaged: [{ path: 'src/crates/assembly/core/src/lib.rs', status: 'modified' }],
      untracked: [],
      conflicts: [],
      current_branch: 'main',
      ahead: 0,
      behind: 0,
    });
    mockGitGetChangedFiles.mockResolvedValue([
      { path: 'src/web-ui/src/App.tsx', status: 'modified' },
      { path: 'src/crates/assembly/core/src/lib.rs', status: 'modified' },
    ]);
    mockGitGetDiff.mockResolvedValue('+first\n-second\n');

    const result = await resolveSlashCommandReviewTarget(
      'src/web-ui/src/App.tsx src/crates/assembly/core/src/lib.rs for regressions',
      'D:\\workspace\\repo',
    );

    expect(mockGitGetStatus).toHaveBeenCalledWith(
      'D:\\workspace\\repo',
      'review_explicit_scope_snapshot',
    );
    expect(mockGitGetChangedFiles).toHaveBeenCalledWith('D:\\workspace\\repo', {
      source: 'HEAD',
      reviewSafe: true,
    });
    expect(result.target.source).toBe('slash_command_explicit_files');
    expect(result.changeStats).toEqual({
      fileCount: 2,
      totalLinesChanged: 2,
      lineCountSource: 'diff_stat',
    });
    expect(result.targetEvidence).toMatchObject({
      source: 'workspace',
      completeness: 'complete',
      workspaceBinding: 'matching_dirty',
    });
  });

  it('expands an explicit directory without widening outside it', async () => {
    mockGitGetStatus.mockResolvedValueOnce({
      staged: [{ path: 'src/inside.ts', status: 'modified' }],
      unstaged: [{ path: 'docs/outside.md', status: 'modified' }],
      untracked: [],
      conflicts: [],
      current_branch: 'main',
      ahead: 0,
      behind: 0,
    });
    mockGitGetChangedFiles.mockResolvedValue([
      { path: 'src/inside.ts', status: 'modified' },
      { path: 'docs/outside.md', status: 'modified' },
    ]);
    mockGitGetDiff.mockResolvedValue('+inside\n');

    const result = await resolveSlashCommandReviewTarget(
      './src/',
      'D:\\workspace\\repo',
    );

    expect(result.target.files.map((file) => file.normalizedPath)).toEqual([
      'src/inside.ts',
    ]);
    expect(mockGitGetDiff).toHaveBeenCalledWith('D:\\workspace\\repo', {
      source: 'HEAD',
      files: ['src/inside.ts'],
      reviewSafe: true,
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
      source: '1111111111111111111111111111111111111111',
      target: '2222222222222222222222222222222222222222',
      reviewSafe: true,
    });
    expect(mockGitGetDiff).toHaveBeenCalledWith('D:\\workspace\\repo', {
      source: '1111111111111111111111111111111111111111',
      target: '2222222222222222222222222222222222222222',
      reviewSafe: true,
    });
    expect(mockGitResolveRevision.mock.invocationCallOrder[0]).toBeLessThan(
      mockGitGetChangedFiles.mock.invocationCallOrder[0],
    );
    expect(result.target.source).toBe('slash_command_git_ref');
    expect(result.changeStats).toEqual({
      fileCount: 1,
      totalLinesChanged: 2,
      lineCountSource: 'diff_stat',
    });
    expect(result.targetEvidence).toMatchObject({
      source: 'git_range',
      baseRevision: '1111111111111111111111111111111111111111',
      headRevision: '2222222222222222222222222222222222222222',
      completeness: 'complete',
      workspaceBinding: 'matching_clean',
    });
  });

  it('rejects unsupported remote Git ranges before running an expensive remote diff', async () => {
    const result = await resolveSlashCommandReviewTarget(
      'main..feature',
      '/remote/workspace',
      'remote-1',
    );

    expect(mockGitResolveRevision).not.toHaveBeenCalled();
    expect(mockGitGetChangedFiles).not.toHaveBeenCalled();
    expect(mockGitGetDiff).not.toHaveBeenCalled();
    expect(result.targetEvidence).toMatchObject({
      source: 'git_range',
      completeness: 'unknown',
      workspaceBinding: 'unavailable',
      limitations: ['remote_exact_diff_unavailable'],
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
    expect(result.targetEvidence).toMatchObject({
      source: 'workspace',
      workspaceBinding: 'matching_dirty',
    });
  });

  it('preserves both paths and rename semantics for an edited workspace rename', async () => {
    mockGitGetStatus.mockResolvedValueOnce({
      staged: [{ path: 'src/new-name.ts', status: 'added' }],
      unstaged: [],
      untracked: [],
      conflicts: [],
      current_branch: 'main',
      ahead: 0,
      behind: 0,
    });
    mockGitGetChangedFiles.mockResolvedValueOnce([{
      path: 'src/new-name.ts',
      old_path: 'src/old-name.ts',
      status: 'renamed',
    }]);
    mockGitGetDiff.mockResolvedValueOnce([
      'diff --git a/src/old-name.ts b/src/new-name.ts',
      'similarity index 90%',
      'rename from src/old-name.ts',
      'rename to src/new-name.ts',
      '@@ -1 +1 @@',
      '-old',
      '+new',
    ].join('\n'));

    const result = await resolveSlashCommandReviewTarget('', 'D:\\workspace\\repo');

    expect(mockGitGetChangedFiles).toHaveBeenCalledWith('D:\\workspace\\repo', {
      source: 'HEAD',
      reviewSafe: true,
    });
    expect(mockGitGetDiff).toHaveBeenCalledWith('D:\\workspace\\repo', {
      source: 'HEAD',
      files: ['src/old-name.ts', 'src/new-name.ts'],
      reviewSafe: true,
    });
    expect(result.target.files).toEqual([
      expect.objectContaining({
        normalizedPath: 'src/new-name.ts',
        normalizedOldPath: 'src/old-name.ts',
        status: 'renamed',
      }),
    ]);
    expect(result.targetEvidence.files).toEqual([
      expect.objectContaining({
        path: 'src/new-name.ts',
        previousPath: 'src/old-name.ts',
        status: 'renamed',
      }),
    ]);
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
    );

    expect(mockWorkspaceReadFile).toHaveBeenCalledWith(
      'D:/workspace/project/src/new.ts',
    );
    expect(stats).toEqual({
      fileCount: 1,
      totalLinesChanged: 3,
      lineCountSource: 'diff_stat',
    });
  });

  it('normalizes absolute session file paths inside the workspace boundary', async () => {
    mockGitGetStatus.mockResolvedValueOnce({
      staged: [{ path: 'src/session.ts', status: 'modified' }],
      unstaged: [],
      untracked: [],
      conflicts: [],
      current_branch: 'main',
      ahead: 0,
      behind: 0,
    });
    mockGitGetChangedFiles.mockResolvedValueOnce([
      { path: 'src/session.ts', status: 'modified' },
    ]);
    mockGitGetDiff.mockResolvedValueOnce('diff --git a/src/session.ts b/src/session.ts\n+change');
    const target = classifyReviewTargetFromFiles(
      ['D:\\workspace\\repo\\src\\session.ts'],
      'session_files',
    );

    const snapshot = await resolveCurrentFileReviewSnapshot(
      'D:\\workspace\\repo',
      target,
    );

    expect(snapshot.target.files[0].normalizedPath).toBe('src/session.ts');
    expect(snapshot.targetEvidence.files[0].path).toBe('src/session.ts');
    expect(mockGitGetDiff).toHaveBeenCalledWith('D:\\workspace\\repo', {
      source: 'HEAD',
      files: ['src/session.ts'],
      reviewSafe: true,
    });
  });

  it('fails closed for absolute session paths outside the workspace', async () => {
    const target = classifyReviewTargetFromFiles(
      ['D:\\other\\outside.ts'],
      'session_files',
    );

    const snapshot = await resolveCurrentFileReviewSnapshot(
      'D:\\workspace\\repo',
      target,
    );

    expect(mockGitGetStatus).not.toHaveBeenCalled();
    expect(mockGitGetDiff).not.toHaveBeenCalled();
    expect(snapshot.targetEvidence.completeness).toBe('unknown');
    expect(snapshot.targetEvidence.limitations).toContain('target_path_outside_workspace');
  });

  it('marks an untracked symlink unavailable without reading its target', async () => {
    mockGitGetStatus.mockResolvedValueOnce({
      staged: [],
      unstaged: [],
      untracked: ['leak.txt'],
      conflicts: [],
      current_branch: 'main',
      ahead: 0,
      behind: 0,
    });
    mockWorkspaceGetFileMetadata.mockResolvedValueOnce({
      isFile: true,
      isSymlink: true,
      size: 20,
    });
    const target = classifyReviewTargetFromFiles(['leak.txt'], 'workspace_diff');

    const snapshot = await resolveCurrentFileReviewSnapshot('/workspace', target);

    expect(mockWorkspaceReadFile).not.toHaveBeenCalled();
    expect(snapshot.targetEvidence.completeness).toBe('partial');
    expect(snapshot.targetEvidence.limitations).toContain('untracked_content_unavailable');
  });

  it('caps untracked snapshot IO before quality selection', async () => {
    const paths = Array.from({ length: 40 }, (_, index) => `generated/file-${index}.txt`);
    mockGitGetStatus.mockResolvedValueOnce({
      staged: [],
      unstaged: [],
      untracked: paths,
      conflicts: [],
      current_branch: 'main',
      ahead: 0,
      behind: 0,
    });
    mockWorkspaceGetFileMetadata.mockResolvedValue({
      isFile: true,
      size: 1024,
    });
    mockWorkspaceReadFile.mockResolvedValue('content\n');
    const target = classifyReviewTargetFromFiles(paths, 'workspace_diff');

    const snapshot = await resolveCurrentFileReviewSnapshot('/workspace', target);

    expect(mockWorkspaceGetFileMetadata).toHaveBeenCalledTimes(32);
    expect(mockWorkspaceReadFile).toHaveBeenCalledTimes(32);
    expect(snapshot.targetEvidence.completeness).toBe('partial');
    expect(snapshot.targetEvidence.limitations).toContain('untracked_content_unavailable');
  });

  it('marks children of collapsed untracked directories unavailable without reading them', async () => {
    mockGitGetStatus.mockResolvedValueOnce({
      staged: [],
      unstaged: [],
      untracked: ['nested/'],
      conflicts: [],
      current_branch: 'main',
      ahead: 0,
      behind: 0,
    });
    const target = classifyReviewTargetFromFiles(
      ['nested/src/file.rs'],
      'session_files',
    );

    const snapshot = await resolveCurrentFileReviewSnapshot('/workspace', target);

    expect(mockWorkspaceGetFileMetadata).not.toHaveBeenCalled();
    expect(mockWorkspaceReadFile).not.toHaveBeenCalled();
    expect(snapshot.targetEvidence.completeness).toBe('partial');
    expect(snapshot.targetEvidence.files[0]).toMatchObject({
      path: 'nested/src/file.rs',
      status: 'unknown',
      completeness: 'partial',
    });
    expect(snapshot.targetEvidence.limitations).toContain(
      'untracked_directory_content_unavailable',
    );
    expect(snapshot.changeStats.lineCountSource).toBe('unknown');
  });

  it('preserves a Unix literal backslash through snapshot diff and untracked reads', async () => {
    const literalPath = 'src/literal\\name.rs';
    mockGitGetStatus.mockResolvedValueOnce({
      staged: [],
      unstaged: [],
      untracked: [literalPath],
      conflicts: [],
      current_branch: 'main',
      ahead: 0,
      behind: 0,
    });
    mockWorkspaceReadFile.mockResolvedValueOnce('content\n');
    const target = classifyReviewTargetFromFiles([literalPath], 'workspace_diff');

    const snapshot = await resolveCurrentFileReviewSnapshot('/workspace', target);

    expect(mockGitGetDiff).toHaveBeenCalledWith('/workspace', {
      source: 'HEAD',
      files: [literalPath],
      reviewSafe: true,
    });
    expect(mockWorkspaceGetFileMetadata).toHaveBeenCalledWith(
      `/workspace/${literalPath}`,
    );
    expect(mockWorkspaceReadFile).toHaveBeenCalledWith(`/workspace/${literalPath}`);
    expect(snapshot.targetEvidence.files[0].path).toBe(literalPath);
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
