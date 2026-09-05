// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { UpdateInstallProgressModal } from './UpdateInstallProgressModal';
import common from '@/locales/en-US/common.json';

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({ t: (key: string, args?: Record<string, string>) => {
    const value = key.split('.').reduce<unknown>((value, part) => (value as Record<string, unknown>)?.[part], common);
    return String(value ?? key).replace(/\{\{(\w+)\}\}/g, (_, name) => args?.[name] ?? '');
  } }),
}));

globalThis.IS_REACT_ACT_ENVIRONMENT = true;
let root: Root;
let container: HTMLDivElement;
beforeEach(() => {
  vi.stubGlobal('ResizeObserver', class {
    observe() {}
    unobserve() {}
    disconnect() {}
  });
  container = document.createElement('div');
  document.body.append(container);
  root = createRoot(container);
});
afterEach(async () => {
  await act(async () => root.unmount());
  container.remove();
  vi.unstubAllGlobals();
});

const buttons = () => Array.from(document.querySelectorAll('button'));
const button = (text: string) => buttons().find(button => button.textContent?.includes(text))!;

it('shows the downloaded version and interruption notice before allowing installation', async () => {
  const install = vi.fn();
  const defer = vi.fn();
  await act(async () => root.render(<UpdateInstallProgressModal
    isOpen installed version="2.0.0" error={null}
    progress={{ downloaded: 100, total: 100 }} onRestart={install} onCloseInstalled={defer}
  />));
  expect(document.body.textContent).toContain('Version 2.0.0 is downloaded');
  expect(document.body.textContent).toContain('interrupt its active sessions');
  expect(install).not.toHaveBeenCalled();
  await act(async () => button(common.update.restartLater).click());
  expect(defer).toHaveBeenCalledOnce();
  expect(install).not.toHaveBeenCalled();
  await act(async () => button(common.update.installAndRestart).click());
  expect(install).toHaveBeenCalledOnce();
});

it('keeps retry and defer actions available after an install error', async () => {
  const download = vi.fn();
  await act(async () => root.render(<UpdateInstallProgressModal
    isOpen installed version="2.0.0" error="installer unavailable"
    progress={{ downloaded: 100, total: 100 }} onDownloadAgain={download}
  />));
  expect(button(common.update.installAndRestart).disabled).toBe(false);
  expect(button(common.update.restartLater).disabled).toBe(false);
  await act(async () => button(common.update.downloadAgain).click());
  expect(download).toHaveBeenCalledOnce();
});

it('prevents dismissing or submitting the dialog again while installation is in progress', async () => {
  const defer = vi.fn();
  const install = vi.fn();
  await act(async () => root.render(<UpdateInstallProgressModal
    isOpen installed installing version="2.0.0" error={null}
    progress={{ downloaded: 100, total: 100 }} onRestart={install} onCloseInstalled={defer}
  />));
  expect(button(common.update.restartLater).disabled).toBe(true);
  expect(button(common.update.installing).disabled).toBe(true);
  await act(async () => {
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    button(common.update.installing).click();
  });
  expect(defer).not.toHaveBeenCalled();
  expect(install).not.toHaveBeenCalled();
});
