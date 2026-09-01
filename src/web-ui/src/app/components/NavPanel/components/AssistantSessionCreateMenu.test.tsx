// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { WorkspaceKind, WorkspaceType, type WorkspaceInfo } from '@/shared/types';

vi.mock('@bitfun/ui', async importOriginal => ({
  ...await importOriginal<typeof import('@bitfun/ui')>(),
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('@/infrastructure/appearance/runtime/AppearanceOverlayHost', () => ({
  getAppearanceOverlayHost: () => document.body,
}));

vi.mock('@/infrastructure/i18n/hooks/useI18n', async () => {
  const { createTestI18nT } = await import('@/test/i18nTestUtils');
  return { useI18n: () => ({ t: createTestI18nT('common') }) };
});

vi.mock('@/shared/utils/useAnchoredPopoverPosition', () => ({
  useAnchoredPopoverPosition: ({ open }: { open: boolean }) =>
    open ? { top: 20, left: 40, placement: 'bottom' } : null,
}));

import AssistantSessionCreateMenu from './AssistantSessionCreateMenu';

const createAssistant = (id: string, name: string, identityName?: string): WorkspaceInfo => ({
  id,
  name,
  rootPath: `/assistants/${id}`,
  workspaceType: WorkspaceType.Other,
  workspaceKind: WorkspaceKind.Assistant,
  assistantId: id,
  languages: [],
  openedAt: '2026-08-06T00:00:00.000Z',
  lastAccessed: '2026-08-06T00:00:00.000Z',
  tags: [],
  identity: identityName ? { name: identityName } : null,
});

describe('AssistantSessionCreateMenu', () => {
  let container: HTMLDivElement;
  let root: Root;
  const primaryAssistant = createAssistant('primary', 'Built-in assistant', 'Mira');
  const secondaryAssistant = createAssistant('secondary', 'Research assistant', 'Sage');

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  const renderMenu = (
    onCreatePrimary = vi.fn(),
    onCreateAssistant = vi.fn(),
  ) => {
    act(() => {
      root.render(
        <AssistantSessionCreateMenu
          assistants={[secondaryAssistant, primaryAssistant]}
          primaryAssistant={primaryAssistant}
          onCreatePrimary={onCreatePrimary}
          onCreateAssistant={onCreateAssistant}
        />,
      );
    });
    return { onCreatePrimary, onCreateAssistant };
  };

  const click = (testId: string) => {
    const button = document.querySelector<HTMLButtonElement>(`[data-testid="${testId}"]`);
    expect(button).not.toBeNull();
    act(() => button!.click());
  };

  it('uses the main action to create a session for the primary assistant', () => {
    const { onCreatePrimary, onCreateAssistant } = renderMenu();

    click('nav-primary-assistant-session-add-btn');

    expect(onCreatePrimary).toHaveBeenCalledTimes(1);
    expect(onCreateAssistant).not.toHaveBeenCalled();
  });

  it('lists the primary assistant first without a badge and creates a session for the selected assistant', () => {
    const { onCreateAssistant } = renderMenu();

    click('nav-assistant-session-menu-toggle');
    const items = [...document.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')];
    expect(items.map(item => item.textContent)).toEqual(['Mira', 'Sage']);

    click('nav-assistant-session-menu-item-secondary');
    expect(onCreateAssistant).toHaveBeenCalledWith(secondaryAssistant);
    expect(document.querySelector('[data-testid="nav-assistant-session-menu"]')).toBeNull();
  });

  it('closes the assistant menu on Escape', () => {
    renderMenu();
    click('nav-assistant-session-menu-toggle');

    const toggle = container.querySelector<HTMLButtonElement>('[data-testid="nav-assistant-session-menu-toggle"]');
    act(() => toggle!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })));

    expect(document.querySelector('[data-testid="nav-assistant-session-menu"]')).toBeNull();
  });
});
