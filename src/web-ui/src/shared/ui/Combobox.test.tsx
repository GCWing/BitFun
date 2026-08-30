// @vitest-environment jsdom
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Combobox, Field, Modal, type ComboboxProps } from '@bitfun/ui';

describe('public Combobox product integration', () => {
  let root: Root;
  let host: HTMLDivElement;
  const change = vi.fn();
  const render = (props: ComboboxProps = {}) => act(() => root.render(<Combobox label="Models" options={[{ value: 'a', label: 'Alpha', group: 'First' }, { value: 'b', label: 'Beta', disabled: true }, { value: 'c', label: 'Gamma', group: 'First' }]} onChange={change} {...props} />));
  const trigger = () => host.querySelector<HTMLButtonElement>('button[role="combobox"]')!;
  const key = (element: Element, value: string, composing = false) => act(() => { element.dispatchEvent(new KeyboardEvent('keydown', { key: value, isComposing: composing, bubbles: true })); });
  const input = () => document.querySelector<HTMLInputElement>('input[role="combobox"]')!;
  const type = (value: string) => act(() => { Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(input(), value); input().dispatchEvent(new Event('input', { bubbles: true })); });
  beforeEach(() => { host = document.createElement('div'); document.body.append(host); root = createRoot(host); change.mockClear(); });
  afterEach(() => { act(() => root.unmount()); host.remove(); vi.restoreAllMocks(); });

  it('navigates grouped options in DOM order and skips disabled entries', () => {
    render(); key(trigger(), 'ArrowDown'); key(input(), 'ArrowDown'); key(input(), 'Enter');
    expect(change).toHaveBeenLastCalledWith('c');
    expect(document.activeElement).toBe(trigger());
    expect(document.querySelector('[role="listbox"]')).toBeNull();
  });
  it('does not accept an IME confirmation as an option selection', () => {
    render({ allowCustomValue: true }); act(() => trigger().click()); type('custom'); key(input(), 'Enter', true);
    expect(change).not.toHaveBeenCalled();
    key(input(), 'Enter'); expect(change).toHaveBeenLastCalledWith('custom');
  });
  it('keeps controlled values authoritative and preserves numeric zero', () => {
    render({ value: 0, options: [{ value: 0, label: 'Zero' }, { value: 1, label: 'One' }] });
    act(() => trigger().click()); key(input(), 'ArrowDown'); key(input(), 'ArrowDown'); key(input(), 'Enter');
    expect(change).toHaveBeenLastCalledWith(1); expect(trigger().textContent).toContain('Zero');
  });
  it('supports multi-selection, select-all, custom values and async option hydration', () => {
    render({ multiple: true, defaultValue: ['custom'], options: [], loading: true, showSelectAll: true });
    act(() => trigger().click()); expect(document.querySelector('[role="status"]')?.textContent).toContain('Loading');
    render({ multiple: true, defaultValue: ['custom'], options: [{ value: 'a', label: 'Alpha' }], showSelectAll: true });
    const all = [...document.querySelectorAll('button')].find(button => button.textContent === 'Select all')!;
    act(() => all.click()); expect(change).toHaveBeenLastCalledWith(['custom', 'a']);
    expect(document.querySelector('[role="listbox"]')).not.toBeNull();
  });
  it('closes only the picker on Escape inside a modal and restores trigger focus', () => {
    const close = vi.fn();
    act(() => root.render(<Modal isOpen title="Provider" onClose={close}><Combobox label="Models" /></Modal>));
    const button = document.querySelector<HTMLButtonElement>('button[role="combobox"]')!;
    act(() => button.click()); key(input(), 'Escape');
    expect(close).not.toHaveBeenCalled(); expect(document.activeElement).toBe(button);
  });
  it('commits a typed custom single value on Tab and releases the popup', () => {
    render({ allowCustomValue: true }); act(() => trigger().click()); type('custom'); key(input(), 'Tab');
    expect(change).toHaveBeenLastCalledWith('custom'); expect(document.querySelector('[role="listbox"]')).toBeNull();
  });
  it('removes individual selected tags without nesting buttons or opening the popup', () => {
    render({ multiple: true, defaultValue: ['a', 'c'] });
    const remove = host.querySelector<HTMLButtonElement>('button[aria-label="Clear selection: Alpha"]')!;
    expect(remove.closest('button[role="combobox"]')).toBeNull();
    act(() => remove.click());
    expect(change).toHaveBeenLastCalledWith(['c']);
    expect(document.querySelector('[role="listbox"]')).toBeNull();
  });
  it('connects the public Field label, description, required and error states to the trigger', () => {
    act(() => root.render(<Field label="Models" description="Choose a model" error="Required" required><Combobox /></Field>));
    expect(host.querySelector('label')?.htmlFor).toBe(trigger().id);
    expect(trigger().getAttribute('aria-label')).toBeNull();
    expect(trigger().getAttribute('aria-required')).toBe('true');
    expect(trigger().getAttribute('aria-invalid')).toBe('true');
    const describedBy = trigger().getAttribute('aria-describedby')!.split(' ');
    expect(describedBy.map(id => document.getElementById(id)?.textContent)).toEqual(['Choose a model', 'Required']);
  });
  it('mounts an initially open, filtered picker into its portal', () => {
    render({ defaultOpen: true, defaultSearchValue: 'gam' });
    expect(input().value).toBe('gam');
    expect(document.querySelectorAll('[role="option"]')).toHaveLength(1);
    expect(document.querySelector('[role="option"]')?.textContent).toBe('Gamma');
    expect(document.activeElement).toBe(input());
  });
  it('flips at the bottom edge and repositions after ancestor scrolling', () => {
    let top = window.innerHeight - 48;
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (this: HTMLElement) {
      if (this.dataset.bfComponent === 'combobox-popup') return new DOMRect(0, 0, 240, 180);
      return new DOMRect(40, top, 240, 40);
    });
    render(); act(() => trigger().click());
    const popup = document.querySelector<HTMLElement>('[data-bf-component="combobox-popup"]')!;
    expect(popup.dataset.placement).toBe('top');
    top = 20;
    act(() => host.dispatchEvent(new Event('scroll', { bubbles: true })));
    expect(popup.dataset.placement).toBe('bottom');
    expect(popup.style.top).toBe('64px');
    expect(popup.style.width).toBe('240px');
  });
  it('keeps explicit inline popups in the local component tree', () => {
    render({ dropdownMode: 'inline' }); act(() => trigger().click());
    expect(host.querySelector('[data-bf-component="combobox"] [role="listbox"]')).not.toBeNull();
    expect(host.querySelector<HTMLElement>('[data-bf-component="combobox-popup"]')?.style.position).toBe('');
  });
});
