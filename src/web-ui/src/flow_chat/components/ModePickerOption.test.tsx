// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ModePickerOption } from './ModePickerOption';

vi.mock('@/component-library', () => ({
  Tooltip: ({
    children,
    content,
  }: {
    children: React.ReactNode;
    content: React.ReactNode;
  }) => <div data-tooltip={typeof content === 'string' ? content : undefined}>{children}</div>,
}));

function makeTranslator(values: Record<string, string>) {
  return (key: string, options?: { defaultValue?: string }) => values[key] ?? options?.defaultValue ?? '';
}

describe('ModePickerOption', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
  });

  it('renders localized IntentCoding mode picker entry with description tooltip content', async () => {
    await act(async () => {
      root.render(
        <ModePickerOption
          t={makeTranslator({
            'chatInput.modeNames.IntentCoding': 'Intent Coding',
            'chatInput.modeDescriptions.IntentCoding': 'Intent-aligned coding',
          })}
          modeOption={{
            id: 'IntentCoding',
            name: 'Intent Coding backend',
            description: 'backend description',
          }}
          currentMode="agentic"
          currentLabel="Current"
          onSelect={vi.fn()}
        />,
      );
    });

    expect(container.textContent).toContain('Intent Coding');
    expect(container.querySelector('[data-tooltip]')?.getAttribute('data-tooltip')).toBe(
      'Intent-aligned coding',
    );
  });

  it('marks the current mode and selects IntentCoding on click', async () => {
    const onSelect = vi.fn();

    await act(async () => {
      root.render(
        <ModePickerOption
          t={makeTranslator({})}
          modeOption={{
            id: 'IntentCoding',
            name: 'Intent Coding backend',
            description: 'backend description',
          }}
          currentMode="IntentCoding"
          currentLabel="Current"
          onSelect={onSelect}
        />,
      );
    });

    const option = container.querySelector('.bitfun-chat-input__mode-option') as HTMLElement;
    expect(option.className).toContain('bitfun-chat-input__mode-option--active');
    expect(container.textContent).toContain('Current');

    await act(async () => {
      option.click();
    });

    expect(onSelect).toHaveBeenCalledWith('IntentCoding');
  });
});
