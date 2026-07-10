import { gitAPI, workspaceAPI } from '@/infrastructure/api';
import type { GitDiffParams, GitStatus } from '@/infrastructure/api/service-api/GitAPI';
import type { ReviewTeamChangeStats } from '@/shared/services/reviewTeamService';
import {
  classifyReviewTargetFromFiles,
  createUnknownReviewTargetClassification,
  type ReviewTargetClassification,
} from '@/shared/services/reviewTargetClassifier';
import { createLogger } from '@/shared/utils/logger';
import {
  collectChangedFilePaths,
  collectWorkspaceDiffFilePaths,
  extractExplicitReviewFilePaths,
  parseSlashCommandGitTarget,
} from './commandParser';

const log = createLogger('DeepReviewService');

export interface ResolvedDeepReviewTarget {
  target: ReviewTargetClassification;
  changeStats: ReviewTeamChangeStats;
}

function countReviewTargetFiles(target: ReviewTargetClassification): number {
  return target.files.filter((file) => !file.excluded).length;
}

export function buildUnknownChangeStats(
  target: ReviewTargetClassification,
): ReviewTeamChangeStats {
  return {
    fileCount: countReviewTargetFiles(target),
    lineCountSource: 'unknown',
  };
}

export function countChangedLinesFromUnifiedDiff(diff: string): number | undefined {
  if (!diff.trim()) {
    return undefined;
  }

  let changedLines = 0;
  for (const line of diff.split(/\r?\n/)) {
    if (
      (line.startsWith('+') && !/^\+\+\+\s/.test(line)) ||
      (line.startsWith('-') && !/^---\s/.test(line))
    ) {
      changedLines += 1;
    }
  }

  return changedLines;
}

function buildDiffChangeStats(
  target: ReviewTargetClassification,
  totalLinesChanged: number | undefined,
): ReviewTeamChangeStats {
  if (totalLinesChanged === undefined) {
    return buildUnknownChangeStats(target);
  }

  return {
    fileCount: countReviewTargetFiles(target),
    totalLinesChanged,
    lineCountSource: 'diff_stat',
  };
}

async function resolveGitDiffChangeStats(
  workspacePath: string,
  params: GitDiffParams,
  target: ReviewTargetClassification,
): Promise<ReviewTeamChangeStats> {
  try {
    const diff = await gitAPI.getDiff(workspacePath, params);
    return buildDiffChangeStats(target, countChangedLinesFromUnifiedDiff(diff));
  } catch (error) {
    log.warn('Failed to resolve Git diff stats for Deep Review target', {
      workspacePath,
      params,
      error,
    });
    return buildUnknownChangeStats(target);
  }
}

async function resolveWorkspaceDiffChangeStats(
  workspacePath: string,
  target: ReviewTargetClassification,
  status: GitStatus,
  remoteConnectionId?: string,
): Promise<ReviewTeamChangeStats> {
  return resolveCurrentFileReviewChangeStats(
    workspacePath,
    target,
    status,
    remoteConnectionId,
  );
}

