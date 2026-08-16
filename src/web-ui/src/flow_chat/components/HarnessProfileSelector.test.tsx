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
      root.render(<HarnessProfileSelector selectedProfile="balanced" onSelectProfile={vi.fn()} />);
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

  it('renders the authoritative selected profile without a pending projection', async () => {
    await act(async () => {
      root.render(<HarnessProfileSelector selectedProfile="minimal" onSelectProfile={vi.fn()} />);
    });

    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="harness-profile-selector"]',
    );
    expect(trigger?.dataset.harnessGear).toBe('1');
    expect(trigger?.dataset.harnessPending).toBeUndefined();
    expect(
      container.querySelector('[data-testid="harness-profile-pending-dot"]'),
    ).toBeNull();
  });

  it('preserves an unknown future profile without pretending it is balanced', async () => {
    await act(async () => {
      root.render(
        <HarnessProfileSelector selectedProfile="future-profile" onSelectProfile={vi.fn()} />,
      );
    });

    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="harness-profile-selector"]',
    );
    expect(trigger?.dataset.harnessGear).toBe('0');
    expect(filledBars(trigger!)).toBe(0);
    expect(trigger?.textContent).toContain('chatInput.harness.unsupportedProfile');
  });

  it('disables profile selection while the authoritative update is in flight', async () => {
    await act(async () => {
      root.render(
        <HarnessProfileSelector
          disabled
          selectedProfile="minimal"
          onSelectProfile={vi.fn()}
        />,
      );
    });

    expect(
      container.querySelector<HTMLButtonElement>('[data-testid="harness-profile-selector"]')
        ?.disabled,
    ).toBe(true);
  });

  it('offers the three gears with an ascending gauge and the promise each makes', async () => {
    await act(async () => {
      root.render(<HarnessProfileSelector selectedProfile="balanced" onSelectProfile={vi.fn()} />);
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

  it('activates minimal and balanced while reporting ultimate as unavailable', async () => {
    const onSelectProfile = vi.fn();
    await act(async () => {
      root.render(<HarnessProfileSelector selectedProfile="balanced" onSelectProfile={onSelectProfile} />);
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
    expect(onSelectProfile).not.toHaveBeenCalled();

    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="harness-profile-selector"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    await act(async () => {
      document.querySelector<HTMLButtonElement>('[data-testid="harness-profile-minimal"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(onSelectProfile).toHaveBeenCalledWith('minimal');
    expect(document.querySelector('.bitfun-harness-selector__menu')).toBeNull();
  });

  it('does not present a persisted ultimate profile as active', async () => {
    await act(async () => {
      root.render(<HarnessProfileSelector selectedProfile="ultimate" onSelectProfile={vi.fn()} />);
    });
    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="harness-profile-selector"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    const ultimate = document.querySelector<HTMLButtonElement>(
      '[data-testid="harness-profile-ultimate"]',
    );
    expect(ultimate?.dataset.bfState).toBe('coming-soon');
    expect(ultimate?.getAttribute('aria-checked')).toBe('false');
  });

  it('keeps a legacy session on its own mode and explains why instead of switching', async () => {
    const onSelectProfile = vi.fn();
    await act(async () => {
      root.render(
        <HarnessProfileSelector legacySession selectedProfile="balanced" onSelectProfile={onSelectProfile} />,
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
    expect(onSelectProfile).not.toHaveBeenCalled();
    expect(notify.info).toHaveBeenCalledTimes(1);
  });
});
