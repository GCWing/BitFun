// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { WorkspaceInfo } from '@/shared/types';
import AssistantCard from './AssistantCard';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('@openbitfun/ui', async importOriginal => ({
  ...await importOriginal<typeof import('@openbitfun/ui')>(),
  IconButton: ({
    children,
    loading,
    size: _size,
    tooltip: _tooltip,
    variant: _variant,
    ...props
  }: React.PropsWithChildren<React.ButtonHTMLAttributes<HTMLButtonElement> & {
    loading?: boolean;
    size?: string;
    tooltip?: React.ReactNode;
    variant?: string;
  }>) => (
    <button {...props} aria-busy={loading || undefined}>
      {children}
    </button>
  ),
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

const workspace = {
  id: 'assistant-1',
  name: 'Research assistant',
  rootPath: '/tmp/assistant-1',
  identity: {
    name: 'Mira',
    emoji: '🧭',
    creature: 'Researcher',
    vibe: 'Methodical and concise',
  },
} as WorkspaceInfo;

describe('AssistantCard actions', () => {
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

  it('keeps configuration and new-session actions distinct', () => {
    const onConfigure = vi.fn();
    const onNewSession = vi.fn();
    const onDelete = vi.fn();
    const onSetPrimary = vi.fn();

    act(() => {
      root.render(
        <div role="list">
          <AssistantCard
            workspace={workspace}
            onClick={onConfigure}
            onNewSession={onNewSession}
            onDelete={onDelete}
            onSetPrimary={onSetPrimary}
          />
        </div>,
      );
    });

    const card = container.querySelector('[role="listitem"]');
    const configure = container.querySelector('.assistant-card__main') as HTMLButtonElement;
    const newSession = container.querySelector(
      'button[data-openbitfun-component="button"][data-openbitfun-variant="fill"]',
    ) as HTMLButtonElement;
    const remove = container.querySelector('[data-openbitfun-part="delete"]') as HTMLButtonElement;
    const setPrimary = container.querySelector('[data-openbitfun-part="setPrimary"]') as HTMLButtonElement;

    expect(card?.tagName).toBe('ARTICLE');
    expect(configure.getAttribute('aria-label')).toContain('Mira');

    act(() => configure.click());
    expect(onConfigure).toHaveBeenCalledTimes(1);
    expect(onNewSession).not.toHaveBeenCalled();

    act(() => newSession.click());
    expect(onNewSession).toHaveBeenCalledTimes(1);
    expect(onConfigure).toHaveBeenCalledTimes(1);

    act(() => setPrimary.click());
    expect(onSetPrimary).toHaveBeenCalledTimes(1);
    expect(onConfigure).toHaveBeenCalledTimes(1);

    act(() => remove.click());
    expect(onDelete).toHaveBeenCalledTimes(1);
    expect(onConfigure).toHaveBeenCalledTimes(1);
  });

  it('disables card actions while setting the primary assistant', () => {
    act(() => {
      root.render(
        <div role="list">
          <AssistantCard
            workspace={workspace}
            onClick={vi.fn()}
            onNewSession={vi.fn()}
            onDelete={vi.fn()}
            onSetPrimary={vi.fn()}
            isSettingPrimary
          />
        </div>,
      );
    });

    const configure = container.querySelector('.assistant-card__main') as HTMLButtonElement;
    const newSession = container.querySelector(
      'button[data-openbitfun-component="button"][data-openbitfun-variant="fill"]',
    ) as HTMLButtonElement;
    const setPrimary = container.querySelector('[data-openbitfun-part="setPrimary"]') as HTMLButtonElement;
    const remove = container.querySelector('[data-openbitfun-part="delete"]') as HTMLButtonElement;
    expect(configure.disabled).toBe(true);
    expect(newSession.disabled).toBe(true);
    expect(setPrimary.disabled).toBe(true);
    expect(setPrimary.getAttribute('aria-busy')).toBe('true');
    expect(remove.disabled).toBe(true);
  });

  it('disables session actions while a new session is starting', () => {
    act(() => {
      root.render(
        <div role="list">
          <AssistantCard
            workspace={workspace}
            onClick={vi.fn()}
            onNewSession={vi.fn()}
            isStartingSession
          />
        </div>,
      );
    });

    const newSession = container.querySelector(
      'button[data-openbitfun-component="button"][data-openbitfun-variant="fill"]',
    ) as HTMLButtonElement;
    expect(newSession.disabled).toBe(true);
    expect(newSession.getAttribute('aria-busy')).toBe('true');
    expect(newSession.textContent).toContain('nursery.card.startingSession');
  });
});
