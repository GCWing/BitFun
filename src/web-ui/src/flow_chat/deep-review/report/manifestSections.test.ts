import { describe, expect, it } from 'vitest';
import type {
  ReviewTeamManifestMember,
  ReviewTeamRunManifest,
} from '@/shared/services/reviewTeamService';
import { DEFAULT_CODE_REVIEW_MARKDOWN_LABELS } from './codeReviewReport';
import { formatRunManifestMarkdownSection } from './manifestSections';

function manifestMember(
  subagentId: string,
  displayName: string,
  reason?: ReviewTeamManifestMember['reason'],
): ReviewTeamManifestMember {
  return {
    subagentId,
    displayName,
    roleName: displayName,
    model: 'fast',
    configuredModel: 'fast',
    defaultModelSlot: 'fast',
    strategyLevel: 'normal',
    strategySource: 'team',
    strategyDirective: 'Review the target.',
    locked: !subagentId.startsWith('Custom'),
    source: subagentId.startsWith('Custom') ? 'extra' : 'core',
    subagentSource: subagentId.startsWith('Custom') ? 'user' : 'builtin',
    ...(reason ? { reason } : {}),
  };
}

function buildRunManifest(): ReviewTeamRunManifest {
  return {
    reviewMode: 'deep',
    workspacePath: '/test-fixtures/project-a',
    policySource: 'default-review-team-config',
    target: {
      source: 'session_files',
      resolution: 'resolved',
      tags: ['frontend'],
      files: ['src/App.tsx'],
      warnings: [],
    },
    strategyLevel: 'normal',
    strategyRecommendation: {
      strategyLevel: 'deep',
      score: 24,
      rationale: 'Large/high-risk change.',
      factors: {
        fileCount: 8,
        totalLinesChanged: 900,
        lineCountSource: 'diff_stat',
        securityFileCount: 2,
        workspaceAreaCount: 3,
        contractSurfaceChanged: true,
      },
    },
    executionPolicy: {
      reviewerTimeoutSeconds: 300,
      judgeTimeoutSeconds: 240,
      reviewerFileSplitThreshold: 20,
      maxSameRoleInstances: 3,
      maxRetriesPerRole: 1,
    },
    concurrencyPolicy: {
      maxParallelInstances: 4,
      staggerSeconds: 0,
      maxQueueWaitSeconds: 60,
      batchExtrasSeparately: true,
      allowProviderCapacityQueue: true,
      allowBoundedAutoRetry: false,
      autoRetryElapsedGuardSeconds: 180,
    },
    preReviewSummary: {
      source: 'target_manifest',
      summary: '1 file, 12 changed lines across 1 workspace area: web-ui (1)',
      fileCount: 1,
      excludedFileCount: 0,
      lineCount: 12,
      lineCountSource: 'diff_stat',
      targetTags: ['frontend'],
      workspaceAreas: [
        {
          key: 'web-ui',
          fileCount: 1,
          sampleFiles: ['src/App.tsx'],
        },
      ],
      warnings: [],
    },
    sharedContextCache: {
      source: 'work_packets',
      strategy: 'reuse_readonly_file_context_by_cache_key',
      entries: [
        {
          cacheKey: 'shared-context:1',
          path: 'src/App.tsx',
          workspaceArea: 'web-ui',
          recommendedTools: ['GetFileDiff', 'Read'],
          consumerPacketIds: [
            'reviewer:ReviewBusinessLogic',
            'reviewer:CustomSecurity',
          ],
        },
      ],
      omittedEntryCount: 0,
    },
    incrementalReviewCache: {
      source: 'target_manifest',
      strategy: 'reuse_completed_packets_when_fingerprint_matches',
      cacheKey: 'incremental-review:abc12345',
      fingerprint: 'abc12345',
      filePaths: ['src/App.tsx'],
      workspaceAreas: ['web-ui'],
      targetTags: ['frontend'],
      reviewerPacketIds: [
        'reviewer:ReviewBusinessLogic',
        'reviewer:CustomSecurity',
      ],
      lineCount: 12,
      lineCountSource: 'diff_stat',
      invalidatesOn: [
        'target_file_set_changed',
        'target_line_count_changed',
        'reviewer_roster_changed',
      ],
    },
    tokenBudget: {
      mode: 'balanced',
      estimatedReviewerCalls: 3,
      maxReviewerCalls: 4,
      maxExtraReviewers: 1,
      largeDiffSummaryFirst: false,
      skippedReviewerIds: ['CustomInvalid'],
      warnings: [],
    },
    coreReviewers: [
      manifestMember('ReviewBusinessLogic', 'Logic reviewer'),
    ],
    qualityGateReviewer: manifestMember('ReviewJudge', 'Quality inspector'),
    enabledExtraReviewers: [
      manifestMember('CustomSecurity', 'Custom security reviewer'),
    ],
    skippedReviewers: [
      manifestMember('ReviewFrontend', 'Frontend reviewer', 'not_applicable'),
      manifestMember('CustomInvalid', 'Custom invalid reviewer', 'invalid_tooling'),
    ],
  };
}

describe('manifestSections', () => {
  it('formats Deep Review manifest markdown without content payload fields', () => {
    const markdown = formatRunManifestMarkdownSection(
      buildRunManifest(),
      DEFAULT_CODE_REVIEW_MARKDOWN_LABELS,
    );

    expect(markdown).toContain('## Run manifest');
    expect(markdown).toContain('- Target: frontend');
    expect(markdown).toContain('- Logic reviewer (ReviewBusinessLogic)');
    expect(markdown).toContain('- Quality inspector (ReviewJudge)');
    expect(markdown).toContain('- Custom invalid reviewer (CustomInvalid): invalid_tooling');
    expect(markdown).toContain('### Shared context cache');
    expect(markdown).toContain(
      '- shared-context:1: src/App.tsx -> reviewer:ReviewBusinessLogic, reviewer:CustomSecurity',
    );
    expect(markdown).not.toContain('source_text');
    expect(markdown).not.toContain('full_diff');
    expect(markdown).not.toContain('model_output');
    expect(markdown).not.toContain('provider_raw_body');
    expect(markdown).not.toContain('full_file_contents');
  });
});
