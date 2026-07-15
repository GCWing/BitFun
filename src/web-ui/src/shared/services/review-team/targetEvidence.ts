import type {
  GitChangedFile,
  GitStatus,
} from '@/infrastructure/api/service-api/GitAPI';
import type { ReviewTargetClassification } from '../reviewTargetClassifier';
import type {
  ReviewTargetEvidence,
  ReviewTargetEvidenceCompleteness,
  ReviewTargetEvidenceFile,
  ReviewTargetEvidenceSource,
  ReviewTargetPullRequestIdentity,
  ReviewTargetWorkspaceBinding,
} from './types';

const REVIEW_TARGET_FILE_LIMIT = 500;
const REVIEW_TARGET_DIFF_TOTAL_CHARS = 80_000;

function maximumUnifiedDiffSectionLength(diff: string | undefined): number {
  if (!diff) return 0;
  const sections = diff.split(/(?=^diff --git )/m).filter(Boolean);
  return Math.max(0, ...sections.map((section) => section.length));
}

function isFullCommitId(value: string | undefined): value is string {
  return Boolean(value && /^[0-9a-f]{40}$/i.test(value));
}

export function stableReviewFingerprint(input: unknown): string {
  const serialized = JSON.stringify(input) ?? 'undefined';
  let hash = 0xcbf29ce484222325n;
  for (let index = 0; index < serialized.length; index += 1) {
    hash ^= BigInt(serialized.charCodeAt(index));
    hash = BigInt.asUintN(64, hash * 0x100000001b3n);
  }
  return hash.toString(16).padStart(16, '0');
}

function normalizedPath(path: string): string {
  return path.replace(/^\.\/+/, '');
}

function statusPathSet(status: GitStatus): Set<string> {
  return new Set([
    ...status.staged.map((file) => normalizedPath(file.path)),
    ...status.unstaged.map((file) => normalizedPath(file.path)),
    ...status.untracked.map(normalizedPath),
    ...status.conflicts.map(normalizedPath),
  ]);
}

export function resolveWorkspaceBinding(params: {
  targetHeadRevision?: string;
  workspaceHeadRevision?: string;
  status?: GitStatus;
}): ReviewTargetWorkspaceBinding {
  if (!params.targetHeadRevision || !params.workspaceHeadRevision || !params.status) {
    return 'unavailable';
  }
  if (params.targetHeadRevision !== params.workspaceHeadRevision) {
    return 'mismatched';
  }
  const dirtyPaths = statusPathSet(params.status);
  return dirtyPaths.size > 0 ? 'matching_dirty' : 'matching_clean';
}

function normalizeStatus(status: string | undefined): ReviewTargetEvidenceFile['status'] {
  const normalized = status?.trim();
  if (!normalized) {
    return 'unknown';
  }
  if (['added', 'modified', 'deleted', 'renamed', 'copied'].includes(normalized)) {
    return normalized as ReviewTargetEvidenceFile['status'];
  }
  if (normalized.includes('C')) {
    return 'unknown';
  }
  if (normalized.includes('R')) {
    return 'renamed';
  }
  if (normalized.includes('A') || normalized.includes('?')) {
    return 'added';
  }
  if (normalized.includes('D')) {
    return 'deleted';
  }
  if (normalized.includes('M') || normalized.includes('T')) {
    return 'modified';
  }
  return 'unknown';
}

function binaryPathsFromUnifiedDiff(diff: string | undefined): {
  paths: Set<string>;
  hasUnassignedBinarySection: boolean;
} {
  const paths = new Set<string>();
  let currentPaths: string[] = [];
  let hasUnassignedBinarySection = false;

  for (const line of diff?.split(/\r?\n/) ?? []) {
    if (line.startsWith('diff --git ')) {
      currentPaths = [];
      const header = line.match(/^diff --git a\/(.+) b\/(.+)$/);
      if (header) {
        currentPaths = [normalizedPath(header[1]), normalizedPath(header[2])];
      }
      continue;
    }
    if (line === 'GIT binary patch' || /^Binary files .+ differ$/.test(line)) {
      if (currentPaths.length === 0) {
        hasUnassignedBinarySection = true;
      }
      currentPaths.forEach((path) => paths.add(path));
    }
  }

  return { paths, hasUnassignedBinarySection };
}

