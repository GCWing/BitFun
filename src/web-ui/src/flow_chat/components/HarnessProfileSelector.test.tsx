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

function density(scope: ParentNode): number {
  const mark = scope.querySelector<HTMLElement>(
    '.bitfun-harness-selector__density-mark',
  );
  return Number(mark?.dataset.harnessDensity ?? 0);
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

  it('reads the active gear from a progressively denser mark rather than from color alone', async () => {
    await act(async () => {
      root.render(<HarnessProfileSelector selectedProfile="balanced" onSelectProfile={vi.fn()} />);
    });

    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="harness-profile-selector"]',
    );
    expect(trigger?.dataset.harnessGear).toBe('2');
    expect(density(trigger!)).toBe(2);
    expect(
      trigger?.querySelector<HTMLElement>('.bitfun-harness-selector__density-mark')
        ?.dataset.harnessProfile,
    ).toBe('balanced');
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
    expect(density(trigger!)).toBe(1);
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
    expect(density(trigger!)).toBe(0);
    expect(
      trigger?.querySelector<HTMLElement>('.bitfun-harness-selector__density-mark')
        ?.dataset.harnessProfile,
    ).toBe('unknown');
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

  it('offers three density gears plus the Creative profile with its BitFun icon', async () => {
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
      'creative',
    ]);
    expect(rows.map(row => density(row))).toEqual([1, 2, 3, 0]);
    for (const row of rows.slice(0, 3)) {
      expect(row.querySelector('.bitfun-harness-selector__profile-promise')).not.toBeNull();
      expect(row.querySelector('.bitfun-harness-selector__density-core')).not.toBeNull();
    }
    const creative = rows[3];
    expect(creative?.querySelector('.bitfun-harness-selector__profile-promise')).not.toBeNull();
    expect(creative?.querySelector('.bitfun-harness-selector__density-core')).toBeNull();
    expect(creative?.querySelector('[data-bf-icon="harness-creative"]')).not.toBeNull();
    expect(creative?.dataset.bfState).toBe('coming-soon');
    expect(rows[1]?.dataset.bfState).toBe('current');
  });

  it('activates minimal and balanced while reporting unfinished profiles as unavailable', async () => {
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
      document.querySelector<HTMLButtonElement>('[data-testid="harness-profile-creative"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(notify.info).toHaveBeenCalledTimes(2);
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

  it('keeps the menu inspectable after the Session starts but explains why another profile cannot be selected', async () => {
    const onSelectProfile = vi.fn();
    await act(async () => {
      root.render(
        <HarnessProfileSelector
          sessionStarted
          selectedProfile="balanced"
          onSelectProfile={onSelectProfile}
        />,
      );
    });

    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="harness-profile-selector"]',
    );
    expect(trigger?.dataset.harnessLocked).toBe('true');
    expect(trigger?.disabled).toBe(false);

    await act(async () => {
      trigger?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    const menu = document.querySelector('.bitfun-harness-selector__menu');
    expect(menu).not.toBeNull();
    expect(
      menu?.querySelector<HTMLElement>('[data-testid="harness-profile-minimal"]')
        ?.dataset.bfState,
    ).toBe('new-session-only');
    expect(
      menu?.querySelector<HTMLElement>('[data-testid="harness-profile-minimal"]')
        ?.textContent,
    ).toContain('chatInput.harness.newSessionOnly');

    await act(async () => {
      document.querySelector<HTMLButtonElement>('[data-testid="harness-profile-minimal"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(onSelectProfile).not.toHaveBeenCalled();
    expect(notify.info).toHaveBeenCalledWith(
      'chatInput.harness.sessionStartedNotice',
      { duration: 3800 },
    );
    expect(document.querySelector('.bitfun-harness-selector__menu')).toBeNull();

    await act(async () => {
      trigger?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    await act(async () => {
      document.querySelector<HTMLButtonElement>('[data-testid="harness-profile-creative"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(notify.info).toHaveBeenLastCalledWith(
      'chatInput.harness.comingSoonNotice',
      { duration: 3200 },
    );
  });

  it.each(['ultimate', 'creative'] as const)(
    'does not present a persisted %s profile as active',
    async (profileId) => {
      await act(async () => {
        root.render(<HarnessProfileSelector selectedProfile={profileId} onSelectProfile={vi.fn()} />);
      });
      await act(async () => {
        container.querySelector<HTMLButtonElement>('[data-testid="harness-profile-selector"]')
          ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      });

      const profile = document.querySelector<HTMLButtonElement>(
        `[data-testid="harness-profile-${profileId}"]`,
      );
      expect(profile?.dataset.bfState).toBe('coming-soon');
      expect(profile?.getAttribute('aria-checked')).toBe('false');
    },
  );

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
