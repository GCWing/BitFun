// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import ThresholdsConfig from './ThresholdsConfig';

const getConfigMock = vi.hoisted(() => vi.fn());
const setConfigMock = vi.hoisted(() => vi.fn());
const resetConfigMock = vi.hoisted(() => vi.fn());
const notificationSuccessMock = vi.hoisted(() => vi.fn());
const notificationErrorMock = vi.hoisted(() => vi.fn());
const translateMock = vi.hoisted(() => vi.fn((key: string) => key));

vi.mock('../services/ConfigManager', () => ({
  configManager: {
    getConfig: getConfigMock,
    setConfig: setConfigMock,
    resetConfig: resetConfigMock,
  },
}));

vi.mock('@/shared/notification-system', () => ({
  useNotification: () => ({
    success: notificationSuccessMock,
    error: notificationErrorMock,
  }),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: translateMock }),
}));

vi.mock('@/component-library', () => ({
  Button: ({
    children,
    disabled,
    onClick,
  }: {
    children: React.ReactNode;
    disabled?: boolean;
    onClick?: () => void;
  }) => (
    <button type="button" disabled={disabled} onClick={onClick}>
      {children}
    </button>
  ),
  ConfigPageLoading: ({ text }: { text: string }) => <div>{text}</div>,
  NumberInput: ({
    value,
    onChange,
    disabled,
    min,
  }: {
    value: number;
    onChange: (value: number) => void;
    disabled?: boolean;
    min?: number;
  }) => (
    <input
      type="number"
      value={value}
      min={min}
      disabled={disabled}
      onChange={(event) => onChange(Number(event.target.value))}
    />
  ),
}));

vi.mock('./common', () => ({
  ConfigPageLayout: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  ConfigPageContent: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  ConfigPageSection: ({ title, children }: { title: string; children: React.ReactNode }) => (
    <section>
      <h3>{title}</h3>
      {children}
    </section>
  ),
  ConfigPageRow: ({ label, children }: { label: React.ReactNode; children: React.ReactNode }) => (
    <div>
      <span>{label}</span>
      {children}
    </div>
  ),
  ConfigPageHeader: ({ title, subtitle, extra }: { title: string; subtitle?: string; extra?: React.ReactNode }) => (
    <header>
      <h2>{title}</h2>
      <p>{subtitle}</p>
      {extra}
    </header>
  ),
}));

let container: HTMLElement;
let root: Root;

beforeEach(() => {
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
  vi.clearAllMocks();
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
});

async function renderConfig(): Promise<void> {
  await act(async () => {
    root.render(<ThresholdsConfig />);
    await Promise.resolve();
  });
}

describe('ThresholdsConfig', () => {
  it('renders domain sections with configured values', async () => {
    getConfigMock.mockResolvedValue({
      subagent: { max_hard_cap: 32, timeout_grace_secs: 10, session_references_per_turn: 5 },
    });
    await renderConfig();

    // Header from i18n (translated key echoed back by the mock).
    expect(container.textContent).toContain('title');
    // Subagent section header + configured value rendered through NumberInput.
    expect(container.textContent).toContain('fields.subagent.__title');
    expect(container.querySelector('input[type="number"]')).not.toBeNull();
  });

  it('falls back to defaults when the config read fails', async () => {
    getConfigMock.mockRejectedValue(new Error('config unavailable'));
    await renderConfig();

    expect(container.querySelectorAll('input[type="number"]').length).toBeGreaterThan(10);
  });

  it('persists a field change through setConfig with the ai.thresholds path', async () => {
    getConfigMock.mockResolvedValue(undefined);
    setConfigMock.mockResolvedValue(undefined);
    await renderConfig();

    const input = container.querySelector('input[type="number"]');
    expect(input).not.toBeNull();

    await act(async () => {
      input!.dispatchEvent(new Event('change', { bubbles: true }));
      // NumberInput onChange forwards the numeric value; drive via native setter.
      const setter = Object.getOwnPropertyDescriptor(
        window.HTMLInputElement.prototype,
        'value'
      )!.set!;
      setter.call(input, '48');
      input!.dispatchEvent(new Event('input', { bubbles: true }));
      await Promise.resolve();
    });

    expect(setConfigMock).toHaveBeenCalledWith(
      expect.stringContaining('ai.thresholds.subagent.max_hard_cap'),
      48,
    );
  });

  it('resets the config through resetConfig', async () => {
    getConfigMock.mockResolvedValue(undefined);
    resetConfigMock.mockResolvedValue(undefined);
    await renderConfig();

    const resetButton = [...container.querySelectorAll('button')].find((button) =>
      button.textContent?.includes('actions.resetToDefaults')
    );
    expect(resetButton).not.toBeUndefined();

    await act(async () => {
      resetButton!.click();
      await Promise.resolve();
    });

    expect(resetConfigMock).toHaveBeenCalledWith('ai.thresholds');
    expect(notificationSuccessMock).toHaveBeenCalled();
  });
});
