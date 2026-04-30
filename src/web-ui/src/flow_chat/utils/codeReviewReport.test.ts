import { describe, expect, it } from 'vitest';
import {
  buildCodeReviewReportSections,
  buildCodeReviewReliabilityNotices,
  formatCodeReviewReportMarkdown,
  getDefaultExpandedCodeReviewSectionIds,
} from './codeReviewReport';
import type { ReviewTeamManifestMember, ReviewTeamRunManifest } from '@/shared/services/reviewTeamService';

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
      rationale: 'Large/high-risk change (8 files, 900 lines; 2 security-sensitive files, 3 workspace areas). Deep review recommended.',
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

  it('surfaces partial reviewer output in coverage notes', () => {
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
      'Security Reviewer timed out after producing partial output: Found likely token logging in src/auth.ts before timeout.',
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
      'compression_preserved',
      'partial_reviewer',
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
          detail: 'Runtime profile capped reviewer fan-out for this large target.',
        },
        {
          kind: 'compression_preserved',
          severity: 'info',
          source: 'runtime',
          detail: 'Compression contract retained modified files and failed commands.',
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
        detail: 'Runtime profile capped reviewer fan-out for this large target.',
      },
      {
        kind: 'compression_preserved',
        severity: 'info',
        source: 'runtime',
        detail: 'Compression contract retained modified files and failed commands.',
      },
    ]);

    const markdown = formatCodeReviewReportMarkdown(report);

    expect(markdown).toContain('## Review Reliability');
    expect(markdown).toContain(
      '- Context pressure rising [warning/runtime]: Runtime profile capped reviewer fan-out for this large target.',
    );
    expect(markdown).toContain(
      '- Compression preserved key facts [info/runtime]: Compression contract retained modified files and failed commands.',
    );
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

    expect(markdown).toContain('# Deep Review Report');
    expect(markdown).toContain('## Executive Summary');
    expect(markdown).toContain('- One fix required.');
    expect(markdown).toContain('## Issues');
    expect(markdown).toContain('src/auth.ts:12');
    expect(markdown).toContain('## Remediation Plan');
    expect(markdown).toContain('## Code Review Team');
  });

  it('exports partial reviewer output in markdown', () => {
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

    expect(markdown).toContain('Security Reviewer (security; Status: partial_timeout)');
    expect(markdown).toContain('Partial output: Found likely token logging in src/auth.ts before timeout.');
    expect(markdown).toContain(
      'Security Reviewer timed out after producing partial output: Found likely token logging in src/auth.ts before timeout.',
    );
  });

  it('exports reviewer packet fallback metadata in markdown', () => {
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

    expect(markdown).toContain('Packet: reviewer:ReviewSecurity:group-1-of-3 (inferred)');
  });

  it('includes the run manifest when exporting a deep review report', () => {
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

    expect(markdown).toContain('## Run manifest');
    expect(markdown).toContain('- Target: frontend');
    expect(markdown).toContain('- Budget: balanced');
    expect(markdown).toContain('- Estimated calls: 3');
    expect(markdown).toContain('- Recommended strategy: deep');
    expect(markdown).toContain('- Recommendation score: 24');
    expect(markdown).toContain('- Recommendation rationale: Large/high-risk change');
    expect(markdown).toContain('- Logic reviewer (ReviewBusinessLogic)');
    expect(markdown).toContain('- Custom security reviewer (CustomSecurity)');
    expect(markdown).toContain('- Quality inspector (ReviewJudge)');
    expect(markdown).toContain('- Frontend reviewer (ReviewFrontend): not_applicable');
    expect(markdown).toContain('- Custom invalid reviewer (CustomInvalid): invalid_tooling');
    expect(markdown).toContain('### Pre-review summary');
    expect(markdown).toContain('- 1 file, 12 changed lines across 1 workspace area: web-ui (1)');
    expect(markdown).toContain('- web-ui: 1 file (src/App.tsx)');
    expect(markdown).toContain('### Shared context cache');
    expect(markdown).toContain('- shared-context:1: src/App.tsx -> reviewer:ReviewBusinessLogic, reviewer:CustomSecurity');
    expect(markdown).toContain('### Incremental review cache');
    expect(markdown).toContain('- Cache key: incremental-review:abc12345');
    expect(markdown).toContain('- Fingerprint: abc12345');
    expect(markdown).toContain('- Invalidates on: target_file_set_changed, target_line_count_changed, reviewer_roster_changed');
  });
});
