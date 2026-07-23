import { describe, expect, it } from 'vitest';
import {
  buildDeepReviewRetryPrompt,
  buildCodeReviewReportSections,
  buildCodeReviewReliabilityNotices,
  extractDeepReviewRetryableSlices,
  formatCodeReviewReportMarkdown,
  getDefaultExpandedCodeReviewSectionIds,
} from './codeReviewReport';
import type {
  ReviewTeamManifestMember,
  ReviewTeamRunManifest,
  ReviewTeamWorkPacket,
} from '@/shared/services/reviewTeamService';

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
      rationale: 'Large/high-risk change (8 files, 900 lines; 2 security-sensitive files, 3 workspace areas). Strict review recommended.',
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
      reviewerTimeoutSeconds: 1800,
      judgeTimeoutSeconds: 1200,
      reviewerFileSplitThreshold: 20,
      maxSameRoleInstances: 3,
      maxRetriesPerRole: 1,
    },
    concurrencyPolicy: {
      maxParallelInstances: 4,
      staggerSeconds: 0,
      batchExtrasSeparately: true,
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

function buildRetryWorkPacket(overrides: Partial<ReviewTeamWorkPacket> = {}): ReviewTeamWorkPacket {
  return {
    packetId: 'reviewer:ReviewSecurity:group-1-of-2',
    phase: 'reviewer',
    launchBatch: 0,
    subagentId: 'ReviewSecurity',
    displayName: 'Security reviewer',
    roleName: 'Security reviewer',
    assignedScope: {
      kind: 'review_target',
      targetSource: 'session_files',
      targetResolution: 'resolved',
      targetTags: ['security'],
      fileCount: 3,
      files: ['src/auth.ts', 'src/session.ts', 'src/audit.ts'],
      excludedFileCount: 0,
      groupIndex: 1,
      groupCount: 2,
    },
    allowedTools: ['Read', 'GetFileDiff'],
    timeoutSeconds: 300,
    requiredOutputFields: ['summary', 'findings'],
    strategyLevel: 'deep',
    strategyDirective: 'Review security-sensitive changes.',
    model: 'fast',
    ...overrides,
  };
}

function buildRetryRunManifest(
  workPackets: ReviewTeamWorkPacket[] = [buildRetryWorkPacket()],
): ReviewTeamRunManifest {
  return {
    ...buildRunManifest(),
    workPackets,
  };
}

function buildReducedScopeRunManifest(): ReviewTeamRunManifest {
  return {
    ...buildRunManifest(),
    scopeProfile: {
      reviewDepth: 'high_risk_only',
      riskFocusTags: ['security', 'cross_boundary_api_contracts'],
      maxDependencyHops: 0,
      optionalReviewerPolicy: 'risk_matched_only',
      allowBroadToolExploration: false,
      coverageExpectation: 'High-risk-only pass; changed files remain visible.',
    },
  };
}

describe('codeReviewReport', () => {
  it('uses structured report sections when present', () => {
    const report = {
      summary: {
        overall_assessment: 'One blocking security issue remains.',
        risk_level: 'high' as const,
        recommended_action: 'request_changes' as const,
        confidence_note: 'Security reviewer timed out, confidence reduced.',
      },
      issues: [
        {
          severity: 'high' as const,
          certainty: 'confirmed' as const,
          category: 'security',
          file: 'src/auth.ts',
          line: 42,
          title: 'Token is logged',
          description: 'The access token is written to logs.',
          suggestion: 'Remove the token from log payloads.',
          source_reviewer: 'Security Reviewer',
          validation_note: 'Quality gate confirmed the token is sensitive.',
        },
      ],
      positive_points: ['Adapter boundary is clear.'],
      review_mode: 'deep' as const,
      review_scope: 'current workspace diff',
      reviewers: [
        {
          name: 'Security Reviewer',
          specialty: 'security',
          status: 'timed_out',
          summary: 'Partial security pass completed.',
          issue_count: 1,
        },
        {
          name: 'Review Quality Inspector',
          specialty: 'quality gate',
          status: 'completed',
          summary: 'Confirmed one finding.',
          issue_count: 1,
        },
      ],
      remediation_plan: ['Remove token logging.', 'Run auth regression tests.'],
      report_sections: {
        executive_summary: ['Fix token logging before merging.'],
        remediation_groups: {
          must_fix: ['Remove token logging.'],
          verification: ['Run auth regression tests.'],
        },
        strength_groups: {
          architecture: ['Adapter boundary is clear.'],
        },
        coverage_notes: ['Security review completed with reduced confidence.'],
      },
    };

    const sections = buildCodeReviewReportSections(report);

    expect(sections.executiveSummary).toEqual(['Fix token logging before merging.']);
    expect(sections.remediationGroups).toEqual([
      { id: 'must_fix', items: ['Remove token logging.'] },
      { id: 'verification', items: ['Run auth regression tests.'] },
    ]);
    expect(sections.strengthGroups).toEqual([
      { id: 'architecture', items: ['Adapter boundary is clear.'] },
    ]);
    expect(sections.coverageNotes).toEqual(['Security review completed with reduced confidence.']);
    expect(sections.issueStats).toMatchObject({ total: 1, high: 1 });
    expect(sections.reviewerStats).toMatchObject({ total: 2, completed: 1, degraded: 1 });
  });

  it('falls back to legacy remediation and positive point fields', () => {
    const report = {
      summary: {
        overall_assessment: 'Looks safe with one suggestion.',
        risk_level: 'low' as const,
        recommended_action: 'approve_with_suggestions' as const,
      },
      issues: [],
      positive_points: ['Tests cover the changed service.'],
      remediation_plan: ['Add a narrow regression assertion.'],
    };

    const sections = buildCodeReviewReportSections(report);

    expect(sections.executiveSummary).toEqual(['Looks safe with one suggestion.']);
    expect(sections.remediationGroups).toEqual([
      { id: 'should_improve', items: ['Add a narrow regression assertion.'] },
    ]);
    expect(sections.strengthGroups).toEqual([
      { id: 'other', items: ['Tests cover the changed service.'] },
    ]);
  });

  it('surfaces partial review output in coverage notes', () => {
    const sections = buildCodeReviewReportSections({
      summary: {
        overall_assessment: 'Review completed with reduced confidence.',
        risk_level: 'medium' as const,
        recommended_action: 'request_changes' as const,
      },
      reviewers: [
        {
          name: 'Security Reviewer',
          specialty: 'security',
          status: 'partial_timeout',
          summary: 'Timed out after finding one likely issue.',
          partial_output: 'Found likely token logging in src/auth.ts before timeout.',
        },
      ],
    });

    expect(sections.reviewerStats).toMatchObject({ total: 1, completed: 0, degraded: 1 });
    expect(sections.coverageNotes).toEqual([
      'A review coverage check stopped before completion after producing partial output: Found likely token logging in src/auth.ts before timeout.',
    ]);
  });

  it('builds compact reliability notices only when review attention is needed', () => {
    expect(buildCodeReviewReliabilityNotices({
      summary: {
        overall_assessment: 'No issues found.',
        risk_level: 'low' as const,
        recommended_action: 'approve' as const,
      },
      reviewers: [{ name: 'Reviewer', specialty: 'logic', status: 'completed', summary: 'Done.' }],
    })).toEqual([]);

    const manifest = {
      ...buildRunManifest(),
      tokenBudget: {
        ...buildRunManifest().tokenBudget,
        largeDiffSummaryFirst: true,
        warnings: ['Large target; reviewers will receive compact scopes.'],
      },
    };
    const notices = buildCodeReviewReliabilityNotices({
      summary: {
        overall_assessment: 'Review completed with reduced confidence.',
        risk_level: 'medium' as const,
        recommended_action: 'request_changes' as const,
      },
      reviewers: [
        {
          name: 'Security Reviewer',
          specialty: 'security',
          status: 'partial_timeout',
          summary: 'Timed out after producing partial evidence.',
          partial_output: 'Found likely token logging in src/auth.ts before timeout.',
        },
      ],
      report_sections: {
        coverage_notes: ['Context compression preserved key file and test facts.'],
        remediation_groups: {
          needs_decision: ['Decide whether to block the release or isolate the feature.'],
        },
      },
    }, manifest);

    expect(notices.map((notice) => notice.kind)).toEqual([
      'context_pressure',
      'skipped_reviewers',
      'token_budget_limited',
      'compression_preserved',
      'partial_reviewer',
      'retry_guidance',
      'user_decision',
    ]);
    expect(notices.find((notice) => notice.kind === 'partial_reviewer')).toMatchObject({
      severity: 'warning',
      count: 1,
    });
  });

  it('prefers structured reliability signals for status and markdown export', () => {
    const report = {
      summary: {
        overall_assessment: 'Review completed with runtime reliability signals.',
        risk_level: 'medium' as const,
        recommended_action: 'request_changes' as const,
      },
      review_mode: 'deep' as const,
      reviewers: [
        {
          name: 'Security Reviewer',
          specialty: 'security',
          status: 'completed',
          summary: 'Completed.',
        },
      ],
      reliability_signals: [
        {
          kind: 'context_pressure',
          severity: 'warning',
          count: 7,
          source: 'runtime',
          detail: 'Runtime profile reduced review coverage for this large target.',
        },
        {
          kind: 'compression_preserved',
          severity: 'info',
          source: 'runtime',
          detail: 'Compression contract retained modified files and failed commands.',
        },
        {
          kind: 'cache_hit',
          severity: 'info',
          count: 2,
          source: 'runtime',
          detail: 'Two review results reused matching cached output.',
        },
        {
          kind: 'cache_miss',
          severity: 'info',
          count: 1,
          source: 'runtime',
          detail: 'One review result ran fresh and updated the cache.',
        },
        {
          kind: 'concurrency_limited',
          severity: 'warning',
          count: 1,
          source: 'runtime',
          detail: 'One review launch hit the configured concurrency cap.',
        },
        {
          kind: 'retry_guidance',
          severity: 'warning',
          count: 1,
          source: 'runtime',
          detail: 'Retry guidance was emitted for partial review coverage.',
        },
      ],
    };

    const notices = buildCodeReviewReliabilityNotices(report);

    expect(notices).toEqual([
      {
        kind: 'context_pressure',
        severity: 'warning',
        count: 7,
        source: 'runtime',
        detail: 'Runtime profile reduced review coverage for this large target.',
      },
      {
        kind: 'compression_preserved',
        severity: 'info',
        source: 'runtime',
        detail: 'Compression contract retained modified files and failed commands.',
      },
      {
        kind: 'cache_hit',
        severity: 'info',
        count: 2,
        source: 'runtime',
        detail: 'Two review results reused matching cached output.',
      },
      {
        kind: 'cache_miss',
        severity: 'info',
        count: 1,
        source: 'runtime',
        detail: 'One review result ran fresh and updated the cache.',
      },
      {
        kind: 'concurrency_limited',
        severity: 'warning',
        count: 1,
        source: 'runtime',
        detail: 'One review launch hit the configured concurrency cap.',
      },
      {
        kind: 'retry_guidance',
        severity: 'warning',
        count: 1,
        source: 'runtime',
        detail: 'Retry guidance was emitted for partial review coverage.',
      },
    ]);

    const markdown = formatCodeReviewReportMarkdown(report);

    expect(markdown).toContain('## Review Reliability');
    expect(markdown).toContain(
      '- Context pressure rising [warning/runtime]: Runtime profile reduced review coverage for this large target.',
    );
    expect(markdown).toContain(
      '- Compression preserved key facts [info/runtime]: Compression contract retained modified files and failed commands.',
    );
    expect(markdown).toContain(
      '- Incremental cache reused review output [info/runtime]: Two review results reused matching cached output.',
    );
    expect(markdown).toContain(
      '- Incremental cache missed or refreshed [info/runtime]: One review result ran fresh and updated the cache.',
    );
    expect(markdown).toContain(
      '- Review launch was concurrency-limited [warning/runtime]: One review launch hit the configured concurrency cap.',
    );
    expect(markdown).toContain(
      '- Retry guidance emitted [warning/runtime]: Retry guidance was emitted for partial review coverage.',
    );
  });

  it('summarizes tailored review scope and token budget tradeoffs from the run manifest', () => {
    const report = {
      summary: {
        overall_assessment: 'Review completed with tailored scope.',
        risk_level: 'medium' as const,
        recommended_action: 'request_changes' as const,
      },
      review_mode: 'deep' as const,
      reviewers: [
        {
          name: 'Business Logic Reviewer',
          specialty: 'logic',
          status: 'completed',
          summary: 'Done.',
        },
      ],
    };
    const notices = buildCodeReviewReliabilityNotices(report, buildRunManifest());

    expect(notices).toEqual([
      {
        kind: 'skipped_reviewers',
        severity: 'info',
        count: 2,
        source: 'manifest',
      },
      {
        kind: 'token_budget_limited',
        severity: 'warning',
        count: 1,
        source: 'manifest',
      },
    ]);

    const markdown = formatCodeReviewReportMarkdown(report, undefined, { runManifest: buildRunManifest() });

    expect(markdown).toContain('- Review scope tailored [info/manifest]: Count: 2');
    expect(markdown).toContain('- Token budget limited review coverage [warning/manifest]: Count: 1');
  });

  it('surfaces focused scope profile in reliability notices and markdown export', () => {
    const report = {
      summary: {
        overall_assessment: 'No blocking issues found in the high-risk pass.',
        risk_level: 'low' as const,
        recommended_action: 'approve' as const,
      },
      review_mode: 'deep' as const,
      reviewers: [],
    };
    const runManifest = buildReducedScopeRunManifest();

    const notices = buildCodeReviewReliabilityNotices(report, runManifest);

    expect(notices).toContainEqual({
      kind: 'reduced_scope',
      severity: 'info',
      source: 'manifest',
      detail: 'High-risk-only pass; changed files remain visible.',
    });

    const markdown = formatCodeReviewReportMarkdown(report, undefined, { runManifest });

    expect(markdown).not.toContain('- Review depth:');
    expect(markdown).not.toContain('- Coverage expectation:');
    expect(markdown).toContain(
      '- Focused review scope [info/manifest]: High-risk-only pass; changed files remain visible.',
    );
  });

  it('keeps evidence pack metadata out of exported markdown', () => {
    const report = {
      summary: {
        overall_assessment: 'Review completed.',
        risk_level: 'low' as const,
        recommended_action: 'approve' as const,
      },
      review_mode: 'deep' as const,
      reviewers: [],
    };
    const runManifest: ReviewTeamRunManifest = {
      ...buildRunManifest(),
      evidencePack: {
        version: 1,
        source: 'target_manifest',
        changedFiles: ['src/App.tsx'],
        diffStat: {
          fileCount: 1,
          totalChangedLines: 12,
          lineCountSource: 'diff_stat',
        },
        domainTags: ['frontend'],
        riskFocusTags: ['cross_boundary_api_contracts'],
        packetIds: ['reviewer:ReviewBusinessLogic', 'judge:ReviewJudge'],
        hunkHints: [
          {
            filePath: 'src/App.tsx',
            changedLineCount: 12,
            lineCountSource: 'diff_stat',
          },
        ],
        contractHints: [
          {
            kind: 'api_contract',
            filePath: 'src/App.tsx',
            source: 'path_classifier',
          },
        ],
        budget: {
          maxChangedFiles: 80,
          maxHunkHints: 80,
          maxContractHints: 40,
          omittedChangedFileCount: 0,
          omittedHunkHintCount: 0,
          omittedContractHintCount: 0,
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
      },
    };

    const markdown = formatCodeReviewReportMarkdown(report, undefined, { runManifest });

    expect(markdown).not.toContain('### Evidence scope');
    expect(markdown).not.toContain('- Changed files: 1; hunk hints: 1; contract hints: 1');
    expect(markdown).not.toContain('- Hints are orientation only and require tool confirmation before findings.');
    expect(markdown).not.toContain('packet ids');
    expect(markdown).not.toContain('target_manifest');
    expect(markdown).not.toContain('metadata_only');
    expect(markdown).not.toContain('source_text');
    expect(markdown).not.toContain('full_diff');
    expect(markdown).not.toContain('model_output');
    expect(markdown).not.toContain('provider_raw_body');
    expect(markdown).not.toContain('full_file_contents');
  });

  it('keeps team and issue details collapsed by default while leaving remediation visible', () => {
    const report = {
      summary: {
        overall_assessment: 'Needs changes.',
        risk_level: 'medium' as const,
        recommended_action: 'request_changes' as const,
      },
      issues: [{ severity: 'medium' as const, title: 'Bug', description: 'Bug' }],
      positive_points: ['Simple fix path.'],
      remediation_plan: ['Fix the bug.'],
      reviewers: [{ name: 'Reviewer', specialty: 'logic', status: 'completed', summary: 'Done.' }],
    };

    expect(getDefaultExpandedCodeReviewSectionIds(report)).toEqual(['summary', 'remediation']);
  });

  it('formats a review report as markdown for document export', () => {
    const markdown = formatCodeReviewReportMarkdown({
      summary: {
        overall_assessment: 'One fix required.',
        risk_level: 'medium' as const,
        recommended_action: 'request_changes' as const,
      },
      review_mode: 'deep' as const,
      review_scope: 'src/auth.ts',
      issues: [
        {
          severity: 'medium' as const,
          certainty: 'confirmed' as const,
          category: 'logic',
          file: 'src/auth.ts',
          line: 12,
          title: 'Missing guard',
          description: 'The null guard is missing.',
          suggestion: 'Add the guard.',
        },
      ],
      positive_points: ['Small surface area.'],
      remediation_plan: ['Add the guard.'],
      reviewers: [{ name: 'Business Logic Reviewer', specialty: 'logic', status: 'completed', summary: 'Found one issue.' }],
    });

    expect(markdown).toContain('# Strict Review Report');
    expect(markdown).toContain('## Executive Summary');
    expect(markdown).toContain('- One fix required.');
    expect(markdown).toContain('## Issues');
    expect(markdown).toContain('src/auth.ts:12');
    expect(markdown).toContain('## Remediation Plan');
    expect(markdown).not.toContain('## Review Execution Plan');
  });

  it('exports partial review output in markdown', () => {
    const markdown = formatCodeReviewReportMarkdown({
      summary: {
        overall_assessment: 'Review completed with partial security evidence.',
        risk_level: 'medium' as const,
        recommended_action: 'request_changes' as const,
      },
      review_mode: 'deep' as const,
      issues: [],
      reviewers: [
        {
          name: 'Security Reviewer',
          specialty: 'security',
          status: 'partial_timeout',
          summary: 'Timed out after producing partial evidence.',
          partial_output: 'Found likely token logging in src/auth.ts before timeout.',
        },
      ],
    });

    expect(markdown).not.toContain('Security Reviewer (security; Status: partial_timeout)');
    expect(markdown).not.toContain('Partial output: Found likely token logging in src/auth.ts before timeout.');
    expect(markdown).toContain(
      'A review coverage check stopped before completion after producing partial output: Found likely token logging in src/auth.ts before timeout.',
    );
  });

  it('keeps internal packet fallback metadata out of exported markdown', () => {
    const markdown = formatCodeReviewReportMarkdown({
      summary: {
        overall_assessment: 'Review completed with inferred packet metadata.',
        risk_level: 'low' as const,
        recommended_action: 'approve' as const,
      },
      review_mode: 'deep' as const,
      issues: [],
      reviewers: [
        {
          name: 'Security Reviewer',
          specialty: 'security',
          status: 'completed',
          summary: 'Checked the first security split.',
          packet_id: 'reviewer:ReviewSecurity:group-1-of-3',
          packet_status_source: 'inferred',
        },
      ],
    });

    expect(markdown).not.toContain('Security Reviewer (security; Status: completed)');
    expect(markdown).not.toContain('reviewer:ReviewSecurity:group-1-of-3');
  });

  it('extracts explicit retry coverage for partial timeout reviewers', () => {
    const slices = extractDeepReviewRetryableSlices({
      review_mode: 'deep',
      reviewers: [
        {
          name: 'Security reviewer',
          specialty: 'security',
          status: 'partial_timeout',
          summary: 'Timed out after two files.',
          packet_id: 'reviewer:ReviewSecurity:group-1-of-2',
          covered_files: ['src/session.ts'],
          retry_scope_files: ['src/auth.ts'],
        },
      ],
    }, buildRetryRunManifest());

    expect(slices).toEqual([
      {
        sourcePacketId: 'reviewer:ReviewSecurity:group-1-of-2',
        reviewerId: 'ReviewSecurity',
        reviewerName: 'Security reviewer',
        sourceStatus: 'partial_timeout',
        coveredFiles: ['src/session.ts'],
        retryScopeFiles: ['src/auth.ts'],
        retryTimeoutSeconds: 150,
      },
    ]);

    const prompt = buildDeepReviewRetryPrompt(slices);
    expect(prompt).toContain('<deep_review_retry_tasks>');
    expect(prompt).toContain('"retry_coverage"');
    expect(prompt).toContain('"source_packet_id": "reviewer:ReviewSecurity:group-1-of-2"');
    expect(prompt).toContain('"retry_scope_files"');
  });

  it('does not retry capacity skips with non-transient reasons', () => {
    const slices = extractDeepReviewRetryableSlices({
      review_mode: 'deep',
      reviewers: [
        {
          name: 'Security reviewer',
          specialty: 'security',
          status: 'capacity_skipped',
          summary: 'Skipped after an auth failure.',
          packet_id: 'reviewer:ReviewSecurity:group-1-of-2',
          capacity_reason: 'auth_error',
          retry_scope_files: ['src/auth.ts'],
        },
      ],
    }, buildRetryRunManifest());

    expect(slices).toEqual([]);
  });

  it('does not infer retry scope without explicit unresolved files', () => {
    const slices = extractDeepReviewRetryableSlices({
      review_mode: 'deep',
      reviewers: [
        {
          name: 'Security reviewer',
          specialty: 'security',
          status: 'partial_timeout',
          summary: 'Timed out with no machine-readable remaining scope.',
          packet_id: 'reviewer:ReviewSecurity:group-1-of-2',
          covered_files: ['src/session.ts'],
        },
      ],
    }, buildRetryRunManifest());

    expect(slices).toEqual([]);
  });

  it('keeps run manifest details out of strict review markdown export', () => {
    const markdown = formatCodeReviewReportMarkdown(
      {
        summary: {
          overall_assessment: 'No validated issues.',
          risk_level: 'low' as const,
          recommended_action: 'approve' as const,
        },
        review_mode: 'deep' as const,
        issues: [],
        reviewers: [],
      },
      undefined,
      { runManifest: buildRunManifest() },
    );

    expect(markdown).not.toContain('## Review Coverage And Cost');
    expect(markdown).not.toContain('- Target: frontend');
    expect(markdown).not.toContain('- Budget: balanced');
    expect(markdown).not.toContain('- Estimated review checks: 3');
    expect(markdown).not.toContain('- Recommended strategy: Deep');
    expect(markdown).not.toContain('- Recommendation score: 24');
    expect(markdown).not.toContain('- Recommendation rationale: Large/high-risk change');
    expect(markdown).not.toContain('- Included coverage: 3 coverage areas.');
    expect(markdown).not.toContain('- Review scope tailored: 2 optional checks outside this run.');
    expect(markdown).not.toContain('Logic reviewer - Logic reviewer');
    expect(markdown).not.toContain('Custom security reviewer - Custom security reviewer');
    expect(markdown).not.toContain('Quality inspector - Quality inspector');
    expect(markdown).not.toContain('Frontend reviewer');
    expect(markdown).not.toContain('not_applicable');
    expect(markdown).not.toContain('Custom invalid reviewer');
    expect(markdown).not.toContain('invalid_tooling');
    expect(markdown).not.toContain('### Pre-review summary');
    expect(markdown).not.toContain('- 1 file, 12 changed lines across 1 workspace area: web-ui (1)');
    expect(markdown).not.toContain('- web-ui: 1 file (src/App.tsx)');
    expect(markdown).not.toContain('### Context reuse');
    expect(markdown).not.toContain('- Reused context entries: 1');
    expect(markdown).not.toContain('### Incremental coverage');
    expect(markdown).not.toContain('- Reused review work: 2');
    expect(markdown).not.toContain('- Refresh triggers: target_file_set_changed, target_line_count_changed, reviewer_roster_changed');
    expect(markdown).toContain('- Review scope tailored [info/manifest]: Count: 2');
    expect(markdown).toContain('- Token budget limited review coverage [warning/manifest]: Count: 1');
    expect(markdown).not.toContain('reviewer:ReviewBusinessLogic');
    expect(markdown).not.toContain('shared-context:1');
    expect(markdown).not.toContain('incremental-review:abc12345');
    expect(markdown).not.toContain('Fingerprint: abc12345');
  });
});
