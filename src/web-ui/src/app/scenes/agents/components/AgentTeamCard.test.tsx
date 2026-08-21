import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import AgentTeamCard from './AgentTeamCard';
import type { AgentTeam } from '../agentsStore';

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: vi.fn(),
  },
  useTranslation: () => ({
    t: (key: string, opts?: { defaultValue?: string; count?: number }) => {
      if (key === 'home.members') return `${opts?.count ?? 0} members`;
      if (key === 'composer.strategy.collaborative') return 'Collaborative';
      if (key === 'teamCard.badges.example') return 'Example';
      if (key === 'teamCard.badges.sharedContext') return 'Shared context';
      if (key === 'agentsOverview.editAgent') return 'Edit';
      return opts?.defaultValue ?? key;
    },
  }),
}));

const team: AgentTeam = {
  id: 'agent-team-coding',
  name: 'Coding Team',
  icon: 'code',
  description: 'Code review and quality',
  members: [
    { agentId: 'agentic', role: 'leader', order: 0 },
    { agentId: 'CodeReview', role: 'member', order: 1 },
  ],
  strategy: 'collaborative',
  shareContext: true,
};

const allAgents = [
  { id: 'agentic', name: 'Agentic', iconKey: 'cpu', capabilities: [{ category: 'coding', level: 5 }] },
  { id: 'CodeReview', name: 'CodeReview', iconKey: 'eye', capabilities: [{ category: 'coding', level: 4 }] },
] as unknown as Parameters<typeof AgentTeamCard>[0]['allAgents'];

let JSDOMCtor: (new (
  html?: string,
  options?: { pretendToBeVisual?: boolean }
) => { window: Window & typeof globalThis }) | null = null;

try {
  const jsdom = await import('jsdom');
  JSDOMCtor = jsdom.JSDOM as typeof JSDOMCtor;
} catch {
  JSDOMCtor = null;
}

const describeWithJsdom = JSDOMCtor ? describe : describe.skip;

describeWithJsdom('AgentTeamCard', () => {
  let dom: { window: Window & typeof globalThis };
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    dom = new JSDOMCtor!('<!doctype html><html><body></body></html>', {
      pretendToBeVisual: true,
      url: 'http://localhost',
    });
    const { window } = dom;
    vi.stubGlobal('window', window);
    vi.stubGlobal('document', window.document);
    vi.stubGlobal('navigator', window.navigator);
    vi.stubGlobal('HTMLElement', window.HTMLElement);
    vi.stubGlobal('MutationObserver', window.MutationObserver);
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
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container, {});
  });

  afterEach(() => {
    root?.unmount();
    container?.remove();
    dom.window.close();
    vi.unstubAllGlobals();
  });

  function mount(props?: Partial<Parameters<typeof AgentTeamCard>[0]>) {
    const onEdit = vi.fn();
    const onOpenDetails = vi.fn();
    act(() => {
      root.render(
        <AgentTeamCard
          team={team}
          allAgents={allAgents}
          onEdit={onEdit}
          onOpenDetails={onOpenDetails}
          topCapabilities={['coding', 'testing']}
          {...props}
        />,
      );
    });
    return { onEdit, onOpenDetails };
  }

  it('renders team name, member count and opens details on click', () => {
    const { onOpenDetails } = mount();

    expect(container.textContent).toContain('Coding Team');
    expect(container.textContent).toContain('2 members');

    const card = container.querySelector('.agent-team-card') as HTMLElement;
    act(() => {
      card.click();
    });
    expect(onOpenDetails).toHaveBeenCalledWith(team);
  });

  it('triggers edit callback from the edit button', () => {
    const { onEdit } = mount();
    const editBtn = container.querySelector('.agent-team-card__icon-btn') as HTMLButtonElement;
    act(() => {
      editBtn.click();
    });
    expect(onEdit).toHaveBeenCalledWith(team.id);
  });
});
