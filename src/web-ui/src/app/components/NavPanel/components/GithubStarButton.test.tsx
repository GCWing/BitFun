// @vitest-environment jsdom

import React from 'react';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const openExternal = vi.hoisted(() => vi.fn());

vi.mock('@/infrastructure/i18n/hooks/useI18n', async () => {
  const { createTestI18nT } = await import('@/test/i18nTestUtils');
  return { useI18n: () => ({ t: createTestI18nT('common') }) };
});

vi.mock('@/component-library', () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('@/infrastructure/api/service-api/SystemAPI', () => ({
  systemAPI: { openExternal },
}));

import GithubStarButton from './GithubStarButton';
import { GITHUB_STAR_CTA_DISMISSED_KEY, GITHUB_STAR_URL } from './githubStarCtaStorage';

let container: HTMLDivElement;
let root: Root;
/** This jsdom environment ships no localStorage, so back the storage module with a stub. */
let store: Map<string, string>;

function render(): void {
  act(() => {
    root.render(<GithubStarButton />);
  });
}

function click(testId: string): void {
  const button = container.querySelector<HTMLButtonElement>(`[data-testid="${testId}"]`);
  expect(button).not.toBeNull();
  act(() => {
    button!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
  });
}

const starButton = () => container.querySelector('[data-testid="nav-footer-github-star-btn"]');

describe('GithubStarButton', () => {
  beforeEach(() => {
    openExternal.mockReset().mockResolvedValue(undefined);
    store = new Map();
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => store.get(key) ?? null,
      setItem: (key: string, value: string) => void store.set(key, value),
    });
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.unstubAllGlobals();
  });

  it('renders the localized star label until it is retired', () => {
    render();

    expect(starButton()).not.toBeNull();
    expect(container.textContent).toContain('Star');
    expect(container.querySelector('[data-testid="nav-footer-github-star-dismiss"]')).toBeNull();
  });

  it('opens the repository in the system browser and retires itself', () => {
    render();
    click('nav-footer-github-star-btn');

    expect(openExternal).toHaveBeenCalledWith(GITHUB_STAR_URL);
    expect(starButton()).toBeNull();
    expect(store.get(GITHUB_STAR_CTA_DISMISSED_KEY)).toBe('true');
  });

  it('stays retired on later mounts', () => {
    store.set(GITHUB_STAR_CTA_DISMISSED_KEY, 'true');
    render();

    expect(starButton()).toBeNull();
  });
});
