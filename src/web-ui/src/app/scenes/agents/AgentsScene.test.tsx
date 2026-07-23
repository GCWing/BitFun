import React, { act } from 'react';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { useAgentsStore } from './agentsStore';
import { isLocallyManageableSubagent } from './agentVisibility';

const useAgentsListMock = vi.hoisted(() => vi.fn());

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: vi.fn(),
  },
  useTranslation: () => ({
    t: (_key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? _key,
  }),
}));

vi.mock('./components/CreateAgentPage', () => ({
  default: () => <div data-testid="create-agent-page">create agent</div>,
}));

vi.mock('./components/AgentCard', () => ({
  default: ({
    agent,
    onOpenDetails,
  }: {
    agent: { name: string };
    onOpenDetails: (agent: unknown) => void;
  }) => (
    <button type="button" onClick={() => onOpenDetails(agent)}>{agent.name}</button>
  ),
}));

vi.mock('./components/CoreAgentCard', () => ({
  default: () => <div />,
}));

vi.mock('./components/useUserToolGroups', () => ({
  useUserToolGroups: () => ({
    groups: [],
    loading: false,
    saveGroups: vi.fn(),
  }),
}));

vi.mock('./components/useUserSkillGroups', () => ({
  useUserSkillGroups: () => ({
    groups: [],
    loading: false,
    saveGroups: vi.fn(),
  }),
}));

vi.mock('./components/SkillGroupPicker', () => ({
  SkillGroupPicker: () => <div data-testid="agent-detail-skill-groups">skill picker</div>,
  SkillGroupSummary: () => <div data-testid="agent-detail-skill-summary">skill summary</div>,
}));

vi.mock('@/component-library', () => ({
  Badge: ({ children }: { children: React.ReactNode }) => <span>{children}</span>,
  Button: ({ children, onClick }: { children: React.ReactNode; onClick?: () => void }) => (
    <button type="button" onClick={onClick}>{children}</button>
  ),
  IconButton: ({ children, onClick }: { children: React.ReactNode; onClick?: () => void }) => (
    <button type="button" onClick={onClick}>{children}</button>
  ),
  Search: () => <input readOnly />,
  Select: () => <div />,
  Switch: () => <input type="checkbox" readOnly />,
  confirmDanger: vi.fn(async () => false),
}));

vi.mock('@/app/components', () => ({
  GalleryDetailModal: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  GalleryEmpty: () => <div />,
  GalleryGrid: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  GalleryLayout: ({ children, className }: { children: React.ReactNode; className?: string }) => (
    <main className={className}>{children}</main>
  ),
  GalleryPageHeader: () => <header />,
  GallerySkeleton: () => <div />,
  GalleryZone: ({ children }: { children: React.ReactNode }) => <section>{children}</section>,
}));

vi.mock('./hooks/useAgentsList', () => ({
  useAgentsList: () => useAgentsListMock(),
}));

function mockAgentsList(overrides: Record<string, unknown> = {}) {
  useAgentsListMock.mockReturnValue({
    allAgents: [],
    filteredAgents: [],
    loading: false,
    availableTools: [],
    getModeProfile: () => null,
    getAgentSkills: () => [],
    getModeManageableSubagents: () => [],
    counts: { builtin: 0, user: 0, project: 0, mode: 0, subagent: 0 },
    loadAgents: vi.fn(),
    getModeConfig: () => undefined,
    handleSetTools: vi.fn(),
    handleResetTools: vi.fn(),
    handleSetSkills: vi.fn(),
    handleResetSkills: vi.fn(),
    handleSetSubagentEnabled: vi.fn(),
    handleSetSubagentModel: vi.fn(),
    ...overrides,
  });
}

vi.mock('@/app/hooks/useGallerySceneAutoRefresh', () => ({
  useGallerySceneAutoRefresh: vi.fn(),
}));

vi.mock('@/infrastructure/contexts/WorkspaceContext', () => ({
  useCurrentWorkspace: () => ({ workspacePath: 'D:/workspace/project' }),
}));

