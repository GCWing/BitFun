import type { ReviewTargetClassification } from '../reviewTargetClassifier';
import type {
  DeepReviewEvidencePack,
  DeepReviewEvidencePackContractHint,
  DeepReviewEvidencePackContractHintKind,
  DeepReviewScopeProfile,
  ReviewTeamChangeStats,
  ReviewTeamWorkPacket,
} from './types';
import { includedReviewTargetFiles } from './pathMetadata';

const EVIDENCE_PACK_CHANGED_FILE_LIMIT = 80;
const EVIDENCE_PACK_HUNK_HINT_LIMIT = 80;
const EVIDENCE_PACK_CONTRACT_HINT_LIMIT = 40;

function evidencePackContractHintKindForFile(
  filePath: string,
  target: ReviewTargetClassification,
): DeepReviewEvidencePackContractHintKind | undefined {
  const file = target.files.find((candidate) => candidate.normalizedPath === filePath);
  const tags = file?.tags ?? [];
  if (tags.includes('frontend_i18n')) {
    return 'i18n_key';
  }
  if (tags.includes('desktop_contract')) {
    return 'tauri_command';
  }
  if (
    tags.includes('api_layer') ||
    tags.includes('frontend_contract') ||
    tags.includes('web_server_contract')
  ) {
    return 'api_contract';
  }
  if (tags.includes('config')) {
    return 'config_key';
  }
  return undefined;
}

function buildEvidencePackContractHints(
  changedFiles: string[],
  target: ReviewTargetClassification,
): DeepReviewEvidencePackContractHint[] {
  return changedFiles
    .map((filePath) => {
      const kind = evidencePackContractHintKindForFile(filePath, target);
      return kind
        ? {
          kind,
          filePath,
          source: 'path_classifier' as const,
        }
        : undefined;
    })
    .filter((hint): hint is DeepReviewEvidencePackContractHint => Boolean(hint));
}

function buildEvidencePackHunkHints(
  changedFiles: string[],
  changeStats: ReviewTeamChangeStats,
): DeepReviewEvidencePack['hunkHints'] {
  const changedLineCount = changeStats.totalLinesChanged === undefined ||
    changedFiles.length === 0
    ? 0
    : Math.ceil(changeStats.totalLinesChanged / changedFiles.length);
  return changedFiles.map((filePath) => ({
    filePath,
    changedLineCount,
    lineCountSource: changeStats.lineCountSource,
  }));
}

export function buildDeepReviewEvidencePack(params: {
  target: ReviewTargetClassification;
  changeStats: ReviewTeamChangeStats;
  scopeProfile?: DeepReviewScopeProfile;
  workPackets: ReviewTeamWorkPacket[];
}): DeepReviewEvidencePack {
  const includedFiles = includedReviewTargetFiles(params.target);
  const allHunkHints = buildEvidencePackHunkHints(includedFiles, params.changeStats);
  const allContractHints = buildEvidencePackContractHints(includedFiles, params.target);
  const changedFiles = includedFiles.slice(0, EVIDENCE_PACK_CHANGED_FILE_LIMIT);
  const hunkHints = allHunkHints.slice(0, EVIDENCE_PACK_HUNK_HINT_LIMIT);
  const contractHints = allContractHints.slice(0, EVIDENCE_PACK_CONTRACT_HINT_LIMIT);

  return {
    version: 1,
    source: 'target_manifest',
    changedFiles,
    diffStat: {
      fileCount: params.changeStats.fileCount,
      ...(params.changeStats.totalLinesChanged !== undefined
        ? { totalChangedLines: params.changeStats.totalLinesChanged }
        : {}),
      lineCountSource: params.changeStats.lineCountSource,
    },
    domainTags: [...params.target.tags],
    riskFocusTags: [...(params.scopeProfile?.riskFocusTags ?? [])],
    packetIds: params.workPackets.map((packet) => packet.packetId),
    hunkHints,
    contractHints,
    budget: {
      maxChangedFiles: EVIDENCE_PACK_CHANGED_FILE_LIMIT,
      maxHunkHints: EVIDENCE_PACK_HUNK_HINT_LIMIT,
      maxContractHints: EVIDENCE_PACK_CONTRACT_HINT_LIMIT,
      omittedChangedFileCount: Math.max(0, includedFiles.length - changedFiles.length),
      omittedHunkHintCount: Math.max(0, allHunkHints.length - hunkHints.length),
      omittedContractHintCount: Math.max(0, allContractHints.length - contractHints.length),
    },
    privacy: {
      content: 'metadata_only',
      excludes: [
        'source_text',
        'full_diff',
        'model_output',
        'provider_raw_body',
        'full_file_contents',
      ],
    },
  };
}
