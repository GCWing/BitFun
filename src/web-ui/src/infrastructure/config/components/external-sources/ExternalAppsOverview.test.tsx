// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ExternalAppsOverview } from './ExternalAppsOverview';
import type { ExternalApplicationView } from './applicationModel';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const application: ExternalApplicationView = {
  ecosystemId: 'opencode',
  displayName: 'OpenCode',
  mode: 'custom',
  status: 'connected_custom',
  primaryAction: 'manage',
  enabled: true,
  counts: { commands: 1, tools: 1, agents: 1, mcps: 1 },
  activeCapabilities: [{ capabilityId: 'tool', count: 1 }],
  sourceCount: 1,
  locations: ['~/.config/opencode'],
  attentionCount: 0,
  connectPlan: [
    { capabilityId: 'command', recommendedAccess: 'auto', effectiveAccess: 'disabled', count: 1 },
    { capabilityId: 'tool', recommendedAccess: 'ask_before_use', effectiveAccess: 'auto', count: 1 },
    { capabilityId: 'subagent', recommendedAccess: 'ask_before_use', effectiveAccess: 'ask_before_use', count: 1 },
    { capabilityId: 'mcp', recommendedAccess: 'ask_before_use', effectiveAccess: 'discover_only', count: 1 },
  ],
};

