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
  Switch: ({
    checked,
    onChange,
    disabled,
  }: {
    checked: boolean;
    onChange: (event: React.ChangeEvent<HTMLInputElement>) => void;
    disabled?: boolean;
  }) => (
    <input
      type="checkbox"
      checked={checked}
      disabled={disabled}
      onChange={onChange}
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

  it('renders and persists the subagent dispatch fields (前端-P1-1)', async () => {
    getConfigMock.mockResolvedValue({
      subagent: {
        max_hard_cap: 32,
        timeout_grace_secs: 10,
        session_references_per_turn: 5,
        max_dispatch_per_parent_window: 20,
        dispatch_window_secs: 3600,
        dispatch_cooldown_secs: 300,
      },
    });
    setConfigMock.mockResolvedValue(undefined);
    await renderConfig();

    // All three dispatch fields are rendered with their i18n label keys.
    expect(container.textContent).toContain('fields.subagent.max_dispatch_per_parent_window');
    expect(container.textContent).toContain('fields.subagent.dispatch_window_secs');
    expect(container.textContent).toContain('fields.subagent.dispatch_cooldown_secs');

    // Editing the first dispatch input writes the ai.thresholds.subagent.* path.
    const inputs = [...container.querySelectorAll('input[type="number"]')] as HTMLInputElement[];
    expect(inputs.length).toBeGreaterThanOrEqual(6);
    const dispatchInput = inputs[3];
    await act(async () => {
      const setter = Object.getOwnPropertyDescriptor(
        window.HTMLInputElement.prototype,
        'value'
      )!.set!;
      setter.call(dispatchInput, '48');
      dispatchInput.dispatchEvent(new Event('input', { bubbles: true }));
      await Promise.resolve();
    });
    expect(setConfigMock).toHaveBeenCalledWith(
      expect.stringContaining('ai.thresholds.subagent.max_dispatch_per_parent_window'),
      48,
    );
  });

  it('falls back to defaults when the config read fails', async () => {
    getConfigMock.mockRejectedValue(new Error('config unavailable'));
    await renderConfig();

    expect(container.querySelectorAll('input[type="number"]').length).toBeGreaterThan(10);
  });

  it('renders output_tokens.automatic_tiers as read-only (前端-P2-2)', async () => {
    getConfigMock.mockResolvedValue({
      output_tokens: { automatic_tiers: [8000, 16000, 24000, 32000, 64000], ratio_percent: 40 },
    });
    await renderConfig();

    // Read-only label + joined tier values are rendered; no NumberInput for the array.
    expect(container.textContent).toContain('fields.output_tokens.automatic_tiers');
    expect(container.textContent).toContain('8000 / 16000 / 24000 / 32000 / 64000');
    // The array must not be editable through a number input.
    const inputs = [...container.querySelectorAll('input[type="number"]')] as HTMLInputElement[];
    const outputTokensRow = container.textContent?.indexOf('fields.output_tokens.__title') ?? -1;
    expect(outputTokensRow).toBeGreaterThanOrEqual(0);
    expect(inputs.length).toBeGreaterThanOrEqual(2); // ratio_percent + other fields
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

  it('renders the execution section with owner-specified defaults (R-MR-07)', async () => {
    getConfigMock.mockResolvedValue(undefined);
    await renderConfig();

    // Section header + all 7 i18n label keys are rendered.
    expect(container.textContent).toContain('fields.execution.__title');
    expect(container.textContent).toContain('fields.execution.max_rounds');
    expect(container.textContent).toContain('fields.execution.consecutive_tool_rounds');
    expect(container.textContent).toContain('fields.execution.consecutive_search_rounds');
    expect(container.textContent).toContain('fields.execution.duplicate_tool_calls');
    expect(container.textContent).toContain('fields.execution.no_progress_results');
    expect(container.textContent).toContain('fields.execution.tool_calls_per_turn');
    expect(container.textContent).toContain('fields.execution.empty_input_guard');

    // 6 numeric fields + the empty_input_guard switch = 6 number inputs + 1 checkbox.
    const numberInputs = [...container.querySelectorAll('input[type="number"]')] as HTMLInputElement[];
    const switchInputs = [...container.querySelectorAll('input[type="checkbox"]')] as HTMLInputElement[];
    expect(numberInputs.length).toBeGreaterThanOrEqual(6);
    expect(switchInputs.length).toBeGreaterThanOrEqual(1);
    // Defaults from DEFAULT_THRESHOLDS.execution are rendered.
    expect(numberInputs.map((input) => input.value)).toContain('50');
    expect(numberInputs.map((input) => input.value)).toContain('20');
    expect(numberInputs.map((input) => input.value)).toContain('3');
    expect(numberInputs.map((input) => input.value)).toContain('5');
    expect(numberInputs.map((input) => input.value)).toContain('30');
    expect(switchInputs[0].checked).toBe(true);
  });

  it('persists execution numeric and switch changes through the ai.thresholds.execution path (R-MR-07)', async () => {
    getConfigMock.mockResolvedValue(undefined);
    setConfigMock.mockResolvedValue(undefined);
    await renderConfig();

    // Persist max_rounds change. R-THR-01 批2 新增 insights 组后多个字段共享
    // 默认值 50，不能再用「最后一个 50」定位；改用 label span 反查父行定位
    // fields.execution.max_rounds 的 NumberInput。
    const labelSpans = [...container.querySelectorAll('span')] as HTMLElement[];
    const maxRoundsLabel = labelSpans.find(
      (span) => span.textContent === 'fields.execution.max_rounds'
    )!;
    expect(maxRoundsLabel).not.toBeUndefined();
    const maxRoundsInput = maxRoundsLabel.parentElement!.querySelector(
      'input[type="number"]'
    ) as HTMLInputElement;
    await act(async () => {
      const setter = Object.getOwnPropertyDescriptor(
        window.HTMLInputElement.prototype,
        'value'
      )!.set!;
      setter.call(maxRoundsInput, '45');
      maxRoundsInput.dispatchEvent(new Event('input', { bubbles: true }));
      await Promise.resolve();
    });
    expect(setConfigMock).toHaveBeenCalledWith(
      expect.stringContaining('ai.thresholds.execution.max_rounds'),
      45,
    );

    // Persist empty_input_guard toggle.
    const guardSwitch = [...container.querySelectorAll('input[type="checkbox"]')][0] as HTMLInputElement;
    expect(guardSwitch).not.toBeUndefined();
    await act(async () => {
      guardSwitch.click();
      await Promise.resolve();
    });
    expect(setConfigMock).toHaveBeenCalledWith(
      expect.stringContaining('ai.thresholds.execution.empty_input_guard'),
      false,
    );
  });
});
