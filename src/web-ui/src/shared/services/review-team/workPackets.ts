import {
  normalizeReviewPath,
  type ReviewTargetClassification,
} from '../reviewTargetClassifier';
import type {
  ReviewTeamChangeStats,
  ReviewTeamWorkPacket,
  ReviewTokenBudgetMode,
} from './types';
import { groupFilesByWorkspaceArea } from './pathMetadata';

export const MANAGED_REVIEW_AGENT_TYPE = 'ReviewGeneral';

export interface ManagedReviewWorkPacketOptions {
  target: ReviewTargetClassification;
  model: string;
  maxFilesPerBatch: number;
  maxBatches: number;
  maxParallelInstances: number;
  maxPlannedFiles?: number;
  timeoutSeconds: number;
  eligibleFilePaths?: string[];
}

export function buildManagedReviewWorkPackets(
  options: ManagedReviewWorkPacketOptions,
): ReviewTeamWorkPacket[] {
  const maxFilesPerBatch = Math.max(1, Math.floor(options.maxFilesPerBatch));
  const maxBatches = Math.max(1, Math.floor(options.maxBatches));
  const maxParallelInstances = Math.max(
    1,
    Math.floor(options.maxParallelInstances),
  );
  const files = options.target.files
    .filter((file) => !file.excluded)
    .map((file) => file.normalizedPath);
  const includedFileSet = new Set(files);
  const reviewableFiles = (options.eligibleFilePaths
    ? options.eligibleFilePaths
      .map(normalizeReviewPath)
      .filter((file, index, orderedFiles) =>
        includedFileSet.has(file) && orderedFiles.indexOf(file) === index)
    : files)
    .slice(0, options.maxPlannedFiles == null
      ? undefined
      : Math.max(1, Math.floor(options.maxPlannedFiles)));
  const groupChunks = groupFilesByWorkspaceArea(reviewableFiles)
    .map((bucket) => {
      const chunks: string[][] = [];
      for (let offset = 0; offset < bucket.files.length; offset += maxFilesPerBatch) {
        chunks.push(bucket.files.slice(offset, offset + maxFilesPerBatch));
      }
      return chunks;
    });
  const groups: string[][] = [];
  for (let chunkIndex = 0; groups.length < maxBatches; chunkIndex += 1) {
    let addedChunk = false;
    for (const chunks of groupChunks) {
      const chunk = chunks[chunkIndex];
      if (!chunk) {
        continue;
      }
      groups.push(chunk);
      addedChunk = true;
      if (groups.length >= maxBatches) {
        break;
      }
    }
    if (!addedChunk) {
      break;
    }
  }

  return groups.map((group, index) => ({
    packetId: `managed-review:batch-${index + 1}-of-${groups.length}`,
    phase: 'reviewer',
    launchBatch: Math.floor(index / maxParallelInstances) + 1,
    subagentId: MANAGED_REVIEW_AGENT_TYPE,
    displayName: `Review batch ${index + 1}`,
    roleName: 'General Review Worker',
    assignedScope: {
      kind: 'review_target',
      targetSource: options.target.source,
      targetResolution: options.target.resolution,
      targetTags: [...options.target.tags],
      fileCount: group.length,
      files: group,
      excludedFileCount: options.target.files.filter((file) => file.excluded).length,
      groupIndex: index + 1,
      groupCount: groups.length,
    },
    allowedTools: ['GetFileDiff', 'Read', 'Grep', 'Glob', 'LS'],
    timeoutSeconds: Math.max(1, Math.floor(options.timeoutSeconds)),
    requiredOutputFields: [
      'packet_id',
      'status',
      'covered_files',
      'findings',
      'coverage_notes',
    ],
    strategyLevel: 'deep',
    strategyDirective:
      'Review only the assigned files as one read-only shard. Return evidence-backed findings and exact coverage; do not modify files or broaden scope.',
    model: options.model,
  }));
}

// Legacy manifests may still contain work packets, but new strict reviews do
// not pre-schedule reviewer calls. This module now retains only the small
// normalization helpers used while building a launch manifest.
export function resolveMaxExtraReviewers(
  mode: ReviewTokenBudgetMode,
  eligibleExtraReviewerCount: number,
  strategyMaxExtraReviewers = Number.MAX_SAFE_INTEGER,
): number {
  if (mode === 'economy') {
    return 0;
  }
  return Math.min(eligibleExtraReviewerCount, strategyMaxExtraReviewers);
}

export function resolveChangeStats(
  target: ReviewTargetClassification,
  stats?: Partial<ReviewTeamChangeStats>,
): ReviewTeamChangeStats {
  const fileCount = Math.max(
    0,
    Math.floor(
      stats?.fileCount ??
        target.files.filter((file) => !file.excluded).length,
    ),
  );
  const totalLinesChanged =
    typeof stats?.totalLinesChanged === 'number' &&
    Number.isFinite(stats.totalLinesChanged)
      ? Math.max(0, Math.floor(stats.totalLinesChanged))
      : undefined;

  return {
    fileCount,
    ...(totalLinesChanged !== undefined ? { totalLinesChanged } : {}),
    lineCountSource:
      totalLinesChanged !== undefined
        ? stats?.lineCountSource ?? 'diff_stat'
        : 'unknown',
  };
}
