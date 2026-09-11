// @vitest-environment jsdom
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { NewProjectDialog } from './NewProjectDialog';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;
const { pickDirectory } = vi.hoisted(() => ({ pickDirectory: vi.fn() }));
vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }));
vi.mock('@/shared/utils/logger', () => ({ createLogger: () => ({ error: vi.fn() }) }));
vi.mock('@/infrastructure/peer-device/pickWorkspaceDirectory', () => ({ pickWorkspaceDirectory: pickDirectory }));

describe('NewProjectDialog composition', () => {
  let root: Root;
  let host: HTMLDivElement;
  const close = vi.fn();
  const button = (label: string) => [...document.querySelectorAll('button')].find((item) => item.textContent === label)!;
  const nameInput = () => document.querySelector<HTMLInputElement>('input:not([readonly])')!;
  const enterName = (value: string) => act(() => {
    const input = nameInput();
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(input, value);
    input.dispatchEvent(new Event('input', { bubbles: true }));
  });

  beforeEach(() => {
    vi.clearAllMocks();
    host = document.createElement('div');
    document.body.append(host);
    root = createRoot(host);
  });
  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it('locks actions while creating and retains the form when creation fails', async () => {
    let rejectCreate!: (reason: Error) => void;
    const pending = new Promise<void>((_resolve, reject) => { rejectCreate = reject; });
    const confirm = vi.fn(() => pending);
    await act(async () => root.render(<NewProjectDialog isOpen defaultParentPath="/srv/workspaces" onClose={close} onConfirm={confirm} />));
    expect(button('newProject.cancel').dataset.openbitfunVariant).toBe('fill');
    expect(button('newProject.create').dataset.openbitfunVariant).toBe('primary');
    enterName(' example-project ');
    await act(async () => { button('newProject.create').click(); });
    expect(confirm).toHaveBeenCalledWith('/srv/workspaces', 'example-project');
    expect(button('newProject.cancel').disabled).toBe(true);
    expect(button('newProject.select').disabled).toBe(true);
    expect(nameInput().disabled).toBe(true);
    act(() => button('newProject.cancel').click());
    expect(close).not.toHaveBeenCalled();
    await act(async () => { rejectCreate(new Error('Directory unavailable')); });
    expect(document.querySelector('[role="alert"]')?.textContent).toContain('Directory unavailable');
    expect(nameInput().value).toBe(' example-project ');
    expect(button('newProject.create').disabled).toBe(false);
    expect(close).not.toHaveBeenCalled();
  });

  it('delegates directory selection to the peer-aware picker and preserves its path', async () => {
    pickDirectory.mockResolvedValue('/srv/remote workspace');
    const confirm = vi.fn(async () => {});
    await act(async () => root.render(<NewProjectDialog isOpen defaultParentPath="/srv" onClose={close} onConfirm={confirm} />));
    await act(async () => { button('newProject.select').click(); });
    expect(pickDirectory).toHaveBeenCalledWith({ title: 'newProject.selectParentDirectory', defaultPath: '/srv' });
    enterName('project');
    expect(document.querySelector('[data-openbitfun-part="preview"]')?.textContent).toContain('/srv/remote workspace/project');
    await act(async () => { button('newProject.create').click(); });
    expect(confirm).toHaveBeenCalledWith('/srv/remote workspace', 'project');
    expect(close).toHaveBeenCalledTimes(1);
  });
});