vi.mock('@/infrastructure/config/services/ConfigManager', () => ({
  configManager: {
    getConfig: vi.fn(async () => false),
    onConfigChange: vi.fn(() => () => {}),
  },
}));

vi.mock('@/shared/notification-system', () => ({
  useNotification: () => ({
    success: vi.fn(),
    error: vi.fn(),
    warning: vi.fn(),
    info: vi.fn(),
  }),
}));

vi.mock('@/infrastructure/api/service-api/SubagentAPI', () => ({
  SubagentAPI: {
    deleteSubagent: vi.fn(),
  },
}));

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

describe('agent editability', () => {
  it('keeps external subagents visible but outside local mutations', () => {
    expect(isLocallyManageableSubagent({ source: 'external' })).toBe(false);
    expect(isLocallyManageableSubagent({ subagentSource: 'external', source: 'user' })).toBe(false);
    expect(isLocallyManageableSubagent({ source: 'builtin' })).toBe(true);
  });
});

describeWithJsdom('AgentsScene', () => {
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

    useAgentsStore.getState().openHome();
    mockAgentsList();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    dom.window.close();
    vi.unstubAllGlobals();
    useAgentsStore.getState().openHome();
  });

  it('keeps agent creation inside a full-height scene page wrapper', async () => {
    useAgentsStore.getState().openCreateAgent();
    const { default: AgentsScene } = await import('./AgentsScene');

    await act(async () => {
      root.render(<AgentsScene />);
    });

    expect(container.querySelector('[data-testid="create-agent-page"]')).toBeTruthy();
    expect(container.querySelector('.bitfun-agents-scene--page')).toBeTruthy();
  }, 10_000);

  it('keeps agent subpages stretched across the active scene viewport', () => {
    const stylesheet = readFileSync(
      fileURLToPath(new URL('./AgentsScene.scss', import.meta.url)),
      'utf8',
    );

    expect(stylesheet).toContain('width: 100%;');
    expect(stylesheet).toContain('flex: 1 1 auto;');
    expect(stylesheet).toContain('min-width: 0;');
  });

  it('shows skill grouping and editing for a custom subagent with the Skill tool', async () => {
    const subagent = {
      key: 'user::skill-worker',
      id: 'skill-worker',
      name: 'Skill worker',
      description: 'Uses specialized workflows.',
      isReadonly: false,
      isReview: false,
      toolCount: 1,
      defaultTools: ['Skill'],
      defaultEnabled: true,
      effectiveEnabled: true,
      source: 'user',
      agentKind: 'subagent' as const,
      capabilities: [],
    };
    mockAgentsList({
      allAgents: [subagent],
      filteredAgents: [subagent],
      getAgentSkills: (agentId: string) => agentId === subagent.id
        ? [{ key: 'user::custom::workflow', effectiveEnabled: true }]
        : [],
    });
    const { default: AgentsScene } = await import('./AgentsScene');

    await act(async () => {
      root.render(<AgentsScene />);
    });
    await act(async () => {
      Array.from(container.querySelectorAll<HTMLButtonElement>('button'))
        .find((button) => button.textContent === subagent.name)
        ?.click();
    });

    const skillsTab = Array.from(container.querySelectorAll<HTMLButtonElement>('[role="tab"]'))
      .find((tab) => tab.textContent?.includes('agentsOverview.skills'));
    expect(skillsTab).toBeTruthy();

    await act(async () => {
      skillsTab?.click();
    });
    expect(container.querySelector('[data-testid="agent-detail-skill-summary"]')).toBeTruthy();

    const manageButton = Array.from(container.querySelectorAll<HTMLButtonElement>('button'))
      .find((button) => button.textContent === 'manage');
    expect(manageButton).toBeTruthy();
    await act(async () => {
      manageButton?.click();
    });
    expect(container.querySelector('[data-testid="agent-detail-skill-groups"]')).toBeTruthy();
  });
});
