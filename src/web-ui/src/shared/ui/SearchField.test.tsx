// @vitest-environment jsdom
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Icon, SearchField, type SearchFieldProps } from '@openbitfun/ui';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

describe('public SearchField product integration', () => {
  let host: HTMLDivElement;
  let root: Root;
  const clear = vi.fn();
  const search = vi.fn();
  const render = (props: Partial<SearchFieldProps> = {}) => act(() => root.render(
    <SearchField
      aria-label="Search sessions"
      clearLabel="Clear search"
      defaultValue="OpenBitFun"
      leadingIcon={<Icon name="search" />}
      onClear={clear}
      onSearch={search}
      shortcut="Ctrl K"
      size="sm"
      {...props}
    />,
  ));
  const clearButton = () => host.querySelector<HTMLButtonElement>('button[aria-label="Clear search"]')!;

  beforeEach(() => {
    host = document.createElement('div');
    document.body.append(host);
    root = createRoot(host);
    clear.mockClear();
    search.mockClear();
  });
  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it('keeps shortcut, trailing content, and clear action together without hiding the action', () => {
    render({ trailing: <span role="status">1 / 5</span> });
    expect(host.textContent).toContain('1 / 5Ctrl K');
    expect(clearButton().closest('[aria-hidden="true"]')).toBeNull();
    const iconWrapper = host.querySelector('[data-openbitfun-component="input"] > [data-openbitfun-part="leading"] > [data-openbitfun-part="icon"]');
    expect(iconWrapper?.querySelector('[data-openbitfun-component="icon"][data-size="lg"]')).not.toBeNull();
    act(() => clearButton().click());
    expect(clear).toHaveBeenCalledTimes(1);
  });

  it.each([{ disabled: true }, { readOnly: true }])('blocks clearing an immutable field: %o', (props) => {
    render(props);
    const input = host.querySelector('input')!;
    expect(input.disabled || input.readOnly).toBe(true);
    expect(clearButton().disabled).toBe(true);
    act(() => {
      clearButton().click();
      clearButton().dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(clear).not.toHaveBeenCalled();
    expect(input.value).toBe('OpenBitFun');
    render();
    act(() => clearButton().click());
    expect(clear).toHaveBeenCalledTimes(1);
  });

  it('preserves focused editing on clear and respects canceled or IME-owned Enter', () => {
    const onKeyDown = vi.fn((event: React.KeyboardEvent<HTMLInputElement>) => event.preventDefault());
    render({ onKeyDown });
    const input = host.querySelector('input')!;
    act(() => input.focus());
    const mouseDown = new MouseEvent('mousedown', { bubbles: true, cancelable: true });
    act(() => clearButton().dispatchEvent(mouseDown));
    expect(mouseDown.defaultPrevented).toBe(true);
    expect(document.activeElement).toBe(input);
    act(() => input.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, cancelable: true, key: 'Enter' })));
    expect(onKeyDown).toHaveBeenCalledTimes(1);
    expect(search).not.toHaveBeenCalled();
    render();
    act(() => input.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Enter', isComposing: true })));
    expect(search).not.toHaveBeenCalled();
    act(() => input.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Enter' })));
    expect(search).toHaveBeenCalledWith('OpenBitFun');
  });
});
