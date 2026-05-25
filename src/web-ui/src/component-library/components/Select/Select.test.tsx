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
  let innerHeight = 800;

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
      if (this instanceof HTMLElement && this.classList.contains('select')) {
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
    getBoundingClientRectSpy.mockRestore();
    offsetHeightSpy.mockRestore();
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
    const dropdown = container.querySelector('.select__dropdown');

    expect(selectRoot?.className).toContain('select--placement-top');
    expect(dropdown?.className).toContain('select__dropdown--top');
  });

  it('keeps the dropdown downward when there is enough room below', async () => {
    getBoundingClientRectSpy.mockImplementation(function () {
      if (this instanceof HTMLElement && this.classList.contains('select')) {
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
    const dropdown = container.querySelector('.select__dropdown');

    expect(selectRoot?.className).toContain('select--placement-bottom');
    expect(dropdown?.className).toContain('select__dropdown--bottom');
  });
});
