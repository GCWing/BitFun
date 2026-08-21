import React, { act } from 'react';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';

const replaceModeToolSelectionMock = vi.hoisted(() => vi.fn(async () => 'ok'));
const resetModeToolSelectionMock = vi.hoisted(() => vi.fn(async () => 'ok'));

vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: vi.fn() },
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? key,
  }),
}));

vi.mock('@/component-library', () => ({
  Badge: ({ children }: { children: React.ReactNode }) => <span>{children}</span>,
  Button: ({ children, onClick, disabled }: {
    children: React.ReactNode;
    onClick?: () => void;
    disabled?: boolean;
  }) => (
    <button type="button" onClick={onClick} disabled={disabled}>{children}</button>
  ),
}));

vi.mock('@/component-library/components/ConfirmDialog/confirmService', () => ({
  confirmDialog: vi.fn(async () => true),
}));

vi.mock('@/infrastructure/api', () => ({
  configAPI: {
    replaceModeToolSelection: replaceModeToolSelectionMock,
    resetModeToolSelection: resetModeToolSelectionMock,
  },
}));

vi.mock('@/infrastructure/hooks/useWorkspaceManagerSync', () => ({
  useWorkspaceManagerSync: () => ({ workspacePath: 'D:/workspace/project' }),
}));

vi.mock('@/app/hooks/useGallerySceneAutoRefresh', () => ({
  useGallerySceneAutoRefresh: vi.fn(),
}));

vi.mock('@/shared/notification-system', () => ({
  useNotification: () => ({
    success: vi.fn(),
    error: vi.fn(),
    warning: vi.fn(),
    info: vi.fn(),
  }),
}));

vi.mock('@/infrastructure/event-bus', () => ({
  globalEventBus: { emit: vi.fn() },
}));

vi.mock('./ToolGroupPicker', () => ({
  GroupManagerModal: () => <div data-testid="tool-group-manager">manager</div>,
  ToolGroupPicker: () => <div />,
  ToolGroupSummary: () => <div />,
}));

vi.mock('./useUserToolGroups', () => ({
  useUserToolGroups: () => ({ groups: [], loading: false, saveGroups: vi.fn() }),
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

describeWithJsdom('ToolSuiteView', () => {
  let dom: { window: Window & typeof globalThis };
  let container: HTMLDivElement;
  let root: Root;
  let ToolSuiteView: React.ComponentType<{
    tools: Array<{ name: string; description: string; is_readonly: boolean }>;
    getModeConfig: (modeId: string) => { enabled_tools: string[]; default_tools: string[] } | null;
    userGroups: never[];
    onSaveUserGroups: (groups: never[]) => Promise<void>;
  }>;

  const tools = [
    { name: 'Read', description: 'Read files', is_readonly: true },
    { name: 'Write', description: 'Write files', is_readonly: false },
    { name: 'mcp__github__search_repos', description: 'Search GitHub repos', is_readonly: true },
  ];

  beforeEach(async () => {
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
    root = createRoot(container);
    const mod = await import('./ToolSuiteView');
    ToolSuiteView = mod.default;
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    dom.window.close();
    vi.unstubAllGlobals();
    replaceModeToolSelectionMock.mockReset();
    resetModeToolSelectionMock.mockReset();
  });

  it('renders the four mode tabs and tool groups (TB-4: mode toolbar)', async () => {
    await act(async () => {
      root.render(
        <ToolSuiteView
          tools={tools as never}
          getModeConfig={() => ({ enabled_tools: ['Read'], default_tools: ['Read', 'Write'] })}
          userGroups={[]}
          onSaveUserGroups={async () => {}}
        />,
      );
    });

    const tabs = Array.from(container.querySelectorAll('[role="tab"]')) as HTMLElement[];
    expect(tabs.length).toBeGreaterThanOrEqual(4);
    expect(tabs.map((tab) => tab.getAttribute('aria-selected'))).toContain('true');
  });

  it('saves a disabled tool via replaceModeToolSelection (TB-4: toggle off blocks)', async () => {
    await act(async () => {
      root.render(
        <ToolSuiteView
          tools={tools as never}
          getModeConfig={() => ({ enabled_tools: ['Read', 'Write'], default_tools: ['Read', 'Write'] })}
          userGroups={[]}
          onSaveUserGroups={async () => {}}
        />,
      );
    });

    // Click the "Read" tool chip to toggle it off (draft), then click Save.
    const readChip = Array.from(container.querySelectorAll<HTMLButtonElement>('button'))
      .find((button) => button.textContent?.includes('Read'));
    expect(readChip).toBeTruthy();

    await act(async () => {
      readChip?.click();
    });

    const saveButton = Array.from(container.querySelectorAll<HTMLButtonElement>('button'))
      .find((button) => button.textContent === 'suite.groupActions.save');
    expect(saveButton).toBeTruthy();

    await act(async () => {
      saveButton?.click();
      // Flush the full async save chain: replaceModeToolSelection await +
      // setState batches + event-bus import (mocked). Multiple microtask
      // flushes keep React act warnings and unmount-time setState away.
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(replaceModeToolSelectionMock).toHaveBeenCalledTimes(1);
    const payload = replaceModeToolSelectionMock.mock.calls[0][0];
    // Read was toggled off: the persisted enabled set must not include Read.
    expect(payload.enabledToolNames).not.toContain('Read');
    expect(payload.enabledToolNames).toContain('Write');
  });

  it('keeps the tools scene stylesheet contract (flex full-height)', () => {
    const stylesheet = readFileSync(
      fileURLToPath(new URL('../../tools/ToolsScene.scss', import.meta.url)),
      'utf8',
    );
    expect(stylesheet).toContain('width: 100%;');
    expect(stylesheet).toContain('height: 100%;');
  });
});
