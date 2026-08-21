// @vitest-environment jsdom
import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import AgentTeamTabBar from './AgentTeamTabBar';
import { useAgentsStore, MOCK_AGENT_TEAMS } from '../agentsStore';

const mocks = vi.hoisted(() => ({
  confirmDanger: vi.fn(async () => true),
}));

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: vi.fn(),
  },
  useTranslation: () => ({
    t: (key: string, opts?: { defaultValue?: string; count?: number; from?: string }) =>
      opts?.defaultValue ?? key,
  }),
}));

vi.mock('@/component-library', () => ({
  confirmDanger: mocks.confirmDanger,
}));

vi.mock('../agentsIcons', () => ({
  AGENT_ICON_MAP: { bot: () => <span /> },
  AGENT_TEAM_ICON_MAP: {
    code: () => <span />,
    chart: () => <span />,
    layout: () => <span />,
    rocket: () => <span />,
    users: () => <span />,
    briefcase: () => <span />,
    layers: () => <span />,
  },
  getAgentTeamAccent: () => 'var(--t-accent)',
}));

vi.mock('./AgentTeamTabBar.scss', () => ({}));

const describeWithJsdom = describe;

describeWithJsdom('AgentTeamTabBar (P0-1 delete confirm + P1-5 unified panel)', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    if (typeof window.matchMedia !== 'function') {
      Object.defineProperty(window, 'matchMedia', {
        writable: true,
        value: vi.fn().mockImplementation(() => ({
          matches: false,
          addEventListener: vi.fn(),
          removeEventListener: vi.fn(),
          addListener: vi.fn(),
          removeListener: vi.fn(),
        })),
      });
    }
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
    mocks.confirmDanger.mockClear();
    mocks.confirmDanger.mockResolvedValue(true);
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    // Restore the default active team so each test renders the mock seed.
    useAgentsStore.getState().setActiveAgentTeam(MOCK_AGENT_TEAMS[0].id);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.unstubAllGlobals();
  });

  const renderBar = async () => {
    await act(async () => {
      root.render(<AgentTeamTabBar />);
    });
  };

  it('renders a tab for every team plus the new-team button', async () => {
    await renderBar();
    const tabs = container.querySelectorAll('.bt-tabbar__tab');
    expect(tabs.length).toBe(useAgentsStore.getState().agentTeams.length);
    expect(container.querySelector('.bt-tabbar__new')).toBeTruthy();
  });

  it('P0-1: confirms before deleting a team and deletes only after confirm', async () => {
    await renderBar();
    const close = container.querySelector('.bt-tabbar__tab-close') as HTMLElement;
    expect(close).toBeTruthy();
    await act(async () => {
      close.click();
    });
    expect(mocks.confirmDanger).toHaveBeenCalledTimes(1);
    expect(useAgentsStore.getState().agentTeams.length).toBeLessThan(MOCK_AGENT_TEAMS.length);
  });

  it('P0-1: does not delete when confirmDanger is rejected', async () => {
    mocks.confirmDanger.mockResolvedValueOnce(false);
    await renderBar();
    const before = useAgentsStore.getState().agentTeams.length;
    const close = container.querySelector('.bt-tabbar__tab-close') as HTMLElement;
    await act(async () => {
      close.click();
    });
    expect(mocks.confirmDanger).toHaveBeenCalledTimes(1);
    expect(useAgentsStore.getState().agentTeams.length).toBe(before);
  });

  it('P1-5: renders a single panel with blank/template tabs and no back-and-forth button', async () => {
    await renderBar();
    const newBtn = container.querySelector('.bt-tabbar__new') as HTMLElement;
    await act(async () => {
      newBtn.click();
    });
    const panel = container.querySelector('.bt-tabbar__panel');
    expect(panel).toBeTruthy();
    expect(panel!.querySelectorAll('.bt-tabbar__panel-tab').length).toBe(2);
    expect(panel!.querySelector('.bt-tabbar__action--primary')).toBeTruthy();
    expect(container.querySelector('.bt-tabbar__tpl-grid')).toBeFalsy();
    expect(panel!.textContent).not.toContain('tabbar.fromTemplate');
    expect(panel!.textContent).not.toContain('←');
  });

  it('P1-5: switching to the template tab shows the template grid inside the same panel', async () => {
    await renderBar();
    const newBtn = container.querySelector('.bt-tabbar__new') as HTMLElement;
    await act(async () => {
      newBtn.click();
    });
    const tabs = Array.from(container.querySelectorAll<HTMLElement>('.bt-tabbar__panel-tab'));
    await act(async () => {
      tabs[1]!.click();
    });
    expect(container.querySelector('.bt-tabbar__tpl-grid')).toBeTruthy();
    expect(container.querySelector('.bt-tabbar__tpl-card')).toBeTruthy();
    // no cross-panel back button
    expect(container.querySelector('.bt-tabbar__tpl-back')).toBeFalsy();
  });

  it('P1-5: selecting a template creates a team and closes the panel', async () => {
    await renderBar();
    const before = useAgentsStore.getState().agentTeams.length;
    const newBtn = container.querySelector('.bt-tabbar__new') as HTMLElement;
    await act(async () => {
      newBtn.click();
    });
    const tabs = Array.from(container.querySelectorAll<HTMLElement>('.bt-tabbar__panel-tab'));
    await act(async () => {
      tabs[1]!.click();
    });
    const card = container.querySelector('.bt-tabbar__tpl-card') as HTMLElement;
    await act(async () => {
      card.click();
    });
    expect(useAgentsStore.getState().agentTeams.length).toBe(before + 1);
    expect(container.querySelector('.bt-tabbar__panel')).toBeFalsy();
  });
});
