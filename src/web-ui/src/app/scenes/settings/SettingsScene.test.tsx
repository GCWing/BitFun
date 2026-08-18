// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import SettingsScene from './SettingsScene';
import { useSettingsStore } from './settingsStore';

vi.mock('../../../infrastructure/config/components/AIModelConfig', () => ({
  default: () => <div data-testid="models-config" />,
}));

vi.mock('../../../infrastructure/config/components/McpToolsConfig', () => ({
  default: () => <div data-testid="mcp-tools-config" />,
}));

vi.mock('../../../infrastructure/config/components/AcpAgentsConfig', () => ({
  default: () => <div data-testid="acp-agents-config" />,
}));

vi.mock('../../../infrastructure/config/components/ExternalSourcesConfig', () => ({
  default: ({
    initialFocus,
    focusRequestId,
  }: {
    initialFocus?: 'hooks';
    focusRequestId?: number;
  }) => (
    <div
      data-testid="external-sources-config"
      data-initial-focus={initialFocus}
      data-focus-request-id={focusRequestId}
    />
  ),
}));

vi.mock('../../../infrastructure/config/components/EditorConfig', () => ({
  default: () => <div data-testid="editor-config" />,
}));

vi.mock('../../../infrastructure/config/components/BasicsConfig', () => ({
  default: () => <div data-testid="basics-config" />,
}));

vi.mock('../../../infrastructure/config/components/AppearanceConfig', () => ({
  default: () => <div data-testid="appearance-config" />,
}));

vi.mock('../../../infrastructure/config/components/ReviewConfig', () => ({
  default: () => <div data-testid="review-config" />,
}));

vi.mock('../../../infrastructure/config/components/QuickActionsConfig', () => ({
  default: () => <div data-testid="quick-actions-config" />,
}));

vi.mock('../../../infrastructure/config/components/VoiceInputConfig', () => ({
  default: () => <div data-testid="voice-input-config" />,
}));

vi.mock('../../../infrastructure/config/components/SessionConfig', () => ({
  SessionPersonalizationConfig: () => <div data-testid="session-personalization-config" />,
  SessionPermissionsConfig: () => <div data-testid="session-permissions-config" />,
}));

vi.mock('./components/ArchivedSessionsConfig', () => ({
  default: () => <div data-testid="archived-sessions-config" />,
}));

vi.mock('./components/KeyboardShortcutsTab', () => ({
  default: () => <div data-testid="keyboard-shortcuts-config" />,
}));

describe('SettingsScene lazy tab routing', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    useSettingsStore.setState({
      activeTab: 'basics',
      contentFocus: null,
      contentFocusRequestId: 0,
      tabTransitionTarget: null,
      tabTransitionMotion: 'instant',
      tabTransitionSequence: 0,
      searchQuery: '',
    });
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  /**
   * The scene holds its very first paint until the active tab's lazy chunk and
   * i18n namespaces are in memory, so a cold entry cannot flash a skeleton and a
   * frame of raw i18n keys. Both land off a macrotask, past what act() flushes.
   */
  async function waitForPanelContent(testId: string) {
    for (let attempt = 0; attempt < 50; attempt += 1) {
      if (container.querySelector(`[data-testid="${testId}"]`)) return;
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 0));
      });
    }
  }

  async function renderActiveTab(
    tab: 'mcp-tools' | 'acp-agents' | 'external-sources' | 'voice-input'
  ) {
    useSettingsStore.setState({ activeTab: tab });
    await act(async () => {
      root.render(<SettingsScene />);
    });
    await waitForPanelContent(`${tab}-config`);
  }

  it('renders the lazy MCP tools config tab', async () => {
    await renderActiveTab('mcp-tools');

    expect(container.querySelector('[data-testid="mcp-tools-config"]')).not.toBeNull();
  });

  it('renders the lazy ACP agents config tab', async () => {
    await renderActiveTab('acp-agents');

    expect(container.querySelector('[data-testid="acp-agents-config"]')).not.toBeNull();
  });

  it('renders the lazy external sources config tab', async () => {
    await renderActiveTab('external-sources');

    expect(container.querySelector('[data-testid="external-sources-config"]')).not.toBeNull();
  });

  it('passes a legacy Hook deep-link focus into External AI applications', async () => {
    useSettingsStore.getState().openTab('external-sources', 'hooks');
    await act(async () => {
      root.render(<SettingsScene />);
    });
    await waitForPanelContent('external-sources-config');

    const externalSources = container.querySelector('[data-testid="external-sources-config"]');
    expect(externalSources?.getAttribute('data-initial-focus')).toBe('hooks');
    expect(externalSources?.getAttribute('data-focus-request-id')).toBe('1');

    await act(async () => {
      useSettingsStore.getState().openTab('external-sources', 'hooks');
    });
    expect(externalSources?.getAttribute('data-focus-request-id')).toBe('2');
  });

  it('renders the lazy voice input config tab', async () => {
    await renderActiveTab('voice-input');

    expect(container.querySelector('[data-testid="voice-input-config"]')).not.toBeNull();
  });

  it('switches settings pages immediately without retaining the outgoing panel', async () => {
    await act(async () => {
      root.render(<SettingsScene />);
    });
    await waitForPanelContent('basics-config');

    /** Only the cold first paint waits for resources; later switches are synchronous. */
    await act(async () => {
      useSettingsStore.setState({ activeTab: 'appearance' });
      await Promise.resolve();
    });

    const scene = container.querySelector('[data-testid="settings-scene"]');
    expect(scene?.getAttribute('data-settings-tab')).toBe('appearance');
    expect(container.querySelector('[data-testid="appearance-config"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="basics-config"]')).toBeNull();
    expect(container.querySelectorAll('[data-testid="settings-scene-content"]')).toHaveLength(1);
  });

  it('bridges a pointer tab switch while making the outgoing panel inert', async () => {
    await act(async () => {
      root.render(<SettingsScene />);
    });
    await waitForPanelContent('basics-config');

    vi.useFakeTimers();
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => (
      window.setTimeout(() => callback(performance.now()), 16)
    ));
    vi.stubGlobal('cancelAnimationFrame', (handle: number) => window.clearTimeout(handle));

    await act(async () => {
      useSettingsStore.getState().setActiveTab('appearance', 'pointer');
      await Promise.resolve();
    });

    const outgoing = container.querySelector('.bitfun-view-transition-boundary__view--outgoing');
    expect(outgoing?.hasAttribute('inert')).toBe(true);
    expect(container.querySelector('[data-testid="basics-config"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="appearance-config"]')).not.toBeNull();

    act(() => vi.advanceTimersByTime(232));
    expect(container.querySelector('[data-testid="basics-config"]')).toBeNull();
    expect(container.querySelectorAll('[data-testid="settings-scene-content"]')).toHaveLength(1);
  });
});