describe('ExternalAppsOverview', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('labels expanded capabilities with authoritative effective access', async () => {
    await act(async () => {
      root.render(
        <ExternalAppsOverview
          applications={[application]}
          t={((key: string) => key) as never}
          totalAttentionCount={0}
          busy={false}
          canMutate
          policiesEnabled
          onToggle={vi.fn()}
          onOpenAdvanced={vi.fn()}
        />,
      );
    });

    const expand = container.querySelector<HTMLButtonElement>(
      '.bitfun-external-sources-config__app-expand',
    );
    await act(async () => expand?.click());

    const rows = Array.from(container.querySelectorAll(
      '.bitfun-external-sources-config__app-capability',
    ));
    expect(rows.map((row) => row.textContent)).toEqual([
      'applications.capabilities.commandapplications.detail.foundCountapplications.capabilityAccess.disabled',
      'applications.capabilities.toolapplications.detail.foundCountapplications.capabilityAccess.auto',
      'applications.capabilities.agentsapplications.detail.foundCountapplications.capabilityAccess.ask_before_use',
      'applications.capabilities.mcpsapplications.detail.foundCountapplications.capabilityAccess.discover_only',
    ]);
  });

  it('renders only the V2 Host status by default instead of inventing capability details', async () => {
    await act(async () => {
      root.render(
        <ExternalAppsOverview
          applications={[{
            ...application,
            mode: undefined,
            status: 'temporarily_unavailable',
            primaryAction: 'retry',
            enabledCount: 2,
            activeCapabilities: [],
          }]}
          t={((key: string) => key) as never}
          totalAttentionCount={0}
          busy={false}
          canMutate
          policiesEnabled
          onToggle={vi.fn()}
          onOpenAdvanced={vi.fn()}
        />,
      );
    });

    expect(container.textContent).toContain('applications.status.temporarily_unavailable');
    expect(container.textContent).not.toContain('applications.summary.enabledCount');
    expect(container.textContent).not.toContain('applications.summary.noContent');
    expect(container.querySelector('button[aria-label^="applications.expand"]')).toBeNull();
  });

  it('does not let a disconnected V2 row bypass its Host primary action', async () => {
    await act(async () => {
      root.render(
        <ExternalAppsOverview
          applications={[{
            ...application,
            applicationId: 'opencode',
            mode: undefined,
            status: 'needs_attention',
            primaryAction: 'review',
            enabled: false,
            enabledCount: 0,
            activeCapabilities: [],
          }]}
          t={((key: string) => key) as never}
          totalAttentionCount={1}
          busy={false}
          canMutate
          policiesEnabled
          onToggle={vi.fn()}
          onOpenAdvanced={vi.fn()}
        />,
      );
    });

    expect(container.querySelector<HTMLInputElement>(
      '[data-bf-part="applicationToggle"] input[type="checkbox"]',
    )?.disabled).toBe(true);
  });

  it('keeps connected as the Host status while surfacing degraded secondary facts', async () => {
    await act(async () => {
      root.render(
        <ExternalAppsOverview
          applications={[{
            ...application,
            applicationId: 'opencode',
            mode: undefined,
            status: 'connected',
            primaryAction: 'view',
            enabledCount: 2,
            health: 'degraded',
            blockedCount: 2,
            conflictCount: 1,
            recoveryActions: [{ type: 'refresh' }],
            activeCapabilities: [],
          }]}
          t={((key: string) => key) as never}
          totalAttentionCount={0}
          busy={false}
          canMutate
          policiesEnabled
          onToggle={vi.fn()}
          onOpenAdvanced={vi.fn()}
        />,
      );
    });

    expect(container.textContent).toContain('applications.status.connected');
    expect(container.textContent).not.toContain('applications.summary.health.degraded');
    const details = container.querySelector<HTMLElement>(
      '[data-bf-part="applicationFacts"]',
    );
    expect(details?.getAttribute('aria-label')).toContain('applications.summary.health.degraded');
    expect(details?.getAttribute('aria-label')).toContain('applications.summary.blockedCount');
    expect(details?.getAttribute('aria-label')).toContain('applications.summary.conflictCount');
    expect(details?.getAttribute('aria-label')).toContain('recoveryActions.refresh');
  });

  it('explains a single review choice in one standard settings row', async () => {
    await act(async () => {
      root.render(
        <ExternalAppsOverview
          applications={[]}
          t={((key: string) => key) as never}
          totalAttentionCount={1}
          busy={false}
          canMutate
          policiesEnabled
          onToggle={vi.fn()}
          onOpenAdvanced={vi.fn()}
          review={{
            open: true,
            loading: false,
            items: [{
              itemRef: { kind: 'tool', stableId: 'tool-a' },
              displayName: 'Tool A',
              displaySummary: 'Run an untranslated local process',
              riskLevel: 'moderate',
              riskReasonCodes: ['process_or_resource_access'],
              recommended: true,
              safetyCeiling: 'review_required',
            }],
            selected: {},
            selectedCount: 1,
            recommendedCount: 1,
            totalCount: 1,
            maxSelectionCount: 1,
            applicationNames: ['OpenCode'],
            itemResults: [],
            completed: false,
            canSubmit: true,
            onClose: vi.fn(),
            onToggleItem: vi.fn(),
            onLoadMore: vi.fn(),
            onSubmit: vi.fn(),
          }}
        />,
      );
    });

    const decision = container.querySelector('.bitfun-external-sources-config__review-decision');
    expect(decision?.classList.contains('bitfun-config-page-row')).toBe(true);
    expect(decision?.textContent).toContain('OpenCode');
    expect(decision?.textContent).toContain('Tool A');
    expect(decision?.textContent).toContain('applications.review.category.tool');
    expect(decision?.textContent).toContain('applications.review.risk.moderate');
    expect(decision?.textContent)
      .toContain('applications.review.riskReason.processOrResourceAccess');
    expect(decision?.textContent).toContain('applications.review.recommendation.enable');
    expect(decision?.querySelectorAll('br')).toHaveLength(1);
    expect(container.textContent).not.toContain('Run an untranslated local process');
    expect(container.querySelector('.bitfun-external-sources-config__review-adjustments'))
      .toBeNull();
    expect(container.querySelector(
      '[data-bf-part="submitReview"][data-review-baseline="recommended"]',
    )?.textContent).toBe('applications.review.enableThisItem');
    expect(container.querySelector(
      '[data-bf-part="submitReview"][data-review-baseline="none"]',
    )?.textContent).toBe('applications.review.doNotEnable');
    expect(container.querySelector('[data-bf-part="attentionSummary"]')).toBeNull();
  });

  it('offers both explicit choices for one selectable item that is disabled by default', async () => {
    const item = {
      itemRef: { kind: 'subagent' as const, stableId: 'agent-a' },
      displayName: 'External agent',
      displaySummary: 'Untranslated backend copy',
      riskLevel: 'high' as const,
      riskReasonCodes: ['delegated_tool_access'],
      recommended: false,
      safetyCeiling: 'review_required' as const,
    };
    const onSubmit = vi.fn();

    await act(async () => {
      root.render(
        <ExternalAppsOverview
          applications={[]}
          t={((key: string) => key) as never}
          totalAttentionCount={1}
          busy={false}
          canMutate
          policiesEnabled
          onToggle={vi.fn()}
          onOpenAdvanced={vi.fn()}
          review={{
            open: true,
            loading: false,
            items: [item],
            selected: {},
            selectedCount: 0,
            recommendedCount: 0,
            totalCount: 1,
            maxSelectionCount: 1,
            applicationNames: ['OpenCode'],
            itemResults: [],
            completed: false,
            canSubmit: true,
            onClose: vi.fn(),
            onToggleItem: vi.fn(),
            onLoadMore: vi.fn(),
            onSubmit,
          }}
        />,
      );
    });

    expect(container.querySelector('.bitfun-external-sources-config__review-adjustments'))
      .toBeNull();
    expect(container.textContent)
      .toContain('applications.review.riskReason.delegatedToolAccess');
    expect(container.querySelector(
      '[data-bf-part="submitReview"][data-review-baseline="none"]',
    )?.textContent).toBe('applications.review.keepDisabled');
    const enable = container.querySelector<HTMLButtonElement>(
      '[data-bf-part="submitReview"][data-review-baseline="recommended"]',
    );
    expect(enable?.textContent).toBe('applications.review.enableThisItem');
    await act(async () => enable?.click());
    expect(onSubmit).toHaveBeenCalledWith('recommended', { item, selected: true });
  });

  it('shows only a compact progress state while the first review page loads', async () => {
    await act(async () => {
      root.render(
        <ExternalAppsOverview
          applications={[]}
          t={((key: string) => key) as never}
          totalAttentionCount={2}
          busy={false}
          canMutate
          policiesEnabled
          onToggle={vi.fn()}
          onOpenAdvanced={vi.fn()}
          review={{
            open: true,
            loading: true,
            items: [],
            selected: {},
            selectedCount: 0,
            recommendedCount: 0,
            totalCount: 2,
            maxSelectionCount: 2,
            applicationNames: ['OpenCode'],
            itemResults: [],
            completed: false,
            canSubmit: true,
            onClose: vi.fn(),
            onToggleItem: vi.fn(),
            onLoadMore: vi.fn(),
            onSubmit: vi.fn(),
          }}
        />,
      );
    });

    expect(container.querySelector('[role="status"]')?.textContent)
      .toBe('applications.review.loading');
    expect(container.querySelector('[data-bf-part="attentionSummary"]')).toBeNull();
    expect(container.querySelector('[data-bf-part="submitReview"]')).toBeNull();
    expect(container.querySelector('.bitfun-external-sources-config__review-adjustments'))
      .toBeNull();
    expect(container.querySelector('.bitfun-external-sources-config__app-list')).toBeNull();
  });
});
