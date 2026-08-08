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

  it('renders the V2 Host status and aggregate instead of inventing capability types', async () => {
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
    expect(container.textContent).toContain('applications.summary.enabledCount');
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
    expect(container.textContent).toContain('applications.summary.health.degraded');
    expect(container.textContent).toContain('applications.summary.blockedCount');
    expect(container.textContent).toContain('applications.summary.conflictCount');
    expect(container.textContent).toContain('recoveryActions.refresh');
  });
});
