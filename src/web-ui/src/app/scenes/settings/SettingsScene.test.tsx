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
  default: () => <div data-testid="external-sources-config" />,
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
    useSettingsStore.setState({ activeTab: 'basics', searchQuery: '' });
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
  });

  async function renderActiveTab(tab: 'mcp-tools' | 'acp-agents' | 'external-sources') {
    useSettingsStore.setState({ activeTab: tab });
    await act(async () => {
      root.render(<SettingsScene />);
    });
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
});
