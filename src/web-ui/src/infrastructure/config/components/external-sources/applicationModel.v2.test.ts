import { describe, expect, it } from 'vitest';
import { buildExternalApplicationsViewV2 } from './applicationModel';

function application(overrides: Record<string, unknown> = {}) {
  return {
    applicationId: 'opencode',
    ecosystemId: 'opencode',
    displayName: 'OpenCode',
    discovery: 'discovered',
    connection: 'connected',
    desiredConnection: 'connected',
    health: 'healthy',
    effectiveStatus: 'connected',
    primaryAction: 'view',
    defaultConnectionPolicy: 'connect',
    defaultConnectionReason: 'supported_by_product',
    enabledCount: 2,
    pendingReviewCount: 0,
    blockedCount: 0,
    conflictCount: 0,
    riskSummary: { highestLevel: null, reasonCodes: [] },
    noticeKey: null,
    userDecision: 'connected',
    recoveryActions: [],
    ...overrides,
  };
}

function snapshot(applications: Array<Record<string, unknown>>, totalAttentionCount = 0) {
  return {
    schemaVersion: 2,
    executionDomainId: 'host-a',
    workspaceScopeId: 'workspace:0123456789abcdef',
    effectiveConnectionScope: 'workspace_override',
    refreshGeneration: 7,
    preferenceRevision: 11,
    safeMode: false,
    hostCapabilities: {},
    applications,
    reviewSummary: totalAttentionCount > 0
      ? {
          reviewId: 'review-7',
          totalCount: totalAttentionCount,
          categoryCounts: [],
          maxSelectionCount: totalAttentionCount,
          riskSummary: { highestLevel: 'high', reasonCodes: ['process_execution'] },
          recommendationSummary: {
            recommendedCount: 1,
            optionalCount: 0,
            blockedCount: 0,
          },
          safetyCeiling: 'review_required',
        }
      : null,
  };
}

describe('external application V2 model', () => {
  it('projects the Host status, primary action, and connection without V1 policy inference', () => {
    const result = buildExternalApplicationsViewV2(snapshot([
      application({
        effectiveStatus: 'needs_attention',
        primaryAction: 'review',
        pendingReviewCount: 3,
      }),
    ], 7) as never);

    expect(result.totalAttentionCount).toBe(7);
    expect(result.applications[0]).toMatchObject({
      ecosystemId: 'opencode',
      status: 'needs_attention',
      primaryAction: 'review',
      enabled: true,
      enabledCount: 2,
      attentionCount: 3,
    });
  });

  it.each([
    ['connected', 'view'],
    ['configuration_available', 'connect'],
    ['no_configuration', 'none'],
    ['needs_attention', 'review'],
    ['temporarily_unavailable', 'retry'],
  ])('preserves the authoritative %s state', (effectiveStatus, primaryAction) => {
    const result = buildExternalApplicationsViewV2(snapshot([
      application({ effectiveStatus, primaryAction }),
    ]) as never);

    expect(result.applications[0].status).toBe(effectiveStatus);
    expect(result.applications[0].primaryAction).toBe(primaryAction);
  });

  it('preserves secondary Host health, issue counts, and recovery facts', () => {
    const result = buildExternalApplicationsViewV2(snapshot([
      application({
        health: 'degraded',
        blockedCount: 2,
        conflictCount: 1,
        recoveryActions: [{ type: 'refresh' }],
      }),
    ]) as never);

    expect(result.applications[0]).toMatchObject({
      status: 'connected',
      primaryAction: 'view',
      health: 'degraded',
      blockedCount: 2,
      conflictCount: 1,
      recoveryActions: [{ type: 'refresh' }],
    });
  });
});
