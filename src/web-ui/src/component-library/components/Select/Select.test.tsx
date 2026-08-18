// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { Select } from './Select';

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({
    t: (key: string, options?: Record<string, unknown> & { defaultValue?: string }) => (
      options?.defaultValue ?? key
    ),
  }),
}));

describe('Select', () => {
  let container: HTMLDivElement;
  let root: Root;
  let getBoundingClientRectSpy: ReturnType<typeof vi.spyOn>;
  let offsetHeightSpy: ReturnType<typeof vi.spyOn>;
  const innerHeight = 800;

  beforeEach(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    Object.defineProperty(window, 'innerHeight', {
      configurable: true,
      value: innerHeight,
    });

    getBoundingClientRectSpy = vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function () {
      if (this instanceof HTMLElement && this.classList.contains('select__trigger')) {
        return {
          top: 700,
          bottom: 740,
          left: 0,
          right: 240,
          width: 240,
          height: 40,
          x: 0,
          y: 700,
          toJSON() {
            return this;
          },
        } as DOMRect;
      }
      return {
        top: 0,
        bottom: 0,
        left: 0,
        right: 0,
        width: 0,
        height: 0,
        x: 0,
        y: 0,
        toJSON() {
          return this;
        },
      } as DOMRect;
    });

    offsetHeightSpy = vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockImplementation(function () {
      if (this instanceof HTMLElement && this.classList.contains('select__dropdown')) {
        return 220;
      }
      return 0;
    });
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    document.querySelector('[data-bf-overlay-host="true"]')?.remove();
    getBoundingClientRectSpy.mockRestore();
    offsetHeightSpy.mockRestore();
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('flips the dropdown upward when there is not enough room below', async () => {
    await act(async () => {
      root.render(
        <Select
          options={[
            { value: 'ask', label: 'Ask' },
            { value: 'allow_once', label: 'Allow once' },
          ]}
          value="ask"
        />
      );
    });

    const trigger = container.querySelector('.select__trigger') as HTMLElement;
    expect(trigger).toBeTruthy();

    await act(async () => {
      trigger.click();
      await Promise.resolve();
    });

    const selectRoot = container.querySelector('.select');
    const dropdown = document.querySelector<HTMLElement>('.select__dropdown');

    expect(selectRoot?.className).toContain('select--placement-top');
    expect(dropdown?.className).toContain('select__dropdown--top');
    expect(dropdown?.parentElement?.getAttribute('data-bf-overlay-host')).toBe('true');
    expect(dropdown?.style.position).toBe('fixed');
    expect(dropdown?.style.top).toBe('480px');
    expect(dropdown?.style.width).toBe('240px');
  });

  it('keeps the dropdown downward when there is enough room below', async () => {
    getBoundingClientRectSpy.mockImplementation(function () {
      if (this instanceof HTMLElement && this.classList.contains('select__trigger')) {
        return {
          top: 100,
          bottom: 140,
          left: 0,
          right: 240,
          width: 240,
          height: 40,
          x: 0,
          y: 100,
          toJSON() {
            return this;
          },
        } as DOMRect;
      }
      return {
        top: 0,
        bottom: 0,
        left: 0,
        right: 0,
        width: 0,
        height: 0,
        x: 0,
        y: 0,
        toJSON() {
          return this;
        },
      } as DOMRect;
    });

    await act(async () => {
      root.render(
        <Select
          options={[
            { value: 'ask', label: 'Ask' },
            { value: 'allow_once', label: 'Allow once' },
          ]}
          value="ask"
        />
      );
    });

    const trigger = container.querySelector('.select__trigger') as HTMLElement;

    await act(async () => {
      trigger.click();
      await Promise.resolve();
    });

    const selectRoot = container.querySelector('.select');
    const dropdown = document.querySelector<HTMLElement>('.select__dropdown');

    expect(selectRoot?.className).toContain('select--placement-bottom');
    expect(dropdown?.className).toContain('select__dropdown--bottom');
  });

  it('repositions a portalled dropdown when an ancestor scrolls', async () => {
    let triggerTop = 100;
    getBoundingClientRectSpy.mockImplementation(function () {
      if (this instanceof HTMLElement && this.classList.contains('select__trigger')) {
        return {
          top: triggerTop,
          bottom: triggerTop + 40,
          left: 80,
          right: 320,
          width: 240,
          height: 40,
          x: 80,
          y: triggerTop,
          toJSON() { return this; },
        } as DOMRect;
      }
      return {
        top: 0,
        bottom: 0,
        left: 0,
        right: 0,
        width: 0,
        height: 0,
        x: 0,
        y: 0,
        toJSON() { return this; },
      } as DOMRect;
    });

    await act(async () => {
      root.render(<Select options={[{ value: 'one', label: 'One' }]} />);
    });
    const trigger = container.querySelector<HTMLElement>('.select__trigger');
    await act(async () => trigger?.click());
    const dropdown = document.querySelector<HTMLElement>('.select__dropdown');
    expect(dropdown?.style.top).toBe('140px');

    triggerTop = 300;
    await act(async () => {
      window.dispatchEvent(new Event('scroll'));
      await new Promise<void>((resolve) => window.requestAnimationFrame(() => resolve()));
    });

    expect(dropdown?.style.top).toBe('340px');
  });

  it('keeps grouped order stable and skips disabled options during keyboard navigation', async () => {
    const onChange = vi.fn();
    Object.defineProperty(HTMLElement.prototype, 'scrollIntoView', {
      configurable: true,
      value: vi.fn(),
    });
    await act(async () => {
      root.render(
        <Select
          options={[
            { value: 'disabled', label: 'Disabled ungrouped', disabled: true },
            { value: 'group-a', label: 'Group A choice', group: 'Group A' },
            { value: 'group-b', label: 'Group B choice', group: 'Group B' },
          ]}
          onChange={onChange}
        />
      );
    });

    const trigger = container.querySelector('.select__trigger') as HTMLElement;
    await act(async () => {
      trigger.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
      await Promise.resolve();
    });

    const listbox = document.querySelector('[role="listbox"]') as HTMLElement;
    const options = Array.from(document.querySelectorAll<HTMLElement>('[role="option"]'));
    expect(options.map((option) => option.textContent)).toEqual([
      'Disabled ungrouped',
      'Group A choice',
      'Group B choice',
    ]);
    expect(trigger.getAttribute('aria-controls')).toBe(listbox.id);
    expect(trigger.getAttribute('aria-activedescendant')).toBe(options[1].id);
    expect(options[1].className).toContain('select__option--highlighted');
    expect(document.querySelector('[role="group"]')?.getAttribute('aria-label')).toBe('Group A');

    await act(async () => {
      trigger.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    });
    expect(onChange).toHaveBeenCalledWith('group-a');
  });

  it('links the searchable input to the listbox and active option', async () => {
    Object.defineProperty(HTMLElement.prototype, 'scrollIntoView', {
      configurable: true,
      value: vi.fn(),
    });
    await act(async () => {
      root.render(
        <Select
          searchable
          searchPlaceholder="Find a choice"
          options={[
            { value: 'disabled', label: 'Disabled', disabled: true },
            { value: 'enabled', label: 'Enabled' },
          ]}
        />
      );
    });
    const trigger = container.querySelector('.select__trigger') as HTMLElement;
    await act(async () => trigger.click());
    const input = document.querySelector('.select__search-input') as HTMLInputElement;
    const listbox = document.querySelector('[role="listbox"]') as HTMLElement;

    await act(async () => {
      input.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
      await Promise.resolve();
    });

    expect(input.getAttribute('aria-controls')).toBe(listbox.id);
    expect(input.getAttribute('aria-label')).toBe('Find a choice');
    expect(input.getAttribute('aria-activedescendant')).toBe(
      document.querySelectorAll<HTMLElement>('[role="option"]')[1].id,
    );
  });

  it('keeps portalled interactions inside the Select and safely retains an outside-closed dropdown', async () => {
    await act(async () => {
      root.render(<Select options={[{ value: 'one', label: 'One' }]} />);
    });
    const trigger = container.querySelector<HTMLElement>('.select__trigger');
    await act(async () => trigger?.click());

    const dropdown = document.querySelector<HTMLElement>('.select__dropdown');
    await act(async () => {
      dropdown?.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
    });
    expect(document.querySelector('.select__dropdown')).not.toBeNull();

    vi.useFakeTimers();
    await act(async () => {
      document.body.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
    });
    const closingDropdown = document.querySelector<HTMLElement>('.select__dropdown');
    expect(closingDropdown).not.toBeNull();
    expect(closingDropdown?.getAttribute('data-open')).toBe('false');
    expect(closingDropdown?.getAttribute('aria-hidden')).toBe('true');
    expect(closingDropdown?.hasAttribute('inert')).toBe(true);
    expect(closingDropdown?.getAttribute('tabindex')).toBe('-1');

    act(() => vi.advanceTimersByTime(119));
    expect(document.querySelector('.select__dropdown')).not.toBeNull();
    act(() => vi.advanceTimersByTime(1));
    expect(document.querySelector('.select__dropdown')).toBeNull();
  });

  it('keeps the last options and resolved placement stable during exit', async () => {
    await act(async () => {
      root.render(
        <Select
          options={[{ value: 'old', label: 'Original option' }]}
          placement="top"
        />,
      );
    });
    const trigger = container.querySelector<HTMLElement>('.select__trigger');
    await act(async () => trigger?.click());

    expect(document.querySelector('.select__dropdown--top')).not.toBeNull();
    expect(document.querySelector('.select__option')?.textContent).toContain('Original option');

    vi.useFakeTimers();
    await act(async () => trigger?.click());
    await act(async () => {
      root.render(
        <Select
          options={[{ value: 'new', label: 'Replacement option' }]}
          placement="bottom"
        />,
      );
    });

    const closingDropdown = document.querySelector<HTMLElement>('.select__dropdown');
    expect(closingDropdown?.className).toContain('select__dropdown--top');
    expect(closingDropdown?.textContent).toContain('Original option');
    expect(closingDropdown?.textContent).not.toContain('Replacement option');

    act(() => vi.advanceTimersByTime(120));
    expect(document.querySelector('.select__dropdown')).toBeNull();
  });

  it('cancels pending dropdown removal when reopened quickly', async () => {
    await act(async () => {
      root.render(<Select options={[{ value: 'one', label: 'One' }]} />);
    });
    const trigger = container.querySelector<HTMLElement>('.select__trigger');
    await act(async () => trigger?.click());

    vi.useFakeTimers();
    await act(async () => trigger?.click());
    act(() => vi.advanceTimersByTime(60));
    await act(async () => trigger?.click());
    act(() => vi.advanceTimersByTime(120));

    const reopenedDropdown = document.querySelector<HTMLElement>('.select__dropdown');
    expect(reopenedDropdown).not.toBeNull();
    expect(reopenedDropdown?.getAttribute('data-open')).toBe('true');
    expect(reopenedDropdown?.getAttribute('aria-hidden')).toBe('false');
    expect(reopenedDropdown?.hasAttribute('inert')).toBe(false);
  });

  it('opens from the keyboard without motion and returns focus when closed', async () => {
    Object.defineProperty(HTMLElement.prototype, 'scrollIntoView', {
      configurable: true,
      value: vi.fn(),
    });
    await act(async () => {
      root.render(
        <Select
          searchable
          options={[{ value: 'one', label: 'One' }]}
        />,
      );
    });
    const trigger = container.querySelector<HTMLElement>('.select__trigger');
    await act(async () => {
      trigger?.dispatchEvent(new KeyboardEvent('keydown', {
        key: 'ArrowDown',
        bubbles: true,
      }));
    });

    const openDropdown = document.querySelector<HTMLElement>('.select__dropdown');
    const searchInput = document.querySelector<HTMLInputElement>('.select__search-input');
    expect(openDropdown?.getAttribute('data-keyboard-open')).toBe('true');
    expect(document.activeElement).toBe(searchInput);

    vi.useFakeTimers();
    await act(async () => {
      searchInput?.dispatchEvent(new KeyboardEvent('keydown', {
        key: 'Escape',
        bubbles: true,
      }));
    });

    const closingDropdown = document.querySelector<HTMLElement>('.select__dropdown');
    expect(document.activeElement).toBe(trigger);
    expect(closingDropdown?.getAttribute('data-open')).toBe('false');
    expect(closingDropdown?.getAttribute('data-keyboard-open')).toBe('true');
    expect(searchInput?.tabIndex).toBe(-1);
  });

  it('retains the explicit inline mode for layout-expanding selectors', async () => {
    await act(async () => {
      root.render(
        <Select
          dropdownMode="inline"
          options={[{ value: 'one', label: 'One' }]}
        />,
      );
    });
    const trigger = container.querySelector<HTMLElement>('.select__trigger');
    await act(async () => trigger?.click());

    const dropdown = container.querySelector<HTMLElement>('.select__dropdown');
    expect(dropdown).not.toBeNull();
    expect(dropdown?.className).toContain('select__dropdown--inline');
    expect(dropdown?.style.position).toBe('');
  });
});