function normalizePath(path: string): string {
  return path.replace(/\\/g, '/').replace(/^\.\//, '');
}

function workspaceFilePath(workspacePath: string, filePath: string): string {
  if (/^(?:[A-Za-z]:[\\/]|\/)/.test(filePath)) {
    return filePath.replace(/\\/g, '/');
  }
  return `${workspacePath.replace(/\\/g, '/').replace(/\/+$/, '')}/${normalizePath(filePath)}`;
}

function countTextFileLines(content: string): number {
  if (!content) {
    return 0;
  }
  const lines = content.split(/\r?\n/);
  if (lines.at(-1) === '') {
    lines.pop();
  }
  return lines.length;
}

export async function resolveCurrentFileReviewChangeStats(
  workspacePath: string,
  target: ReviewTargetClassification,
  knownStatus?: GitStatus,
  remoteConnectionId?: string,
): Promise<ReviewTeamChangeStats> {
  const targetFiles = target.files
    .filter((file) => !file.excluded)
    .map((file) => normalizePath(file.normalizedPath));
  try {
    const status = knownStatus
      ?? await gitAPI.getStatus(workspacePath, 'review_file_scope_stats');
    const diff = await gitAPI.getDiff(workspacePath, {
      source: 'HEAD',
      files: targetFiles,
    });
    const changedPaths = new Set(collectWorkspaceDiffFilePaths(status).map(normalizePath));
    const untrackedPaths = new Set(status.untracked.map(normalizePath));
    const conflictPaths = new Set(status.conflicts.map(normalizePath));
    const changedTargetFiles = targetFiles.filter((file) => changedPaths.has(file));

    if (changedTargetFiles.some((file) => conflictPaths.has(file))) {
      return buildUnknownChangeStats(target);
    }

    const trackedChangedFiles = changedTargetFiles.filter((file) => !untrackedPaths.has(file));
    const trackedLineCount = countChangedLinesFromUnifiedDiff(diff);
    if (trackedChangedFiles.length > 0 && trackedLineCount === undefined) {
      return buildUnknownChangeStats(target);
    }

    let totalLinesChanged = trackedLineCount ?? 0;
    for (const file of changedTargetFiles) {
      if (!untrackedPaths.has(file)) {
        continue;
      }
      const content = remoteConnectionId
        ? await workspaceAPI.readFileContent(
            workspaceFilePath(workspacePath, file),
            undefined,
            remoteConnectionId,
          )
        : await workspaceAPI.readFileContent(workspaceFilePath(workspacePath, file));
      totalLinesChanged += countTextFileLines(content);
    }

    return buildDiffChangeStats(target, totalLinesChanged);
  } catch (error) {
    log.warn('Failed to resolve file-scoped Review change stats', {
      workspacePath,
      targetFiles,
      error,
    });
    return buildUnknownChangeStats(target);
  }
}

export async function resolveSlashCommandReviewTarget(
  commandFocus: string,
  workspacePath?: string,
  remoteConnectionId?: string,
): Promise<ResolvedDeepReviewTarget> {
  const explicitFilePaths = extractExplicitReviewFilePaths(commandFocus);
  if (explicitFilePaths.length > 0) {
    const target = classifyReviewTargetFromFiles(
      explicitFilePaths,
      'slash_command_explicit_files',
    );
    return { target, changeStats: buildUnknownChangeStats(target) };
  }

  const gitTarget = parseSlashCommandGitTarget(commandFocus);
  if (gitTarget) {
    if (!workspacePath) {
      const target = createUnknownReviewTargetClassification('slash_command_git_ref');
      return { target, changeStats: buildUnknownChangeStats(target) };
    }

    try {
      const changedFiles = await gitAPI.getChangedFiles(workspacePath, gitTarget);
      const target = classifyReviewTargetFromFiles(
        collectChangedFilePaths(changedFiles),
        'slash_command_git_ref',
      );
      const changeStats = await resolveGitDiffChangeStats(
        workspacePath,
        gitTarget,
        target,
      );
      return { target, changeStats };
    } catch (error) {
      log.warn('Failed to resolve Git target for Deep Review target', {
        workspacePath,
        gitTarget,
        error,
      });
      const target = createUnknownReviewTargetClassification('slash_command_git_ref');
      return { target, changeStats: buildUnknownChangeStats(target) };
    }
  }

  if (workspacePath) {
    try {
      const status = await gitAPI.getStatus(workspacePath, 'deep_review_target_resolver');
      const target = classifyReviewTargetFromFiles(
        collectWorkspaceDiffFilePaths(status),
        'workspace_diff',
      );
      const changeStats = await resolveWorkspaceDiffChangeStats(
        workspacePath,
        target,
        status,
        remoteConnectionId,
      );
      return { target, changeStats };
    } catch (error) {
      log.warn('Failed to resolve workspace diff for Deep Review target', {
        workspacePath,
        error,
      });
    }
  }

  const target = createUnknownReviewTargetClassification(
    commandFocus ? 'manual_prompt' : 'unknown',
  );
  return { target, changeStats: buildUnknownChangeStats(target) };
}
