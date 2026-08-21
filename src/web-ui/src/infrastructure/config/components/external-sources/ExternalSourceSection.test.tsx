// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import enUS from '@/locales/en-US/settings/external-apps.json';
import type { ExternalSourcePresentationGroup } from '../../externalSourcePresentation';
import { ExternalSourceSection } from './ExternalSourceSection';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

function translate(key: string): string {
  const value = key.split('.').reduce<unknown>((current, segment) => (
    current && typeof current === 'object'
      ? (current as Record<string, unknown>)[segment]
      : undefined
  ), enUS);
  return typeof value === 'string' ? value : key;
}

describe('ExternalSourceSection', () => {
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

  it('renders the localized category for an unavailable MCP runtime', async () => {
    const group: ExternalSourcePresentationGroup = {
      key: 'opencode:mcp',
      ecosystemId: 'opencode',
      scopes: ['user_global'],
      displayName: 'OpenCode MCP',
      location: 'opencode.json',
      lifecycle: 'degraded',
      members: [],
      counts: { commands: 0, tools: 0, agents: 0, mcps: 1 },
      diagnostics: [{
        severity: 'warning',
        code: 'external_mcp.runtime_unavailable',
        message: 'external_mcp.runtime.failed',
      }],
    };

    await act(async () => {
      root.render(
        <ExternalSourceSection
          groups={[group]}
          t={translate as never}
          renderSourceMembers={() => null}
        />,
      );
    });

    expect(container.textContent).toContain(enUS.diagnostics.category.runtimeUnavailable);
    expect(container.textContent).not.toContain('diagnostics.category.runtimeUnavailable');
    expect(container.textContent).not.toContain('external_mcp.runtime.failed');
  });
});
