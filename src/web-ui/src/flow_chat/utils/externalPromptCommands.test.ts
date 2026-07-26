import { describe, expect, it } from 'vitest';
import type { ExternalSourceCatalogSnapshot } from '@/infrastructure/api/service-api/ExternalSourcesAPI';
import {
  buildExternalPromptCommandItems,
  classifyExternalPromptCommandCatalogIssue,
  externalPromptComposerIsUnchanged,
  isExternalPromptSubmissionTargetCurrent,
  routeUnmatchedExternalPromptCommand,
  resolveExternalPromptCommandInvocation,
} from './externalPromptCommands';
import { ExternalSourceApiError } from '@/infrastructure/api/service-api/ExternalSourcesAPI';

function snapshot(
  overrides: Partial<ExternalSourceCatalogSnapshot> = {},
): ExternalSourceCatalogSnapshot {
  return {
    hostCapabilities: {
      canRefresh: true,
      canMutatePolicy: true,
      canManageSources: true,
      canApproveRuntime: true,
      canExecuteExternalAssets: true,
      canSetSafeMode: true,
      canRevealSourceLocation: true,
    },
    generation: 1,
    discoveryPending: false,
    sources: [{
      stableKey: 'claude-code.commands:project',
      record: {
        key: { providerId: 'claude-code.commands', sourceId: 'project' },
        ecosystemId: 'claude-code',
        displayName: 'Claude Code project commands',
        sourceKind: 'commands',
        scope: 'project',
        location: '.claude/commands',
        executionDomainId: 'local-user',
        health: 'available',
        contentVersion: 'source-v1',
      },
      lifecycle: 'available',
    }],
    commands: [{
      candidateId: 'opaque-claude-review',
      definition: {
        id: {
          source: { providerId: 'claude-code.commands', sourceId: 'project' },
          localId: 'review',
        },
        name: 'review',
        description: 'Review changes',
        availability: { state: 'available' },
        contentVersion: 'behavior-v1',
      },
    }],
    commandConflicts: [],
    integrationPolicy: {} as ExternalSourceCatalogSnapshot['integrationPolicy'],
    ...overrides,
  };
}

describe('external prompt command projection', () => {
  it('projects public catalog facts without inventing a command namespace', () => {
    const items = buildExternalPromptCommandItems(snapshot());

    expect(items).toEqual([expect.objectContaining({
      id: 'opaque-claude-review',
      command: '/review',
      candidateId: 'opaque-claude-review',
      contentVersion: 'behavior-v1',
      label: 'Review changes · Claude Code project commands',
      available: true,
    })]);
    expect(JSON.stringify(items)).not.toContain('/external:');
    expect(JSON.stringify(items)).not.toContain('/builtin:');
  });

  it('uses the opaque host candidate id and does not reproduce Rust encoding', () => {
    const source = snapshot();
    source.commands[0].candidateId = 'opaque:review-命令';
    source.commands[0].definition.id.localId = '命令';

    expect(buildExternalPromptCommandItems(source)[0].candidateId).toBe('opaque:review-命令');

    delete source.commands[0].candidateId;
    expect(buildExternalPromptCommandItems(source)).toEqual([]);
  });

  it('keeps unresolved providers as source-labelled candidates with one plain name', () => {
    const items = buildExternalPromptCommandItems(snapshot({
      commands: [],
      commandConflicts: [{
        conflictKey: 'review-conflict',
        commandName: 'review',
        candidates: [
          {
            candidateId: 'claude-review',
            source: { providerId: 'claude-code.commands', sourceId: 'project' },
            sourceDisplayName: 'Claude Code project commands',
            ecosystemId: 'claude-code',
            contentVersion: 'claude-v1',
            commandDescription: 'Review with Claude conventions',
            sourceScope: 'project',
            sourceLocation: '.claude/commands',
            availability: { state: 'available' },
          },
          {
            candidateId: 'opencode-review',
            source: { providerId: 'opencode.commands', sourceId: 'project' },
            sourceDisplayName: 'OpenCode project commands',
            ecosystemId: 'opencode',
            contentVersion: 'opencode-v1',
            commandDescription: 'Review with OpenCode conventions',
            sourceScope: 'project',
            sourceLocation: '.opencode/commands',
            availability: { state: 'available' },
          },
        ],
      }],
      preferenceRevision: 9,
    }));

    expect(items.map(item => item.command)).toEqual(['/review', '/review']);
    expect(items.map(item => item.label)).toEqual([
      'Review with Claude conventions · Claude Code project commands',
      'Review with OpenCode conventions · OpenCode project commands',
    ]);
    expect(items.every(item => item.conflictKey === 'review-conflict')).toBe(true);
    expect(items.every(item => item.expectedPreferenceRevision === 9)).toBe(true);
  });
});

