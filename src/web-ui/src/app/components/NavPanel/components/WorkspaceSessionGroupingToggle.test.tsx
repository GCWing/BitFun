// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@bitfun/ui', async importOriginal => ({
  ...await importOriginal<typeof import('@bitfun/ui')>(),
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('@/infrastructure/i18n', async () => {
  const { createTestI18nT } = await import('@/test/i18nTestUtils');
  return { useI18n: () => ({ t: createTestI18nT('common') }) };
});

import WorkspaceSessionGroupingToggle from './WorkspaceSessionGroupingToggle';
import { useWorkspaceSessionViewStore } from '../workspaceSessionView';

describe('WorkspaceSessionGroupingToggle', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    localStorage.clear();
    useWorkspaceSessionViewStore.getState().setGrouping('grouped');
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    act(() => root.render(<WorkspaceSessionGroupingToggle />));
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('uses one semantic icon toggle backed by the persisted grouping state', () => {
    const toggle = container.querySelector<HTMLButtonElement>(
      '[data-testid="nav-workspace-session-view-toggle"]',
    );
    expect(toggle?.dataset.viewMode).toBe('grouped');
    expect(toggle?.getAttribute('aria-pressed')).toBe('false');
    expect(toggle?.querySelector('[data-session-view-icon="grouped"]')).not.toBeNull();
    expect(toggle?.querySelector('[data-session-view-icon="all"]')).toBeNull();

    act(() => toggle!.click());
    expect(useWorkspaceSessionViewStore.getState().grouping).toBe('all');
    expect(toggle?.dataset.viewMode).toBe('all');
    expect(toggle?.getAttribute('aria-pressed')).toBe('true');
    expect(toggle?.querySelector('[data-session-view-icon="all"]')).not.toBeNull();
    expect(toggle?.querySelector('[data-session-view-icon="grouped"]')).toBeNull();

    act(() => toggle!.click());
    expect(useWorkspaceSessionViewStore.getState().grouping).toBe('grouped');
    expect(toggle?.dataset.viewMode).toBe('grouped');
  });
});
