// @vitest-environment jsdom
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ConfirmDialogRenderer } from './ConfirmDialogRenderer';
import { confirmDialogChoice, useConfirmDialogStore } from './confirmDialogService';

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

describe('shared confirmation action roles', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    useConfirmDialogStore.setState({ isOpen: false, options: null, resolve: null });
    host = document.createElement('div');
    document.body.append(host);
    root = createRoot(host);
    act(() => root.render(<ConfirmDialogRenderer />));
  });

  afterEach(() => {
    act(() => root.unmount());
    useConfirmDialogStore.getState().cancel();
    host.remove();
  });

  it.each(['cancel', 'secondary', 'confirm'] as const)('keeps %s distinct in a three-choice dialog', async (choice) => {
    let result!: ReturnType<typeof confirmDialogChoice>;
    act(() => {
      result = confirmDialogChoice({ title: 'Run command?', cancelText: 'Cancel', secondaryText: 'Always allow', confirmText: 'Allow once' });
    });
    const buttons = document.querySelectorAll<HTMLButtonElement>('[role="alertdialog"] footer button');
    expect([...buttons].map(button => button.textContent)).toEqual(['Cancel', 'Always allow', 'Allow once']);
    expect([...buttons].map(button => button.getAttribute('data-openbitfun-variant'))).toEqual(['fill', 'outline', 'primary']);
    act(() => buttons[['cancel', 'secondary', 'confirm'].indexOf(choice)].click());
    await expect(result).resolves.toBe(choice);
  });

  it('retains the danger tone when the primary confirmation is destructive', async () => {
    let result!: ReturnType<typeof confirmDialogChoice>;
    act(() => {
      result = confirmDialogChoice({ title: 'Delete?', confirmDanger: true, confirmText: 'Delete' });
    });
    const button = document.querySelector<HTMLButtonElement>('[role="alertdialog"] footer button[data-openbitfun-variant="primary"]')!;
    expect(button.getAttribute('data-openbitfun-tone')).toBe('danger');
    act(() => button.click());
    await expect(result).resolves.toBe('confirm');
  });
});