function cappedFiles(
  files: ReviewTargetEvidenceFile[],
): { files: ReviewTargetEvidenceFile[]; omittedFileCount: number } {
  return {
    files: files.slice(0, REVIEW_TARGET_FILE_LIMIT),
    omittedFileCount: Math.max(0, files.length - REVIEW_TARGET_FILE_LIMIT),
  };
}

function finalCompleteness(
  requested: ReviewTargetEvidenceCompleteness,
  files: ReviewTargetEvidenceFile[],
  omittedFileCount: number,
): ReviewTargetEvidenceCompleteness {
  if (requested === 'stale' || requested === 'unknown') {
    return requested;
  }
  if (
    omittedFileCount > 0 ||
    files.some((file) => file.completeness !== 'complete')
  ) {
    return 'partial';
  }
  return requested;
}

function evidence(params: {
  source: ReviewTargetEvidence['source'];
  baseRevision?: string;
  headRevision?: string;
  workspaceBinding: ReviewTargetWorkspaceBinding;
  files: ReviewTargetEvidenceFile[];
  completeness: ReviewTargetEvidenceCompleteness;
  limitations?: string[];
  pullRequest?: ReviewTargetPullRequestIdentity;
  omittedFileCount?: number;
  fingerprintInput: unknown;
}): ReviewTargetEvidence {
  const capped = cappedFiles(params.files);
  const omittedFileCount = capped.omittedFileCount + (params.omittedFileCount ?? 0);
  const limitations = [...(params.limitations ?? [])];
  if (omittedFileCount > 0 && !limitations.includes('target_file_limit_exceeded')) {
    limitations.push('target_file_limit_exceeded');
  }
  const completeness = finalCompleteness(
    params.completeness,
    capped.files,
    omittedFileCount,
  );
  const fingerprint = stableReviewFingerprint({
    source: params.source,
    baseRevision: params.baseRevision ?? null,
    headRevision: params.headRevision ?? null,
    workspaceBinding: params.workspaceBinding,
    pullRequest: params.pullRequest ?? null,
    files: capped.files,
    omittedFileCount,
    limitations,
    evidence: params.fingerprintInput,
  });

  return {
    version: 1,
    source: params.source,
    fingerprint,
    ...(params.baseRevision ? { baseRevision: params.baseRevision } : {}),
    ...(params.headRevision ? { headRevision: params.headRevision } : {}),
    completeness,
    workspaceBinding: params.workspaceBinding,
    ...(params.pullRequest ? { pullRequest: params.pullRequest } : {}),
    files: capped.files,
    limitations,
    ...(omittedFileCount > 0
      ? { omittedFileCount }
      : {}),
  };
}

export function buildUnknownReviewTargetEvidence(
  target: ReviewTargetClassification,
  limitation: string,
): ReviewTargetEvidence {
  const source: ReviewTargetEvidenceSource = target.source === 'slash_command_git_ref'
    ? 'git_range'
    : 'workspace';
  return evidence({
    source,
    workspaceBinding: 'unavailable',
    completeness: 'unknown',
    files: target.files.filter((file) => !file.excluded).map((file) => ({
      path: file.normalizedPath,
      ...(file.normalizedOldPath ? { previousPath: file.normalizedOldPath } : {}),
      status: file.status,
      completeness: 'partial',
    })),
    limitations: [limitation],
    fingerprintInput: target,
  });
}

