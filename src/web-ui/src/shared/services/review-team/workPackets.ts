import type { ReviewTargetClassification } from '../reviewTargetClassifier';
import {
  DEFAULT_REVIEW_TEAM_MODEL,
  JUDGE_WORK_PACKET_REQUIRED_OUTPUT_FIELDS,
  REVIEWER_WORK_PACKET_REQUIRED_OUTPUT_FIELDS,
} from './defaults';
import { toManifestMember } from './manifestMembers';
import { groupFilesByWorkspaceArea } from './pathMetadata';
import type {
  ReviewTeamChangeStats,
  ReviewTeamConcurrencyPolicy,
  ReviewTeamExecutionPolicy,
  ReviewTeamMember,
  ReviewTeamWorkPacket,
  ReviewTeamWorkPacketScope,
  ReviewTokenBudgetMode,
} from './types';

// Work packets are pure launch-plan metadata. They must not inspect file
// contents or make runtime retry/queue decisions.
export function resolveMaxExtraReviewers(
  mode: ReviewTokenBudgetMode,
  eligibleExtraReviewerCount: number,
): number {
  if (mode === 'economy') {
    return 0;
  }
  return eligibleExtraReviewerCount;
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

function buildWorkPacketScopeFromFiles(
  target: ReviewTargetClassification,
  files: string[],
  group?: { index: number; count: number },
): ReviewTeamWorkPacketScope {
  return {
    kind: 'review_target',
    targetSource: target.source,
    targetResolution: target.resolution,
    targetTags: [...target.tags],
    fileCount: files.length,
    files,
    excludedFileCount:
      target.files.length - target.files.filter((file) => !file.excluded).length,
    ...(group ? { groupIndex: group.index, groupCount: group.count } : {}),
  };
}

function buildWorkPacket(params: {
  member: ReviewTeamMember;
  phase: ReviewTeamWorkPacket['phase'];
  launchBatch: number;
  scope: ReviewTeamWorkPacketScope;
  timeoutSeconds: number;
}): ReviewTeamWorkPacket {
  const manifestMember = toManifestMember(params.member);
  const packetGroupSuffix =
    params.phase === 'reviewer' &&
    params.scope.groupIndex !== undefined &&
    params.scope.groupCount !== undefined
      ? `:group-${params.scope.groupIndex}-of-${params.scope.groupCount}`
      : '';

  return {
    packetId: `${params.phase}:${manifestMember.subagentId}${packetGroupSuffix}`,
    phase: params.phase,
    launchBatch: params.launchBatch,
    subagentId: manifestMember.subagentId,
    displayName: manifestMember.displayName,
    roleName: manifestMember.roleName,
    assignedScope: params.scope,
    allowedTools: [...params.member.allowedTools],
    timeoutSeconds: params.timeoutSeconds,
    requiredOutputFields:
      params.phase === 'judge'
        ? [...JUDGE_WORK_PACKET_REQUIRED_OUTPUT_FIELDS]
        : [...REVIEWER_WORK_PACKET_REQUIRED_OUTPUT_FIELDS],
    strategyLevel: manifestMember.strategyLevel,
    strategyDirective: manifestMember.strategyDirective,
    model: manifestMember.model || DEFAULT_REVIEW_TEAM_MODEL,
  };
}

function splitFilesIntoGroups(files: string[], groupCount: number): string[][] {
  if (groupCount <= 1) {
    return [files];
  }

  const groups: string[][] = [];
  let cursor = 0;
  for (let index = 0; index < groupCount; index += 1) {
    const remainingFiles = files.length - cursor;
    const remainingGroups = groupCount - index;
    const groupSize = Math.ceil(remainingFiles / remainingGroups);
    groups.push(files.slice(cursor, cursor + groupSize));
    cursor += groupSize;
  }
  return groups;
}

function splitFilesIntoModuleAwareGroups(
  files: string[],
  groupCount: number,
): string[][] {
  if (groupCount <= 1) {
    return [files];
  }

  const buckets = groupFilesByWorkspaceArea(files);
  if (buckets.length <= 1) {
    return splitFilesIntoGroups(files, groupCount);
  }

  if (buckets.length >= groupCount) {
    const groups = Array.from({ length: groupCount }, () => [] as string[]);
    const sortedBuckets = [...buckets].sort(
      (a, b) => b.files.length - a.files.length || a.index - b.index,
    );

    for (const bucket of sortedBuckets) {
      let targetIndex = 0;
      for (let index = 1; index < groups.length; index += 1) {
        if (groups[index].length < groups[targetIndex].length) {
          targetIndex = index;
        }
      }
      groups[targetIndex].push(...bucket.files);
    }

    return groups.filter((group) => group.length > 0);
  }

  const chunkCounts = buckets.map(() => 1);
  let remainingChunks = groupCount - buckets.length;
  while (remainingChunks > 0) {
    let targetBucketIndex = -1;
    let largestAverageChunkSize = 0;

    for (let index = 0; index < buckets.length; index += 1) {
      if (chunkCounts[index] >= buckets[index].files.length) {
        continue;
      }
      const averageChunkSize = buckets[index].files.length / chunkCounts[index];
      if (averageChunkSize > largestAverageChunkSize) {
        largestAverageChunkSize = averageChunkSize;
        targetBucketIndex = index;
      }
    }

    if (targetBucketIndex === -1) {
      break;
    }

    chunkCounts[targetBucketIndex] += 1;
    remainingChunks -= 1;
  }

  return buckets.flatMap((bucket, index) =>
    splitFilesIntoGroups(bucket.files, chunkCounts[index]),
  );
}

function effectiveMaxSameRoleInstances(params: {
  executionPolicy: ReviewTeamExecutionPolicy;
  concurrencyPolicy: ReviewTeamConcurrencyPolicy;
  reviewerMemberCount: number;
}): number {
  const reviewerMemberCount = Math.max(1, params.reviewerMemberCount);
  const maxPerRole = Math.floor(
    params.concurrencyPolicy.maxParallelInstances / reviewerMemberCount,
  );

  return Math.max(
    1,
    Math.min(params.executionPolicy.maxSameRoleInstances, Math.max(1, maxPerRole)),
  );
}

function resolveReviewerPacketScopes(
  target: ReviewTargetClassification,
  executionPolicy: ReviewTeamExecutionPolicy,
  concurrencyPolicy: ReviewTeamConcurrencyPolicy,
  reviewerMemberCount: number,
): ReviewTeamWorkPacketScope[] {
  const includedFiles = target.files
    .filter((file) => !file.excluded)
    .map((file) => file.normalizedPath);
  const shouldSplit =
    executionPolicy.reviewerFileSplitThreshold > 0 &&
    executionPolicy.maxSameRoleInstances > 1 &&
    includedFiles.length > executionPolicy.reviewerFileSplitThreshold;

  if (!shouldSplit) {
    return [buildWorkPacketScopeFromFiles(target, includedFiles)];
  }

  const maxSameRoleInstances = effectiveMaxSameRoleInstances({
    executionPolicy,
    concurrencyPolicy,
    reviewerMemberCount,
  });
  const groupCount = Math.min(
    maxSameRoleInstances,
    Math.ceil(includedFiles.length / executionPolicy.reviewerFileSplitThreshold),
  );
  if (groupCount <= 1) {
    return [buildWorkPacketScopeFromFiles(target, includedFiles)];
  }

  const fileGroups = splitFilesIntoModuleAwareGroups(includedFiles, groupCount);
  return fileGroups.map((files, index) =>
    buildWorkPacketScopeFromFiles(target, files, {
      index: index + 1,
      count: fileGroups.length,
    }),
  );
}

export function buildWorkPackets(params: {
  reviewerMembers: ReviewTeamMember[];
  judgeMember?: ReviewTeamMember;
  target: ReviewTargetClassification;
  executionPolicy: ReviewTeamExecutionPolicy;
  concurrencyPolicy: ReviewTeamConcurrencyPolicy;
}): ReviewTeamWorkPacket[] {
  const reviewerScopes = resolveReviewerPacketScopes(
    params.target,
    params.executionPolicy,
    params.concurrencyPolicy,
    params.reviewerMembers.length,
  );
  const fullScope = buildWorkPacketScopeFromFiles(
    params.target,
    params.target.files
      .filter((file) => !file.excluded)
      .map((file) => file.normalizedPath),
  );
  const reviewerSeeds = params.reviewerMembers.flatMap((member) =>
    reviewerScopes.map((scope) => ({ member, scope })),
  );
  const buildReviewerPacketsForSeeds = (
    seeds: typeof reviewerSeeds,
    firstLaunchBatch: number,
  ): ReviewTeamWorkPacket[] => seeds.map((seed, index) =>
    buildWorkPacket({
      member: seed.member,
      phase: 'reviewer',
      launchBatch:
        firstLaunchBatch +
        Math.floor(index / params.concurrencyPolicy.maxParallelInstances),
      scope: seed.scope,
      timeoutSeconds: params.executionPolicy.reviewerTimeoutSeconds,
    }),
  );
  const reviewerPackets = params.concurrencyPolicy.batchExtrasSeparately
    ? (() => {
      const coreReviewerPackets = buildReviewerPacketsForSeeds(
        reviewerSeeds.filter((seed) => seed.member.source === 'core'),
        1,
      );
      const extraFirstLaunchBatch = coreReviewerPackets.length > 0
        ? Math.max(...coreReviewerPackets.map((packet) => packet.launchBatch)) + 1
        : 1;
      return [
        ...coreReviewerPackets,
        ...buildReviewerPacketsForSeeds(
          reviewerSeeds.filter((seed) => seed.member.source === 'extra'),
          extraFirstLaunchBatch,
        ),
      ];
    })()
    : buildReviewerPacketsForSeeds(reviewerSeeds, 1);
  const finalReviewerBatch = reviewerPackets.reduce(
    (maxBatch, packet) => Math.max(maxBatch, packet.launchBatch),
    0,
  );
  const judgePacket = params.judgeMember
    ? [
      buildWorkPacket({
        member: params.judgeMember,
        phase: 'judge',
        launchBatch: finalReviewerBatch + 1,
        scope: fullScope,
        timeoutSeconds: params.executionPolicy.judgeTimeoutSeconds,
      }),
    ]
    : [];

  return [...reviewerPackets, ...judgePacket];
}
