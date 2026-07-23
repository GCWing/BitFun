// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import ExternalHooksPanel from './ExternalHooksPanel';

const getCatalogMock = vi.hoisted(() => vi.fn());

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, params?: Record<string, unknown>) => (
      params ? `${key}:${JSON.stringify(params)}` : key
    ),
  }),
}));

vi.mock('@/component-library', () => ({
  Button: ({ children, disabled, onClick, ...props }: React.ButtonHTMLAttributes<HTMLButtonElement>) => (
    <button type="button" disabled={disabled} onClick={onClick} {...props}>{children}</button>
  ),
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('@/infrastructure/api/service-api/ExternalHooksAPI', () => ({
  externalHooksAPI: { getCatalog: getCatalogMock },
}));

vi.mock('./common', () => ({
  ConfigPageSection: ({ children, title, description, extra }: {
    children: React.ReactNode;
    title: string;
    description?: React.ReactNode;
    extra?: React.ReactNode;
  }) => (
    <section>
      <h2>{title}</h2>
      <div>{description}</div>
      {extra}
      {children}
    </section>
  ),
}));

const snapshot = {
  schemaVersion: 1,
  discoveryPending: false,
  providers: [{
    providerId: 'claude-code.hooks',
    ecosystemId: 'claude-code',
    displayName: 'Claude Code Hooks',
  }],
  sources: [{
    key: { providerId: 'claude-code.hooks', sourceId: 'project-settings' },
    ecosystemId: 'claude-code',
    displayName: 'Claude Code project Hooks',
    sourceKind: 'settings',
    scope: 'project',
    locationHint: '.claude/settings.json',
    health: 'available',
    contentVersion: 'v1',
    diagnostics: [],
  }],
  entries: [{
    stableKey: 'pre-tool-use',
    source: { providerId: 'claude-code.hooks', sourceId: 'project-settings' },
    nativeEvent: 'PreToolUse',
    matcher: { kind: 'pattern', display: 'Bash|Read' },
    handlerKind: 'command',
    projectionStatus: 'mapped',
    nativeActivation: 'unknown',
    mapping: { hookPoint: 'tool_before' },
    contentVersion: 'v1',
  }, {
    stableKey: 'notification',
    source: { providerId: 'claude-code.hooks', sourceId: 'project-settings' },
    nativeEvent: 'Notification',
    matcher: { kind: 'any' },
    handlerKind: 'command',
    projectionStatus: 'native_only',
    nativeActivation: 'unknown',
    contentVersion: 'v1',
  }, {
    stableKey: 'opaque',
    source: { providerId: 'claude-code.hooks', sourceId: 'project-settings' },
    nativeEvent: '<dynamic>',
    matcher: { kind: 'dynamic' },
    handlerKind: 'function',
    projectionStatus: 'opaque',
    nativeActivation: 'unknown',
    contentVersion: 'v1',
  }],
  staleProviderIds: [],
  failedProviderIds: [],
  diagnostics: [],
};

describe('ExternalHooksPanel', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean })
      .IS_REACT_ACT_ENVIRONMENT = true;
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    getCatalogMock.mockReset();
    getCatalogMock.mockResolvedValue(snapshot);
  });

  afterEach(() => {
    vi.useRealTimers();
    act(() => root.unmount());
    container.remove();
  });

  it('renders mapped and native-only Hooks without executable handler details', async () => {
    await act(async () => {
      root.render(<ExternalHooksPanel workspacePath="D:/workspace/project" />);
      await Promise.resolve();
    });

    expect(container.textContent).toContain('PreToolUse');
    expect(container.textContent).toContain('hooks.projection.tool_before');
    expect(container.textContent).toContain('Notification');
    expect(container.textContent).toContain('hooks.projection.nativeOnly');
    expect(container.textContent).toContain('hooks.projection.opaque');
    expect(container.textContent).toContain('hooks.matcherValue.all');
    expect(container.textContent).not.toContain('curl');
    expect(getCatalogMock).toHaveBeenCalledWith('D:/workspace/project', true);
  });

  it('shows source diagnostics once and keeps provider diagnostics separate', async () => {
    const sourceDiagnostic = {
      severity: 'warning',
      assetKind: 'hook',
      code: 'claude.hook.partial',
      message: 'source-only diagnostic',
      source: snapshot.sources[0].key,
    };
    getCatalogMock.mockResolvedValue({
      ...snapshot,
      sources: [{ ...snapshot.sources[0], diagnostics: [sourceDiagnostic] }],
      diagnostics: [
        sourceDiagnostic,
        {
          severity: 'info',
          assetKind: 'hook',
          code: 'claude.hook.coverage',
          message: 'provider-wide diagnostic',
        },
      ],
    });

    await act(async () => {
      root.render(<ExternalHooksPanel workspacePath="D:/workspace/project" />);
      await Promise.resolve();
    });

    expect(container.textContent?.match(/source-only diagnostic/g)).toHaveLength(1);
    expect(container.textContent?.match(/provider-wide diagnostic/g)).toHaveLength(1);
  });

  it('shows a remote-specific unsupported state without reading local Hooks', async () => {
    await act(async () => {
      root.render(
        <ExternalHooksPanel workspacePath="/remote/project" unsupportedReason="remote" />,
      );
      await Promise.resolve();
    });

    expect(container.textContent).toContain('hooks.unsupported.remote');
    expect(getCatalogMock).not.toHaveBeenCalled();
  });

  it('refreshes explicitly without exposing edit or execute controls', async () => {
    await act(async () => {
      root.render(<ExternalHooksPanel workspacePath="D:/workspace/project" />);
      await Promise.resolve();
    });
    const refresh = container.querySelector<HTMLButtonElement>('button[aria-label="hooks.refresh"]');
    expect(refresh).not.toBeNull();
    await act(async () => {
      refresh?.click();
      await Promise.resolve();
    });

    expect(getCatalogMock).toHaveBeenLastCalledWith('D:/workspace/project', true);
    expect(container.querySelector('[data-hook-action="edit"]')).toBeNull();
    expect(container.querySelector('[data-hook-action="execute"]')).toBeNull();
  });

  it('responds to the shared page refresh epoch', async () => {
    await act(async () => {
      root.render(<ExternalHooksPanel workspacePath="D:/workspace/project" refreshEpoch={0} />);
      await Promise.resolve();
    });
    getCatalogMock.mockClear();

    await act(async () => {
      root.render(<ExternalHooksPanel workspacePath="D:/workspace/project" refreshEpoch={1} />);
      await Promise.resolve();
    });

    expect(getCatalogMock).toHaveBeenCalledWith('D:/workspace/project', true);
  });

  it('bounds mounted Hook entries until the user asks for more', async () => {
    const entries = Array.from({ length: 250 }, (_, index) => ({
      ...snapshot.entries[0],
      stableKey: `entry-${index}`,
      nativeEvent: `Event${index}`,
    }));
    getCatalogMock.mockResolvedValue({ ...snapshot, entries });

    await act(async () => {
      root.render(<ExternalHooksPanel workspacePath="D:/workspace/project" />);
      await Promise.resolve();
    });

    expect(container.querySelectorAll('.bitfun-external-hooks__entries > li')).toHaveLength(100);
    const showMore = Array.from(container.querySelectorAll('button')).find(
      (button) => button.textContent?.includes('hooks.showMoreEntries'),
    );
    expect(showMore).toBeDefined();
    await act(async () => showMore?.dispatchEvent(new MouseEvent('click', { bubbles: true })));
    expect(container.querySelectorAll('.bitfun-external-hooks__entries > li')).toHaveLength(200);
  });

  it('uses one entry budget across every visible source in a provider', async () => {
    const sources = Array.from({ length: 20 }, (_, sourceIndex) => ({
      ...snapshot.sources[0],
      key: { providerId: 'claude-code.hooks', sourceId: `source-${sourceIndex}` },
      displayName: `Source ${sourceIndex}`,
    }));
    const entries = sources.flatMap((source, sourceIndex) => (
      Array.from({ length: 100 }, (_, entryIndex) => ({
        ...snapshot.entries[0],
        stableKey: `entry-${sourceIndex}-${entryIndex}`,
        source: source.key,
        nativeEvent: `Event${sourceIndex}_${entryIndex}`,
      }))
    ));
    getCatalogMock.mockResolvedValue({ ...snapshot, sources, entries });

    await act(async () => {
      root.render(<ExternalHooksPanel workspacePath="D:/workspace/project" />);
      await Promise.resolve();
    });

    expect(container.querySelectorAll('.bitfun-external-hooks__entries > li')).toHaveLength(100);
    expect(container.querySelector('button[aria-label*="Claude Code Hooks"]')).not.toBeNull();
  });

  it('keeps empty providers visible and announces the final empty state', async () => {
    getCatalogMock.mockResolvedValue({ ...snapshot, sources: [], entries: [] });

    await act(async () => {
      root.render(<ExternalHooksPanel workspacePath="D:/workspace/project" />);
      await Promise.resolve();
    });

    expect(container.textContent).toContain('Claude Code Hooks');
    expect(container.textContent).toContain('hooks.providerEmpty');
    expect(container.querySelector('[aria-live="polite"]')?.textContent).toContain('hooks.empty');
  });

  it('distinguishes failed empty providers from successful empty discovery', async () => {
    getCatalogMock.mockResolvedValue({
      ...snapshot,
      sources: [],
      entries: [],
      failedProviderIds: ['claude-code.hooks'],
    });

    await act(async () => {
      root.render(<ExternalHooksPanel workspacePath="D:/workspace/project" />);
      await Promise.resolve();
    });

    expect(container.textContent).toContain('hooks.providerFailed');
    expect(container.textContent).toContain('hooks.health.failed');
    expect(container.textContent).not.toContain('hooks.providerEmpty');
  });

  it('does not announce a stale empty catalog as a successful empty discovery', async () => {
    getCatalogMock.mockResolvedValue({
      ...snapshot,
      sources: [],
      entries: [],
      staleProviderIds: ['claude-code.hooks'],
    });

    await act(async () => {
      root.render(<ExternalHooksPanel workspacePath="D:/workspace/project" />);
      await Promise.resolve();
    });

    expect(container.textContent).toContain('hooks.providerStaleEmpty');
    expect(container.textContent).toContain('hooks.health.stale');
    expect(container.textContent).not.toContain('hooks.empty');
  });

  it('bounds provider and catalog diagnostics until the user asks for more', async () => {
    const providerDiagnostics = Array.from({ length: 30 }, (_, index) => ({
      severity: 'warning',
      assetKind: 'hook',
      code: `claude.hook.source_${index}`,
      message: `source diagnostic ${index}`,
      source: snapshot.sources[0].key,
    }));
    const catalogDiagnostics = Array.from({ length: 30 }, (_, index) => ({
      severity: 'warning',
      assetKind: 'hook',
      code: `claude.hook.catalog_${index}`,
      message: `catalog diagnostic ${index}`,
    }));
    getCatalogMock.mockResolvedValue({
      ...snapshot,
      sources: [{ ...snapshot.sources[0], diagnostics: providerDiagnostics }],
      diagnostics: [...providerDiagnostics, ...catalogDiagnostics],
    });

    await act(async () => {
      root.render(<ExternalHooksPanel workspacePath="D:/workspace/project" />);
      await Promise.resolve();
    });

    expect(container.querySelectorAll('.bitfun-external-hooks__diagnostics > li')).toHaveLength(20);
    expect(container.querySelectorAll('.bitfun-external-hooks__catalog-diagnostics li')).toHaveLength(20);
    expect(Array.from(container.querySelectorAll('button')).filter(
      (button) => button.textContent?.includes('hooks.showMoreDiagnostics'),
    )).toHaveLength(2);
  });

  it('polls the cached snapshot until deferred discovery completes', async () => {
    vi.useFakeTimers();
    getCatalogMock
      .mockResolvedValueOnce({ ...snapshot, discoveryPending: true, sources: [], entries: [] })
      .mockResolvedValueOnce({ ...snapshot, discoveryPending: true, sources: [], entries: [] })
      .mockResolvedValueOnce(snapshot);

    await act(async () => {
      root.render(<ExternalHooksPanel workspacePath="D:/workspace/project" />);
      await Promise.resolve();
    });
    expect(container.textContent).toContain('hooks.pending');

    await act(async () => {
      await vi.advanceTimersByTimeAsync(250);
      await Promise.resolve();
    });

    expect(container.textContent).toContain('hooks.pending');

    await act(async () => {
      await vi.advanceTimersByTimeAsync(250);
      await Promise.resolve();
    });

    expect(getCatalogMock).toHaveBeenLastCalledWith('D:/workspace/project', false);
    expect(container.textContent).toContain('PreToolUse');
  });

  it('stops failed polling and allows an explicit refresh to recover', async () => {
    vi.useFakeTimers();
    getCatalogMock
      .mockResolvedValueOnce({ ...snapshot, discoveryPending: true, sources: [], entries: [] })
      .mockRejectedValueOnce(new Error('transport unavailable'))
      .mockResolvedValueOnce(snapshot);

    await act(async () => {
      root.render(<ExternalHooksPanel workspacePath="D:/workspace/project" />);
      await Promise.resolve();
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(250);
      await Promise.resolve();
    });

    expect(container.textContent).toContain('hooks.pollInterrupted');
    expect(container.textContent).not.toContain('hooks.staleAfterRefresh');
    expect(container.textContent).not.toContain('hooks.providerEmpty');
    const refresh = container.querySelector<HTMLButtonElement>('button[aria-label="hooks.refresh"]');
    expect(refresh?.disabled).toBe(false);
    await act(async () => {
      refresh?.click();
      await Promise.resolve();
    });

    expect(getCatalogMock).toHaveBeenLastCalledWith('D:/workspace/project', true);
    expect(container.textContent).toContain('PreToolUse');
  });
});
