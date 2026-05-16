import type { ReviewTargetClassification } from '../reviewTargetClassifier';
import type {
  ReviewTeamChangeStats,
  ReviewTeamIncrementalReviewCacheInvalidation,
  ReviewTeamIncrementalReviewCachePlan,
  ReviewTeamSharedContextCachePlan,
  ReviewTeamSharedContextTool,
  ReviewTeamWorkPacket,
  ReviewStrategyLevel,
} from './types';
import {
  includedReviewTargetFiles,
  workspaceAreaForReviewPath,
} from './pathMetadata';

const SHARED_CONTEXT_CACHE_ENTRY_LIMIT = 80;
const SHARED_CONTEXT_CACHE_RECOMMENDED_TOOLS: ReviewTeamSharedContextTool[] = [
  'GetFileDiff',
  'Read',
];

export function buildSharedContextCachePlan(
  workPackets: ReviewTeamWorkPacket[] = [],
): ReviewTeamSharedContextCachePlan {
  const fileContextByPath = new Map<
    string,
    {
      path: string;
      workspaceArea: string;
      consumerPacketIds: string[];
      firstSeenIndex: number;
    }
  >();
  let nextSeenIndex = 0;

  for (const packet of workPackets) {
    if (packet.phase !== 'reviewer') {
      continue;
    }

    for (const path of packet.assignedScope.files) {
      let entry = fileContextByPath.get(path);
      if (!entry) {
        entry = {
          path,
          workspaceArea: workspaceAreaForReviewPath(path),
          consumerPacketIds: [],
          firstSeenIndex: nextSeenIndex,
        };
        nextSeenIndex += 1;
        fileContextByPath.set(path, entry);
      }
      if (!entry.consumerPacketIds.includes(packet.packetId)) {
        entry.consumerPacketIds.push(packet.packetId);
      }
    }
  }

  const repeatedFileContexts = Array.from(fileContextByPath.values())
    .filter((entry) => entry.consumerPacketIds.length > 1)
    .sort((a, b) => a.firstSeenIndex - b.firstSeenIndex);
  const entries = repeatedFileContexts
    .slice(0, SHARED_CONTEXT_CACHE_ENTRY_LIMIT)
    .map((entry, index) => ({
      cacheKey: `shared-context:${index + 1}`,
      path: entry.path,
      workspaceArea: entry.workspaceArea,
      recommendedTools: [...SHARED_CONTEXT_CACHE_RECOMMENDED_TOOLS],
      consumerPacketIds: entry.consumerPacketIds,
    }));

  return {
    source: 'work_packets',
    strategy: 'reuse_readonly_file_context_by_cache_key',
    entries,
    omittedEntryCount: Math.max(
      0,
      repeatedFileContexts.length - SHARED_CONTEXT_CACHE_ENTRY_LIMIT,
    ),
  };
}

const INCREMENTAL_REVIEW_CACHE_INVALIDATIONS: ReviewTeamIncrementalReviewCacheInvalidation[] = [
  'target_file_set_changed',
  'target_line_count_changed',
  'target_tag_changed',
  'target_warning_changed',
  'reviewer_roster_changed',
  'strategy_changed',
];

function stableFingerprint(input: unknown): string {
  const serialized = JSON.stringify(input);
  let hash = 0x811c9dc5;
  for (let index = 0; index < serialized.length; index += 1) {
    hash ^= serialized.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193);
  }
  return (hash >>> 0).toString(16).padStart(8, '0');
}

export function buildIncrementalReviewCachePlan(params: {
  target: ReviewTargetClassification;
  changeStats: ReviewTeamChangeStats;
  strategyLevel: ReviewStrategyLevel;
  workPackets: ReviewTeamWorkPacket[];
}): ReviewTeamIncrementalReviewCachePlan {
  const filePaths = includedReviewTargetFiles(params.target)
    .sort((a, b) => a.localeCompare(b));
  const workspaceAreas = Array.from(
    new Set(filePaths.map((file) => workspaceAreaForReviewPath(file))),
  ).sort((a, b) => a.localeCompare(b));
  const targetTags = [...params.target.tags].sort((a, b) => a.localeCompare(b));
  const targetWarnings = params.target.warnings
    .map((warning) => warning.code)
    .sort((a, b) => a.localeCompare(b));
  const reviewerPacketIds = params.workPackets
    .filter((packet) => packet.phase === 'reviewer')
    .map((packet) => packet.packetId)
    .sort((a, b) => a.localeCompare(b));
  const fingerprint = stableFingerprint({
    source: params.target.source,
    resolution: params.target.resolution,
    filePaths,
    workspaceAreas,
    targetTags,
    targetWarnings,
    lineCount: params.changeStats.totalLinesChanged ?? null,
    lineCountSource: params.changeStats.lineCountSource,
    reviewerPacketIds,
    strategyLevel: params.strategyLevel,
  });

  return {
    source: 'target_manifest',
    strategy: 'reuse_completed_packets_when_fingerprint_matches',
    cacheKey: `incremental-review:${fingerprint}`,
    fingerprint,
    filePaths,
    workspaceAreas,
    targetTags,
    reviewerPacketIds,
    ...(params.changeStats.totalLinesChanged !== undefined
      ? { lineCount: params.changeStats.totalLinesChanged }
      : {}),
    lineCountSource: params.changeStats.lineCountSource,
    invalidatesOn: [...INCREMENTAL_REVIEW_CACHE_INVALIDATIONS],
  };
}
