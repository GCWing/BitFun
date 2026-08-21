// @vitest-environment jsdom

/**
 * AssistantDefaultsPage component-level toggle-logic tests (L5-P2-2).
 *
 * Covers the "toggle -> persist -> re-render" loop:
 * 1. Initial load reflects the persisted enabled_tools (from
 *    configAPI.getAgentProfileConfig)
 * 2. Clicking a tool Switch persists the new enabled_tools via
 *    configAPI.setAgentProfileConfig and re-renders the checked state
 * 3. Toggle interaction writes the user-configured localStorage marker
 *
 * Previously only AssistantDefaultsPage.presentation.test.ts covered SCSS
 * styles; no component-level guard existed for the toggle logic.
 */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

// ── mocks ──────────────────────────────────────────────────────────────

const mocks = vi.hoisted(() => ({
  setAgentProfileConfig: vi.fn(async () => 'ok'),
  getAgentProfileConfig: vi.fn(async () => ({
    agent_id: 'Claw',
    enabled_tools: ['Read', 'Grep'],
    default_tools: ['Read', 'Grep'],
  })),
  resetAgentProfileConfig: vi.fn(async () => 'ok'),
  getModeSkillConfigs: vi.fn(async () => []),
}));

vi.mock('@/infrastructure/api/service-api/ConfigAPI', () => ({
  configAPI: {
    getAgentProfileConfig: mocks.getAgentProfileConfig,
    setAgentProfileConfig: mocks.setAgentProfileConfig,
    resetAgentProfileConfig: mocks.resetAgentProfileConfig,
    getModeSkillConfigs: mocks.getModeSkillConfigs,
    setModeSkillDisabled: vi.fn(async () => 'ok'),
  },
}));

vi.mock('@/infrastructure/api/service-api/MCPAPI', () => ({
  MCPAPI: {
    getServers: vi.fn(async () => []),
  },
}));

vi.mock('@/infrastructure/event-bus', () => ({
  globalEventBus: {
    emit: vi.fn(),
    on: vi.fn(),
    off: vi.fn(),
  },
}));

vi.mock('@/shared/notification-system', () => ({
  notificationService: {
    error: vi.fn(),
    success: vi.fn(),
  },
}));

vi.mock('@/app/scenes/profile/nurseryStore', () => ({
  useNurseryStore: () => ({
    openGallery: vi.fn(),
  }),
}));

vi.mock('@/infrastructure/config/skillSourcePresentation', () => ({
  buildSkillCoverageSourceMap: () => new Map(),
  formatSkillOrigin: () => 'builtin',
  getModeSkillRuntimeStatus: () => ({ kind: 'enabled' }),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('@/component-library', () => ({
  Switch: ({
    checked,
    onChange,
    loading: _loading,
    disabled: _disabled,
    size: _size,
    'aria-label': ariaLabel,
  }: {
    checked: boolean;
    onChange?: () => void;
    loading?: boolean;
    disabled?: boolean;
    size?: string;
    'aria-label'?: string;
  }) => (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={ariaLabel}
      onClick={onChange}
      data-testid={`switch-${ariaLabel}`}
    />
  ),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === 'get_all_tools_info') {
      return [
        { name: 'Read', description: 'read', is_readonly: true },
        { name: 'Grep', description: 'grep', is_readonly: true },
        { name: 'GetToolSpec', description: 'gateway', is_readonly: true },
      ];
    }
    return [];
  }),
}));

vi.mock('@/app/components', () => ({
  GalleryZone: ({
    title,
    children,
    tools,
  }: {
    title: string;
    children?: React.ReactNode;
    tools?: React.ReactNode;
  }) => (
    <section data-testid={`zone-${title}`}>
      <h3>{title}</h3>
      {tools}
      {children}
    </section>
  ),
}));

import AssistantDefaultsPage from './AssistantDefaultsPage';

describe('AssistantDefaultsPage toggle -> persist -> re-render', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    vi.clearAllMocks();
    localStorage.clear();
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('reflects persisted enabled_tools on initial load', async () => {
    // Same marker: avoid default-select-all refill mutating the config under test.
    localStorage.setItem('assistant-defaults:user-configured', '1');
    mocks.getAgentProfileConfig.mockResolvedValueOnce({
      agent_id: 'Claw',
      enabled_tools: ['Read', 'Grep'],
      default_tools: ['Read', 'Grep'],
    });
    await act(async () => {
      root.render(<AssistantDefaultsPage />);
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    // Read / Grep start checked from persisted config
    const readSwitch = container.querySelector('[data-testid="switch-Read"]');
    const grepSwitch = container.querySelector('[data-testid="switch-Grep"]');
    expect(readSwitch?.getAttribute('aria-checked')).toBe('true');
    expect(grepSwitch?.getAttribute('aria-checked')).toBe('true');
  });

  it('persists a new enabled_tools via setAgentProfileConfig and re-renders', async () => {
    // Mark as user-configured so the first-visit default-select-all effect
    // does not auto-enable every selectable tool (which would make Grep
    // checked before the toggle under test).
    localStorage.setItem('assistant-defaults:user-configured', '1');
    mocks.getAgentProfileConfig.mockResolvedValueOnce({
      agent_id: 'Claw',
      enabled_tools: ['Read'],
      default_tools: ['Read'],
    });
    await act(async () => {
      root.render(<AssistantDefaultsPage />);
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    // Initial state: Read enabled, Grep disabled
    expect(
      container.querySelector('[data-testid="switch-Grep"]')?.getAttribute('aria-checked'),
    ).toBe('false');

    // Click Grep switch to enable it
    await act(async () => {
      container
        .querySelector('[data-testid="switch-Grep"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    // Persist call: setAgentProfileConfig receives new enabled_tools containing Grep
    expect(mocks.setAgentProfileConfig).toHaveBeenCalledTimes(1);
    const [agentId, config] = mocks.setAgentProfileConfig.mock.calls[0];
    expect(agentId).toBe('Claw');
    expect((config as { enabled_tools: string[] }).enabled_tools).toContain('Grep');

    // Re-render: Grep switch now checked
    expect(
      container.querySelector('[data-testid="switch-Grep"]')?.getAttribute('aria-checked'),
    ).toBe('true');
  });

  it('writes the user-configured localStorage marker on toggle interaction', async () => {
    // No marker initially: this test asserts the toggle writes it.
    mocks.getAgentProfileConfig.mockResolvedValueOnce({
      agent_id: 'Claw',
      enabled_tools: ['Read'],
      default_tools: ['Read'],
    });
    await act(async () => {
      root.render(<AssistantDefaultsPage />);
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    expect(localStorage.getItem('assistant-defaults:user-configured')).toBeNull();

    await act(async () => {
      container
        .querySelector('[data-testid="switch-Read"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    expect(localStorage.getItem('assistant-defaults:user-configured')).toBe('1');
  });
});
