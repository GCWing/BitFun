// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { LocalizedCombobox } from './LocalizedCombobox';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({
    t: (key: string, options?: { value?: string }) => (
      key === 'select.useCustomValue' ? `Use "${options?.value ?? ''}"` : key
    ),
  }),
}));

const options = [
  { label: 'Alpha', value: 'alpha' },
  { label: 'Beta', value: 'beta' },
  { label: 'Gamma', value: 'gamma' },
  { disabled: true, label: 'Delta', value: 'delta' },
] as const;

function dispatchKey(target: Element, key: string) {
  target.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key }));
}

function enterText(input: HTMLInputElement, value: string) {
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
  setter?.call(input, value);
  input.dispatchEvent(new Event('input', { bubbles: true }));
}

describe('LocalizedCombobox', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    Object.defineProperty(window, 'requestAnimationFrame', {
      configurable: true,
      value: (callback: FrameRequestCallback) => {
        callback(0);
        return 1;
      },
    });
    Object.defineProperty(window, 'cancelAnimationFrame', {
      configurable: true,
      value: () => undefined,
    });
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.querySelector('[data-bf-overlay-host="true"]')?.remove();
  });

  it('portals into the Appearance host and follows standard selection keys', async () => {
    const onValueChange = vi.fn();
    await act(async () => {
      root.render(
        <LocalizedCombobox
          dropdownTestId="combobox-popover"
          onValueChange={onValueChange}
          options={options}
          triggerAriaLabel="Model"
          triggerTestId="combobox-trigger"
        />,
      );
    });

    const trigger = container.querySelector<HTMLButtonElement>('[data-testid="combobox-trigger"]')!;
    await act(async () => dispatchKey(trigger, 'ArrowDown'));

    const overlayHost = document.querySelector<HTMLElement>('[data-bf-overlay-host="true"]');
    const popover = overlayHost?.querySelector<HTMLElement>('[data-testid="combobox-popover"]');
    expect(popover).not.toBeNull();
    expect(trigger.getAttribute('aria-expanded')).toBe('true');
    expect(popover?.querySelector('[data-value="alpha"]')?.getAttribute('data-active')).toBe('true');

    await act(async () => dispatchKey(trigger, 'End'));
    expect(popover?.querySelector('[data-value="gamma"]')?.getAttribute('data-active')).toBe('true');

    await act(async () => dispatchKey(trigger, 'Home'));
    expect(popover?.querySelector('[data-value="alpha"]')?.getAttribute('data-active')).toBe('true');

    await act(async () => dispatchKey(trigger, 'ArrowDown'));
    await act(async () => dispatchKey(trigger, 'Enter'));
    expect(onValueChange).toHaveBeenLastCalledWith('beta');
    expect(document.querySelector('[data-testid="combobox-popover"]')).toBeNull();
    expect(document.activeElement).toBe(trigger);

    await act(async () => dispatchKey(trigger, 'ArrowDown'));
    expect(document.querySelector('[data-testid="combobox-popover"]')).not.toBeNull();
    await act(async () => dispatchKey(trigger, 'Escape'));
    expect(document.querySelector('[data-testid="combobox-popover"]')).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });

  it('keeps multi-select open while toggling values', async () => {
    const onValueChange = vi.fn();
    await act(async () => {
      root.render(
        <LocalizedCombobox
          defaultValue={['alpha']}
          dropdownTestId="combobox-popover"
          multiple
          onValueChange={onValueChange}
          options={options}
          triggerAriaLabel="Models"
          triggerTestId="combobox-trigger"
        />,
      );
    });

    await act(async () => container.querySelector<HTMLButtonElement>('[data-testid="combobox-trigger"]')?.click());
    const beta = document.querySelector<HTMLButtonElement>('[data-value="beta"]')!;
    await act(async () => beta.click());

    expect(onValueChange).toHaveBeenLastCalledWith(['alpha', 'beta']);
    expect(document.querySelector('[data-testid="combobox-popover"]')).not.toBeNull();
    expect(document.querySelector('[data-value="beta"]')?.getAttribute('aria-selected')).toBe('true');
  });

  it('accepts custom values after IME composition completes', async () => {
    const onValueChange = vi.fn();
    await act(async () => {
      root.render(
        <LocalizedCombobox
          allowCustomValue
          dropdownTestId="combobox-popover"
          onValueChange={onValueChange}
          options={options}
          searchable
          triggerAriaLabel="Model"
          triggerTestId="combobox-trigger"
        />,
      );
    });

    await act(async () => container.querySelector<HTMLButtonElement>('[data-testid="combobox-trigger"]')?.click());
    const search = document.querySelector<HTMLInputElement>('input[role="combobox"]')!;
    await act(async () => {
      search.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
      enterText(search, 'Custom');
      dispatchKey(search, 'Enter');
    });

    expect(onValueChange).not.toHaveBeenCalled();
    expect(document.querySelector('[data-testid="combobox-popover"]')).not.toBeNull();

    await act(async () => {
      search.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true }));
      dispatchKey(search, 'Enter');
    });

    expect(onValueChange).toHaveBeenLastCalledWith('Custom');
    expect(document.querySelector('[data-testid="combobox-popover"]')).toBeNull();
  });

  it('keeps pointer hover separate from keyboard active state', async () => {
    await act(async () => {
      root.render(
        <LocalizedCombobox
          options={options}
          triggerAriaLabel="Model"
          triggerTestId="combobox-trigger"
        />,
      );
    });

    const trigger = container.querySelector<HTMLButtonElement>('[data-testid="combobox-trigger"]')!;
    await act(async () => dispatchKey(trigger, 'ArrowDown'));
    const search = document.querySelector<HTMLInputElement>('input[role="combobox"]')!;
    const alpha = document.querySelector<HTMLElement>('[data-value="alpha"]')!;
    const beta = document.querySelector<HTMLElement>('[data-value="beta"]')!;

    await act(async () => {
      beta.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
      beta.dispatchEvent(new MouseEvent('mousemove', { bubbles: true }));
    });

    expect(alpha.getAttribute('data-active')).toBe('true');
    expect(beta.getAttribute('data-active')).toBe('false');
    expect(search.getAttribute('aria-activedescendant')).toBe(alpha.id);
  });
});
