/**
 * @vitest-environment jsdom
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { HarnessProfileSelector } from './HarnessProfileSelector';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) =>
      options?.defaultValue ?? key,
  }),
}));

vi.mock('@/component-library', () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('@/infrastructure/appearance/runtime/AppearanceOverlayHost', () => ({
  getAppearanceOverlayHost: () => document.body,
}));

const notify = vi.hoisted(() => ({ info: vi.fn() }));

vi.mock('@/shared/notification-system', () => ({
  notificationService: notify,
}));

function filledBars(scope: ParentNode): number {
  return scope.querySelectorAll(
    '.bitfun-harness-selector__gauge-bar[data-filled="true"]',
  ).length;
}

describe('HarnessProfileSelector', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean })
      .IS_REACT_ACT_ENVIRONMENT = true;
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.querySelector('.bitfun-harness-selector__menu')?.remove();
    vi.clearAllMocks();
  });

  it('reads the active gear from the gauge rather than from a color alone', async () => {
    await act(async () => {
      root.render(<HarnessProfileSelector active onActivateBalanced={vi.fn()} />);
    });

    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="harness-profile-selector"]',
    );
    expect(trigger?.dataset.harnessGear).toBe('2');
    expect(filledBars(trigger!)).toBe(2);
    expect(trigger?.dataset.harnessPending).toBeUndefined();
    expect(
      container.querySelector('[data-testid="harness-profile-pending-dot"]'),
    ).toBeNull();
  });

  it('marks a gear that has not shaped a turn yet as pending', async () => {
    await act(async () => {
      root.render(<HarnessProfileSelector onActivateBalanced={vi.fn()} />);
    });

    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="harness-profile-selector"]',
    );
    expect(trigger?.dataset.harnessPending).toBe('true');
    expect(
      container.querySelector('[data-testid="harness-profile-pending-dot"]'),
    ).not.toBeNull();
  });

  it('offers the three gears with an ascending gauge and the promise each makes', async () => {
    await act(async () => {
      root.render(<HarnessProfileSelector active onActivateBalanced={vi.fn()} />);
    });
    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="harness-profile-selector"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    const menu = document.querySelector<HTMLElement>('.bitfun-harness-selector__menu');
    expect(menu).not.toBeNull();
    const rows = Array.from(menu!.querySelectorAll<HTMLElement>('[data-bf-part="profile"]'));
    expect(rows.map(row => row.dataset.bfProfile)).toEqual([
      'minimal',
      'balanced',
      'ultimate',
    ]);
    expect(rows.map(row => filledBars(row))).toEqual([1, 2, 3]);
    for (const row of rows) {
      expect(row.querySelector('.bitfun-harness-selector__profile-promise')).not.toBeNull();
    }
    expect(rows[1]?.dataset.bfState).toBe('current');
  });

  it('activates the reachable gear and reports the others as unavailable', async () => {
    const onActivateBalanced = vi.fn();
    await act(async () => {
      root.render(<HarnessProfileSelector onActivateBalanced={onActivateBalanced} />);
    });
    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="harness-profile-selector"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    await act(async () => {
      document.querySelector<HTMLButtonElement>('[data-testid="harness-profile-ultimate"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(notify.info).toHaveBeenCalledTimes(1);
    expect(onActivateBalanced).not.toHaveBeenCalled();

    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="harness-profile-selector"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    await act(async () => {
      document.querySelector<HTMLButtonElement>('[data-testid="harness-profile-balanced"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(onActivateBalanced).toHaveBeenCalledTimes(1);
    expect(document.querySelector('.bitfun-harness-selector__menu')).toBeNull();
  });

  it('keeps a legacy session on its own mode and explains why instead of switching', async () => {
    const onActivateBalanced = vi.fn();
    await act(async () => {
      root.render(
        <HarnessProfileSelector legacySession onActivateBalanced={onActivateBalanced} />,
      );
    });

    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="harness-profile-selector"]',
    );
    expect(trigger?.dataset.harnessLegacy).toBe('true');
    expect(trigger?.dataset.harnessPending).toBeUndefined();

    await act(async () => {
      trigger?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    await act(async () => {
      document.querySelector<HTMLButtonElement>('[data-testid="harness-profile-balanced"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(onActivateBalanced).not.toHaveBeenCalled();
    expect(notify.info).toHaveBeenCalledTimes(1);
  });
});
