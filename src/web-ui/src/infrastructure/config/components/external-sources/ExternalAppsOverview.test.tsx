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
});
