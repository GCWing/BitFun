// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ConfigActionBar } from './ConfigActionBar';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

describe('ConfigActionBar', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('exposes one status and explicit save/discard actions', () => {
    const onSave = vi.fn();
    const onDiscard = vi.fn();
    act(() => root.render(
      <ConfigActionBar
        status="unsaved"
        onSave={onSave}
        onDiscard={onDiscard}
      />,
    ));

    expect(container.querySelector('[data-openbitfun-part="fieldStatus"]')?.textContent).toBe('changeStatus.unsaved');
    const buttons = container.querySelectorAll('button');
    expect(buttons).toHaveLength(2);
    act(() => (buttons[0] as HTMLButtonElement).click());
    act(() => (buttons[1] as HTMLButtonElement).click());
    expect(onDiscard).toHaveBeenCalledOnce();
    expect(onSave).toHaveBeenCalledOnce();
  });

  it('locks both actions while a save is in flight', () => {
    act(() => root.render(
      <ConfigActionBar
        status="saving"
        saving
        onSave={vi.fn()}
        onDiscard={vi.fn()}
      />,
    ));

    const buttons = container.querySelectorAll<HTMLButtonElement>('button');
    expect(buttons[0].disabled).toBe(true);
    expect(buttons[1].disabled).toBe(true);
  });

  it('stays out of the page when there is no pending change or status message', () => {
    act(() => root.render(
      <ConfigActionBar
        status="saved"
        onSave={vi.fn()}
        onDiscard={vi.fn()}
      />,
    ));

    expect(container.querySelector('[data-openbitfun-part="actionBar"]')).toBeNull();
  });

  it('keeps an explicit saved message visible when a page provides one', () => {
    act(() => root.render(
      <ConfigActionBar
        status="saved"
        statusMessage="Saved just now"
        onSave={vi.fn()}
        onDiscard={vi.fn()}
      />,
    ));

    expect(container.querySelector('[data-openbitfun-part="actionBar"]')).not.toBeNull();
    expect(container.textContent).toContain('Saved just now');
    expect(container.querySelectorAll('button')).toHaveLength(0);
  });
});