export function buildWorkspaceReviewTargetEvidence(params: {
  target: ReviewTargetClassification;
  baseRevision?: string;
  diff?: string;
  status: GitStatus;
  untrackedContentFingerprints?: Record<string, string>;
}): ReviewTargetEvidence {
  const conflictPaths = new Set(params.status.conflicts.map(normalizedPath));
  const includedTargetFiles = params.target.files.filter((file) => !file.excluded);
  const targetPaths = new Set(includedTargetFiles.map((file) => file.normalizedPath));
  const targetUntrackedPaths = params.status.untracked
    .map(normalizedPath)
    .filter((path) => targetPaths.has(path));
  const untrackedDirectoryPrefixes = params.status.untracked
    .map(normalizedPath)
    .filter((path) => path.endsWith('/'));
  const untrackedDirectoryTargets = [...targetPaths].filter((path) =>
    untrackedDirectoryPrefixes.some((prefix) => path.startsWith(prefix))
  );
  const unavailableUntrackedContent = targetUntrackedPaths.some((path) =>
    !params.untrackedContentFingerprints?.[path] ||
    params.untrackedContentFingerprints[path] === 'unavailable'
  ) || untrackedDirectoryTargets.length > 0;
  const binaryDiff = binaryPathsFromUnifiedDiff(params.diff);
  const maximumDiffSection = maximumUnifiedDiffSectionLength(params.diff);
  const diffExceedsBudget = maximumDiffSection > REVIEW_TARGET_DIFF_TOTAL_CHARS;
  const changedStatusByPath = new Map<string, ReviewTargetEvidenceFile['status']>();
  for (const file of params.status.staged) {
    changedStatusByPath.set(normalizedPath(file.path), normalizeStatus(file.status));
  }
  for (const file of params.status.unstaged) {
    changedStatusByPath.set(normalizedPath(file.path), normalizeStatus(file.status));
  }
  for (const path of params.status.untracked) {
    changedStatusByPath.set(normalizedPath(path), 'added');
  }
  for (const path of params.status.conflicts) {
    changedStatusByPath.set(normalizedPath(path), 'unknown');
  }

  const diffFingerprint = stableReviewFingerprint({
    diff: params.diff ?? '',
    untracked: params.untrackedContentFingerprints ?? {},
  });
  const limitations = [
    'mutable_workspace_evidence',
    ...(params.status.conflicts.length > 0 ? ['conflicted_files_present'] : []),
    ...(params.diff === undefined ? ['workspace_diff_unavailable'] : []),
    ...(unavailableUntrackedContent ? ['untracked_content_unavailable'] : []),
    ...(untrackedDirectoryTargets.length > 0
      ? ['untracked_directory_content_unavailable']
      : []),
    ...(diffExceedsBudget ? ['target_diff_budget_exceeded'] : []),
    ...(binaryDiff.paths.size > 0 || binaryDiff.hasUnassignedBinarySection
      ? ['binary_diff_unavailable']
      : []),
  ];
  const completeness: ReviewTargetEvidenceCompleteness =
    params.diff !== undefined &&
    params.status.conflicts.length === 0 &&
    !unavailableUntrackedContent &&
    !diffExceedsBudget &&
    binaryDiff.paths.size === 0 &&
    !binaryDiff.hasUnassignedBinarySection
      ? 'complete'
      : 'partial';

  return evidence({
    source: 'workspace',
    baseRevision: params.baseRevision,
    headRevision: `worktree:${diffFingerprint}`,
    workspaceBinding: 'matching_dirty',
    completeness,
    files: includedTargetFiles.map((file) => ({
      path: file.normalizedPath,
      ...(file.normalizedOldPath ? { previousPath: file.normalizedOldPath } : {}),
      status: untrackedDirectoryTargets.includes(normalizedPath(file.normalizedPath))
        ? 'unknown'
        : file.status === 'renamed' || file.status === 'copied'
        ? file.status
        : changedStatusByPath.get(file.normalizedPath) ?? file.status,
      completeness: binaryDiff.paths.has(normalizedPath(file.normalizedPath))
        ? 'unavailable'
        : untrackedDirectoryTargets.includes(normalizedPath(file.normalizedPath))
          ? 'partial'
        : targetUntrackedPaths.includes(normalizedPath(file.normalizedPath)) &&
            (!params.untrackedContentFingerprints?.[normalizedPath(file.normalizedPath)] ||
              params.untrackedContentFingerprints[normalizedPath(file.normalizedPath)] === 'unavailable')
          ? 'partial'
        : conflictPaths.has(normalizedPath(file.normalizedPath))
          ? 'partial'
          : 'complete',
    })),
    limitations,
    fingerprintInput: {
      diffFingerprint,
      target: params.target,
    },
  });
}

