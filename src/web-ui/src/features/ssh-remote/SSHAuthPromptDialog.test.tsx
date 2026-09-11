// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot } from 'react-dom/client';
import { expect, it, vi } from 'vitest';
import { SSHAuthPromptDialog } from './SSHAuthPromptDialog';

vi.mock('@/infrastructure/i18n', () => ({ useI18n: () => ({ t: (key: string) => key }) }));
vi.mock('./pickSshPrivateKeyPath', () => ({ pickSshPrivateKeyPath: vi.fn(), pickSshCertificatePath: vi.fn() }));

it('retains the SSH prompt during exit and clears credentials before reopening', () => {
  vi.useFakeTimers();
  const container = document.createElement('div');
  document.body.appendChild(container);
  const root = createRoot(container);
  const render = (open: boolean) => act(() => root.render(
    <SSHAuthPromptDialog open={open} targetDescription={open ? 'user@remote:22' : ''}
      defaultAuthMethod="password" initialUsername={open ? 'user' : ''} lockUsername
      onSubmit={() => undefined} onCancel={() => undefined} />,
  ));
  try {
    render(true);
    const surface = document.querySelector<HTMLElement>('[role="dialog"]')!;
    const password = surface.querySelector<HTMLInputElement>('input[type="password"]')!;
    act(() => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(password, 'test-password');
      password.dispatchEvent(new Event('input', { bubbles: true }));
    });
    render(false);
    expect(surface.dataset.state).toBe('exiting');
    expect(surface.textContent).toContain('user@remote:22');
    expect(password.isConnected).toBe(true);
    act(() => vi.advanceTimersByTime(180));
    expect(surface.isConnected).toBe(false);
    render(true);
    expect(document.querySelector<HTMLInputElement>('input[type="password"]')?.value).toBe('');
  } finally {
    act(() => root.unmount());
    container.remove();
    vi.useRealTimers();
  }
});
