// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { GroupChatCreateDialog } from './GroupChatCreateDialog';
import { useGroupChatStore } from '../../../../../flow_chat/store/groupChatStore';

vi.mock('@/infrastructure/api/service-api/ApiClient', () => ({
  api: { invoke: vi.fn() },
}));

import { api } from '@/infrastructure/api/service-api/ApiClient';
const mockedInvoke = vi.mocked(api.invoke);

const assistants = [
  { sessionId: 'm-1', name: 'Assistant One' },
  { sessionId: 'm-2', name: 'Assistant Two' },
];

let container: HTMLDivElement;
let root: Root;

function renderDialog(onClose = vi.fn()) {
  act(() => {
    root.render(
      <GroupChatCreateDialog workspacePath="/ws" availableAssistants={assistants} onClose={onClose} />
    );
  });
  return onClose;
}

describe('GroupChatCreateDialog', () => {
  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    mockedInvoke.mockReset();
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
  });

  it('renders the create dialog with name input and member options', () => {
    renderDialog();
    expect(container.querySelector('[data-bf-part="nameInput"]')).toBeTruthy();
    const options = Array.from(container.querySelectorAll('[data-bf-part="memberOption"]'));
    expect(options.length).toBe(2);
  });

  it('create button is disabled until name and members are provided', () => {
    renderDialog();
    const create = container.querySelector('[data-bf-part="create"]') as HTMLButtonElement;
    expect(create.disabled).toBe(true);
  });

  it('creates a room via createRoom (group_chat_create) with free mode', async () => {
    const onClose = renderDialog();
    mockedInvoke.mockResolvedValue({
      schemaVersion: 1,
      roomId: 'room-new',
      name: 'My Group',
      owner: { kind: 'master' },
      mode: 'free',
      roundRobinCursor: 0,
      createdAt: 1,
      lastActiveAt: 1,
      status: 'active',
      memberLimit: 50,
    });

    // 输入群名（受控 input 用 native setter）。
    const nameInput = container.querySelector('[data-bf-part="nameInput"]') as HTMLInputElement;
    const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value')?.set;
    act(() => {
      setter?.call(nameInput, 'My Group');
      nameInput.dispatchEvent(new Event('input', { bubbles: true }));
    });

    // 勾选成员。
    const option = container.querySelector('[data-bf-part="memberOption"] input') as HTMLInputElement;
    act(() => {
      option.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    const create = container.querySelector('[data-bf-part="create"]') as HTMLButtonElement;
    expect(create.disabled).toBe(false);

    await act(async () => {
      create.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    const createCall = mockedInvoke.mock.calls.find(([command]) => command === 'group_chat_create');
    expect(createCall).toBeTruthy();
    expect(createCall?.[1]).toEqual(
      expect.objectContaining({ name: 'My Group', mode: 'free' }),
    );
    expect(onClose).toHaveBeenCalled();
  });
});
