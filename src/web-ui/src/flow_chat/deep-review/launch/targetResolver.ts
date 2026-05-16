import { gitAPI } from '@/infrastructure/api';
import type { GitDiffParams } from '@/infrastructure/api/service-api/GitAPI';
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
): Promise<ReviewTeamChangeStats> {
  return resolveGitDiffChangeStats(workspacePath, { source: 'HEAD' }, target);
}

export async function resolveSlashCommandReviewTarget(
  commandFocus: string,
  workspacePath?: string,
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

  if (!commandFocus && workspacePath) {
    try {
      const status = await gitAPI.getStatus(workspacePath);
      const target = classifyReviewTargetFromFiles(
        collectWorkspaceDiffFilePaths(status),
        'workspace_diff',
      );
      const changeStats = await resolveWorkspaceDiffChangeStats(
        workspacePath,
        target,
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
