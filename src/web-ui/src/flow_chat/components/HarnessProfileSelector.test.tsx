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

  it('keeps the active Harness icon out of the ChatInput trigger', async () => {
    await act(async () => {
      root.render(
        <HarnessProfileSelector
          selectedProfile="balanced"
          onSelectProfile={vi.fn()}
        />,
      );
    });

    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="harness-profile-selector"]',
    );
    expect(trigger?.querySelector('.bitfun-harness-selector__density-mark')).toBeNull();
    expect(trigger?.textContent).toBe('chatInput.harness.profiles.balanced.name');
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
    expect(trigger?.querySelector('.bitfun-harness-selector__density-mark')).toBeNull();
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
    expect(trigger?.querySelector('.bitfun-harness-selector__density-mark')).toBeNull();
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

  it('nests the menu-item picker inside its parent menu and closes it after an Agent choice', async () => {
    const onSelectAgent = vi.fn();
    const onSelectionComplete = vi.fn();
    await act(async () => {
      root.render(
        <div data-testid="parent-add-menu">
          <HarnessProfileSelector
            presentation="menu-item"
            selectedProfile="balanced"
            otherAgents={[{ id: 'DeepResearch', name: 'Deep Research' }]}
            onSelectProfile={vi.fn()}
            onSelectAgent={onSelectAgent}
            onSelectionComplete={onSelectionComplete}
          />
        </div>,
      );
    });

    const selectorRoot = container.querySelector<HTMLElement>(
      '[data-bf-component="harness-selector"][data-bf-part="root"]',
    );
    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="harness-profile-selector"]',
    );
    expect(selectorRoot?.dataset.bfPresentation).toBe('menu-item');
    expect(trigger?.querySelector('.bitfun-harness-selector__trigger-chevron')).not.toBeNull();
    expect(
      trigger?.querySelector('.bitfun-harness-selector__trigger-label')?.textContent,
    ).toBe('chatInput.harness.menuLabel');
    expect(
      trigger?.querySelector('.bitfun-harness-selector__trigger-current')?.textContent,
    ).toContain('chatInput.current');
    expect(
      trigger?.querySelector('.bitfun-harness-selector__trigger-current-value')?.textContent,
    ).toBe('chatInput.harness.profiles.balanced.name');

    await act(async () => {
      trigger?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    const menu = selectorRoot?.querySelector<HTMLElement>('.bitfun-harness-selector__menu');
    expect(menu).not.toBeNull();
    expect(menu?.dataset.bfPlacement).toBe('side');
    expect(container.querySelector('[data-testid="parent-add-menu"]')?.contains(menu ?? null))
      .toBe(true);

    await act(async () => {
      menu?.querySelector<HTMLButtonElement>('[data-testid="harness-profile-other"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(menu?.dataset.bfPage).toBe('agents');
    expect(onSelectionComplete).not.toHaveBeenCalled();

    await act(async () => {
      menu?.querySelector<HTMLButtonElement>('[data-testid="harness-agent-DeepResearch"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(onSelectAgent).toHaveBeenCalledWith('DeepResearch');
    expect(onSelectionComplete).toHaveBeenCalledTimes(1);
    expect(selectorRoot?.querySelector('.bitfun-harness-selector__menu')).toBeNull();
  });

  it('offers three Harness gears, Creative, and the second-level Agents entry', async () => {
    await act(async () => {
      root.render(
        <HarnessProfileSelector
          selectedProfile="balanced"
          otherAgents={[
            { id: 'DeepResearch', name: 'Deep Research' },
            { id: 'Cowork', name: 'Cowork' },
            { id: 'Plan', name: 'Plan' },
          ]}
          onSelectProfile={vi.fn()}
        />,
      );
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
      'other',
    ]);
    expect(rows.map(row => density(row))).toEqual([1, 2, 3, 0, 0]);
    for (const row of rows.slice(0, 3)) {
      expect(row.querySelector('.bitfun-harness-selector__density-core')).not.toBeNull();
    }
    expect(menu?.querySelector('.bitfun-harness-selector__profile-promise')).toBeNull();
    const creative = rows[3];
    expect(creative?.querySelector('.bitfun-harness-selector__density-core')).toBeNull();
    expect(creative?.querySelector('[data-bf-icon="harness-creative"]')).not.toBeNull();
    expect(creative?.dataset.bfState).toBe('available');
    const other = rows[4];
    expect(other?.querySelector('.bitfun-harness-selector__density-core')).toBeNull();
    expect(other?.querySelector('.lucide-bot')).not.toBeNull();
    expect(other?.querySelector('.bitfun-harness-selector__agent-count')?.textContent).toBe('3');
    expect(rows[1]?.dataset.bfState).toBe('current');
  });

  it('opens Agents as a second level and selects a main Agent without a separate chip', async () => {
    const onSelectAgent = vi.fn();
    await act(async () => {
      root.render(
        <HarnessProfileSelector
          selectedProfile="balanced"
          otherAgents={[
            { id: 'DeepResearch', name: 'Deep Research' },
            { id: 'Cowork', name: 'Cowork' },
            { id: 'Plan', name: 'Plan' },
          ]}
          onSelectProfile={vi.fn()}
          onSelectAgent={onSelectAgent}
        />,
      );
    });
    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="harness-profile-selector"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    await act(async () => {
      document.querySelector<HTMLButtonElement>('[data-testid="harness-profile-other"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    const menu = document.querySelector<HTMLElement>('.bitfun-harness-selector__menu');
    expect(menu?.dataset.bfPage).toBe('agents');
    expect(Array.from(menu!.querySelectorAll<HTMLElement>('[data-bf-part="agent"]')).map(
      row => row.dataset.bfAgentId,
    )).toEqual(['DeepResearch', 'Cowork', 'Plan']);

    await act(async () => {
      document.querySelector<HTMLButtonElement>('[data-testid="harness-agent-DeepResearch"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(onSelectAgent).toHaveBeenCalledWith('DeepResearch');
    expect(document.querySelector('.bitfun-harness-selector__menu')).toBeNull();

    await act(async () => {
      root.render(
        <HarnessProfileSelector
          selectedProfile="other"
          selectedAgentId="DeepResearch"
          otherAgents={[{ id: 'DeepResearch', name: 'Deep Research' }]}
          onSelectProfile={vi.fn()}
          onSelectAgent={onSelectAgent}
        />,
      );
    });
    expect(
      container.querySelector('[data-testid="harness-profile-selector"]')?.textContent,
    ).toBe('Deep Research');
  });

  it('activates every implemented profile including Creative', async () => {
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
    expect(onSelectProfile).toHaveBeenCalledWith('ultimate');
    expect(onSelectProfile).toHaveBeenCalledTimes(1);

    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="harness-profile-selector"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    await act(async () => {
      document.querySelector<HTMLButtonElement>('[data-testid="harness-profile-creative"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(notify.info).not.toHaveBeenCalled();
    expect(onSelectProfile).toHaveBeenCalledTimes(2);
    expect(onSelectProfile).toHaveBeenLastCalledWith('creative');

    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="harness-profile-selector"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    await act(async () => {
      document.querySelector<HTMLButtonElement>('[data-testid="harness-profile-minimal"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(onSelectProfile).toHaveBeenCalledTimes(3);
    expect(onSelectProfile).toHaveBeenLastCalledWith('minimal');
    expect(onSelectProfile).toHaveBeenCalledWith('minimal');
    expect(document.querySelector('.bitfun-harness-selector__menu')).toBeNull();
  });

  it('collapses a started Session into its signature before exposing new-Session choices', async () => {
    const onSelectProfile = vi.fn();
    const onStartNewSession = vi.fn();
    await act(async () => {
      root.render(
        <HarnessProfileSelector
          sessionStarted
          selectedProfile="balanced"
          otherAgents={[{ id: 'Plan', name: 'Plan' }]}
          onSelectProfile={onSelectProfile}
          onStartNewSession={onStartNewSession}
        />,
      );
    });

    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="harness-profile-selector"]',
    );
    expect(trigger?.dataset.harnessLocked).toBe('true');
    expect(trigger?.dataset.harnessFixed).toBe('true');
    expect(trigger?.disabled).toBe(false);
    expect(trigger?.textContent).toBe('chatInput.harness.profiles.balanced.name');

    await act(async () => {
      trigger?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    const menu = document.querySelector<HTMLElement>('.bitfun-harness-selector__menu');
    expect(menu).not.toBeNull();
    expect(menu?.dataset.bfPage).toBe('summary');
    expect(menu?.querySelector('[data-bf-part="profile"]')).toBeNull();
    expect(
      menu?.querySelector('[data-testid="harness-session-summary"]')?.textContent,
    ).toContain('chatInput.harness.profiles.balanced.name');
    expect(menu?.querySelector('.bitfun-harness-selector__session-scope')).toBeNull();
    const startNewSession = menu?.querySelector<HTMLButtonElement>(
      '[data-testid="harness-start-new-session"]',
    );
    expect(startNewSession?.querySelector('.lucide-message-square-plus')).toBeNull();
    expect(startNewSession?.querySelectorAll('svg')).toHaveLength(1);

    await act(async () => {
      startNewSession?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(menu?.dataset.bfPage).toBe('profiles');
    expect(
      menu?.querySelector<HTMLButtonElement>('[data-testid="harness-profile-minimal"]')
        ?.getAttribute('role'),
    ).toBe('menuitem');
    expect(
      menu?.querySelector<HTMLElement>('[data-testid="harness-profile-minimal"]')
        ?.dataset.bfState,
    ).toBe('available');
    expect(menu?.textContent).not.toContain('chatInput.harness.newSessionOnly');

    await act(async () => {
      menu?.querySelector<HTMLButtonElement>('[data-testid="harness-profile-minimal"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(onSelectProfile).not.toHaveBeenCalled();
    expect(onStartNewSession).toHaveBeenCalledWith(
      { kind: 'profile', id: 'minimal' },
    );
    expect(document.querySelector('.bitfun-harness-selector__menu')).toBeNull();
  });

  it('creates a new Session from a different main Agent without mutating the current Agent', async () => {
    const onSelectAgent = vi.fn();
    const onStartNewSession = vi.fn();
    await act(async () => {
      root.render(
        <HarnessProfileSelector
          sessionStarted
          selectedProfile="other"
          selectedAgentId="Plan"
          otherAgents={[
            { id: 'Plan', name: 'Plan' },
            { id: 'Cowork', name: 'Cowork' },
          ]}
          onSelectProfile={vi.fn()}
          onSelectAgent={onSelectAgent}
          onStartNewSession={onStartNewSession}
        />,
      );
    });
    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="harness-profile-selector"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(
      document.querySelector('[data-testid="harness-session-summary"]')?.textContent,
    ).toContain('Plan');
    await act(async () => {
      document.querySelector<HTMLButtonElement>('[data-testid="harness-start-new-session"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    await act(async () => {
      document.querySelector<HTMLButtonElement>('[data-testid="harness-profile-other"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(
      document.querySelector<HTMLElement>('[data-testid="harness-agent-Plan"]')?.dataset.bfState,
    ).toBe('available');
    expect(
      document.querySelector<HTMLElement>('[data-testid="harness-agent-Cowork"]')?.dataset.bfState,
    ).toBe('available');

    await act(async () => {
      document.querySelector<HTMLButtonElement>('[data-testid="harness-agent-Cowork"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(onSelectAgent).not.toHaveBeenCalled();
    expect(onStartNewSession).toHaveBeenCalledWith(
      { kind: 'agent', id: 'Cowork' },
    );
  });

  it.each(['creative'] as const)(
    'presents a persisted %s profile as active',
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
      expect(profile?.dataset.bfState).toBe('current');
      expect(profile?.getAttribute('aria-checked')).toBe('true');
    },
  );

  it('keeps a legacy Session fixed while offering the same new-Session path', async () => {
    const onSelectProfile = vi.fn();
    const onStartNewSession = vi.fn();
    await act(async () => {
      root.render(
        <HarnessProfileSelector
          legacySession
          selectedProfile="balanced"
          onSelectProfile={onSelectProfile}
          onStartNewSession={onStartNewSession}
        />,
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
      document.querySelector<HTMLButtonElement>('[data-testid="harness-start-new-session"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    await act(async () => {
      document.querySelector<HTMLButtonElement>('[data-testid="harness-profile-balanced"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(onSelectProfile).not.toHaveBeenCalled();
    expect(onStartNewSession).toHaveBeenCalledWith(
      { kind: 'profile', id: 'balanced' },
    );
    expect(notify.info).not.toHaveBeenCalled();
  });
});