describe('external prompt command invocation resolution', () => {
  const items = buildExternalPromptCommandItems(snapshot());

  it('preserves arguments and the guarded candidate selected from the picker', () => {
    expect(resolveExternalPromptCommandInvocation(
      '/review focus on auth',
      items,
      new Set(['/review']),
      items[0].candidateId,
    )).toEqual({
      state: 'resolved',
      item: items[0],
      arguments: 'focus on auth',
    });
  });

  it('fails closed for direct names that collide with another product command', () => {
    expect(resolveExternalPromptCommandInvocation(
      '/review focus on auth',
      items,
      new Set(['/review']),
    )).toEqual({ state: 'conflict', command: '/review' });
  });

  it('routes an unambiguous direct external command and ignores ordinary prompts', () => {
    expect(resolveExternalPromptCommandInvocation(
      '/review focus on auth',
      items,
      new Set(),
    )).toEqual({
      state: 'resolved',
      item: items[0],
      arguments: 'focus on auth',
    });
    expect(resolveExternalPromptCommandInvocation('please review', items, new Set()))
      .toEqual({ state: 'none' });
  });
});

describe('external prompt command lifecycle guards', () => {
  it('distinguishes unsupported hosts from retryable catalog failures', () => {
    expect(classifyExternalPromptCommandCatalogIssue(new ExternalSourceApiError(
      'host_capability_unavailable',
      'not supported',
      false,
    ))).toBe('host_unavailable');
    expect(classifyExternalPromptCommandCatalogIssue(new ExternalSourceApiError(
      'timeout',
      'timed out',
      true,
    ))).toBe('load_failed');
  });

  it('rejects an async expansion result after its session or workspace changes', () => {
    expect(isExternalPromptSubmissionTargetCurrent('session-a', 'session-a', '/repo', '/repo'))
      .toBe(true);
    expect(isExternalPromptSubmissionTargetCurrent('session-a', 'session-b', '/repo', '/repo'))
      .toBe(false);
    expect(isExternalPromptSubmissionTargetCurrent('session-a', 'session-a', '/repo', '/other'))
      .toBe(false);
  });

  it('preserves a draft typed while an external command is expanding', () => {
    expect(externalPromptComposerIsUnchanged('/review', '/review')).toBe(true);
    expect(externalPromptComposerIsUnchanged('/review', 'next question')).toBe(false);
  });

  it('waits for unknown slash commands during discovery without blocking known native commands', () => {
    expect(routeUnmatchedExternalPromptCommand({
      hasNativeCommand: false,
      catalogLoading: false,
      discoveryPending: true,
      catalogIssue: undefined,
    })).toBe('wait');
    expect(routeUnmatchedExternalPromptCommand({
      hasNativeCommand: true,
      catalogLoading: false,
      discoveryPending: false,
      catalogIssue: 'load_failed',
    })).toBe('native');
    expect(routeUnmatchedExternalPromptCommand({
      hasNativeCommand: false,
      catalogLoading: false,
      discoveryPending: false,
      catalogIssue: 'load_failed',
    })).toBe('load_failed');
  });
});