export function buildGitRangeReviewTargetEvidence(params: {
  target: ReviewTargetClassification;
  changedFiles: GitChangedFile[];
  baseRevision?: string;
  headRevision?: string;
  workspaceHeadRevision?: string;
  status?: GitStatus;
  diff?: string;
}): ReviewTargetEvidence {
  const includedTargetPaths = new Set(
    params.target.files
      .filter((file) => !file.excluded)
      .flatMap((file) => [
        normalizedPath(file.normalizedPath),
        ...(file.normalizedOldPath ? [normalizedPath(file.normalizedOldPath)] : []),
      ]),
  );
  const workspaceBinding = resolveWorkspaceBinding({
    targetHeadRevision: params.headRevision,
    workspaceHeadRevision: params.workspaceHeadRevision,
    status: params.status,
  });
  const revisionsComplete = isFullCommitId(params.baseRevision) && isFullCommitId(params.headRevision);
  const binaryDiff = binaryPathsFromUnifiedDiff(params.diff);
  const maximumDiffSection = maximumUnifiedDiffSectionLength(params.diff);
  const diffExceedsBudget = maximumDiffSection > REVIEW_TARGET_DIFF_TOTAL_CHARS;
  const limitations = [
    ...(!revisionsComplete ? ['git_revision_unresolved'] : []),
    ...(params.diff === undefined ? ['git_diff_unavailable'] : []),
    ...(workspaceBinding === 'matching_dirty' ? ['workspace_has_local_changes'] : []),
    ...(workspaceBinding === 'mismatched' ? ['workspace_head_mismatch'] : []),
    ...(diffExceedsBudget ? ['target_diff_budget_exceeded'] : []),
    ...(binaryDiff.paths.size > 0 || binaryDiff.hasUnassignedBinarySection
      ? ['binary_diff_unavailable']
      : []),
  ];
  const diffFingerprint = stableReviewFingerprint(params.diff ?? '');

  return evidence({
    source: 'git_range',
    baseRevision: params.baseRevision,
    headRevision: params.headRevision,
    workspaceBinding,
    completeness:
      revisionsComplete &&
      params.diff !== undefined &&
      workspaceBinding === 'matching_clean' &&
      !diffExceedsBudget &&
      binaryDiff.paths.size === 0 &&
      !binaryDiff.hasUnassignedBinarySection
        ? 'complete'
        : 'partial',
    files: params.changedFiles.filter((file) => (
      includedTargetPaths.has(normalizedPath(file.path)) ||
      Boolean(file.old_path && includedTargetPaths.has(normalizedPath(file.old_path)))
    )).map((file) => ({
      path: normalizedPath(file.path),
      ...(file.old_path ? { previousPath: normalizedPath(file.old_path) } : {}),
      status: normalizeStatus(file.status),
      completeness: binaryDiff.paths.has(normalizedPath(file.path)) ||
        Boolean(file.old_path && binaryDiff.paths.has(normalizedPath(file.old_path)))
        ? 'unavailable'
        : 'complete',
    })),
    limitations,
    fingerprintInput: {
      diffFingerprint,
      target: params.target,
    },
  });
}

export function buildPullRequestReviewTargetEvidence(params: {
  target: ReviewTargetClassification;
  baseRevision?: string;
  headRevision?: string;
  pullRequest: ReviewTargetPullRequestIdentity;
  files: Array<{
    path: string;
    oldPath?: string | null;
    status: string;
    diffAvailable: boolean;
  }>;
  omittedFileCount?: number;
  limitations?: string[];
}): ReviewTargetEvidence {
  const revisionsComplete = isFullCommitId(params.baseRevision) && isFullCommitId(params.headRevision);
  const limitations = [
    ...(params.limitations ?? []),
    ...(!revisionsComplete ? ['provider_revision_unresolved'] : []),
  ];
  const files = params.files.map((file) => ({
    path: normalizedPath(file.path),
    ...(file.oldPath ? { previousPath: normalizedPath(file.oldPath) } : {}),
    status: normalizeStatus(file.status),
    completeness: file.diffAvailable ? 'complete' as const : 'unavailable' as const,
  }));
  const complete = revisionsComplete
    && (params.omittedFileCount ?? 0) === 0
    && files.every((file) => file.completeness === 'complete');

  return evidence({
    source: 'pull_request',
    baseRevision: params.baseRevision,
    headRevision: params.headRevision,
    workspaceBinding: 'unavailable',
    pullRequest: params.pullRequest,
    files,
    completeness: complete ? 'complete' : 'partial',
    limitations,
    omittedFileCount: params.omittedFileCount,
    fingerprintInput: {
      pullRequest: params.pullRequest,
      target: params.target,
      files,
    },
  });
}

export function allowsReviewLiveRepositoryContext(
  evidence: ReviewTargetEvidence | undefined,
): boolean {
  return Boolean(
    evidence &&
    evidence.source === 'git_range' &&
    evidence.workspaceBinding === 'matching_clean' &&
    isFullCommitId(evidence.baseRevision) &&
    isFullCommitId(evidence.headRevision),
  );
}
