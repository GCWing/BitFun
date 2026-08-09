// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ReasoningConfig } from '../types';
import ReasoningConfigPanel from './ReasoningConfigPanel';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@/component-library', () => ({
  Button: ({
    children,
    variant: _variant,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement> & { variant?: string }) => (
    <button type="button" {...props}>{children}</button>
  ),
}));

vi.mock('./ReasoningPresetEditor', () => ({
  default: ({
    value,
    onChange,
    onValidationChange,
  }: {
    value: ReasoningConfig;
    onChange: (value: ReasoningConfig) => void;
    onValidationChange: (invalid: boolean) => void;
  }) => (
    <div>
      <button
        type="button"
        data-testid="change-reasoning"
        onClick={() => onChange({
          ...value,
          presets: [{ id: 'custom', actions: [{ type: 'effort', value: 'high' }] }],
        })}
      >
        change
      </button>
      <button
        type="button"
        data-testid="invalidate-reasoning"
        onClick={() => onValidationChange(true)}
      >
        invalidate
      </button>
    </div>
  ),
}));

describe('ReasoningConfigPanel', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
  });

  it('keeps edits local until Apply is selected', () => {
    const onApply = vi.fn();
    act(() => root.render(
      <ReasoningConfigPanel
        value={{ catalog: { source: 'auto' }, presets: [] }}
        onCancel={vi.fn()}
        onApply={onApply}
      />,
    ));

    act(() => container.querySelector<HTMLButtonElement>('[data-testid="change-reasoning"]')?.click());
    expect(onApply).not.toHaveBeenCalled();

    const apply = Array.from(container.querySelectorAll('button'))
      .find(button => button.textContent === 'reasoningPresets.apply');
    act(() => apply?.click());

    expect(onApply).toHaveBeenCalledWith({
      catalog: { source: 'auto' },
      presets: [{ id: 'custom', actions: [{ type: 'effort', value: 'high' }] }],
    });
  });

  it('cancels without applying and blocks invalid drafts', () => {
    const onCancel = vi.fn();
    const onApply = vi.fn();
    act(() => root.render(
      <ReasoningConfigPanel
        value={{ catalog: { source: 'auto' }, presets: [] }}
        onCancel={onCancel}
        onApply={onApply}
      />,
    ));

    act(() => container.querySelector<HTMLButtonElement>('[data-testid="invalidate-reasoning"]')?.click());
    const buttons = Array.from(container.querySelectorAll('button'));
    const apply = buttons.find(button => button.textContent === 'reasoningPresets.apply');
    const cancel = buttons.find(button => button.textContent === 'actions.cancel');
    expect(apply?.disabled).toBe(true);

    act(() => cancel?.click());
    expect(onCancel).toHaveBeenCalledOnce();
    expect(onApply).not.toHaveBeenCalled();
  });
});
