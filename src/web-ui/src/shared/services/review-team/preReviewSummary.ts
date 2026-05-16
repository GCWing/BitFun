import type { ReviewTargetClassification } from '../reviewTargetClassifier';
import type {
  ReviewTeamChangeStats,
  ReviewTeamPreReviewSummary,
} from './types';
import {
  groupFilesByWorkspaceArea,
  includedReviewTargetFiles,
  pluralize,
} from './pathMetadata';

const PRE_REVIEW_SUMMARY_SAMPLE_FILE_LIMIT = 3;
const PRE_REVIEW_SUMMARY_AREA_LIMIT = 8;

export function buildPreReviewSummary(
  target: ReviewTargetClassification,
  changeStats: ReviewTeamChangeStats,
): ReviewTeamPreReviewSummary {
  const includedFiles = includedReviewTargetFiles(target);
  const excludedFileCount = target.files.length - includedFiles.length;
  const allWorkspaceAreas = groupFilesByWorkspaceArea(includedFiles)
    .sort((a, b) => b.files.length - a.files.length || a.index - b.index);
  const workspaceAreas = allWorkspaceAreas
    .slice(0, PRE_REVIEW_SUMMARY_AREA_LIMIT)
    .map((area) => ({
      key: area.key,
      fileCount: area.files.length,
      sampleFiles: area.files.slice(0, PRE_REVIEW_SUMMARY_SAMPLE_FILE_LIMIT),
    }));
  const lineCount = changeStats.totalLinesChanged;
  const lineCountLabel =
    lineCount === undefined
      ? 'unknown changed lines'
      : `${lineCount} changed lines`;
  const areaLabel = workspaceAreas.length > 0
    ? workspaceAreas.map((area) => `${area.key} (${area.fileCount})`).join(', ')
    : 'no resolved workspace area';
  const targetTags = [...target.tags];
  const tagLabel = targetTags.filter((tag) => tag !== 'unknown').join(', ') || 'unknown';
  const omittedAreaCount = Math.max(
    0,
    allWorkspaceAreas.length - workspaceAreas.length,
  );
  const summaryParts = [
    `${pluralize(changeStats.fileCount, 'file')}, ${lineCountLabel} across ${pluralize(allWorkspaceAreas.length, 'workspace area')}: ${areaLabel}`,
    `tags: ${tagLabel}`,
    omittedAreaCount > 0 ? `${pluralize(omittedAreaCount, 'workspace area')} omitted from summary` : undefined,
  ].filter(Boolean);

  return {
    source: 'target_manifest',
    summary: summaryParts.join('; '),
    fileCount: changeStats.fileCount,
    excludedFileCount,
    ...(lineCount !== undefined ? { lineCount } : {}),
    lineCountSource: changeStats.lineCountSource,
    targetTags,
    workspaceAreas,
    warnings: target.warnings.map((warning) => warning.code),
  };
}
