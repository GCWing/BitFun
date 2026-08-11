// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { GroupChatMemberPicker } from './GroupChatMemberPicker';
import type { GroupChatActor, GroupChatMember } from '../types/flow-chat';

function sampleMembers(): GroupChatMember[] {
  return [
    { sessionId: 'm-owner', role: 'owner', joinedAt: 1, agentType: 'Claw', displayName: 'Owner Claw' },
    { sessionId: 'm-1', role: 'member', joinedAt: 1, agentType: 'Claw', displayName: 'Assistant One' },
  ];
}

const availableAssistants = [
  { sessionId: 'm-1', name: 'Assistant One' },
  { sessionId: 'm-2', name: 'Assistant Two' },
  { sessionId: 'm-3', name: 'Assistant Three' },
];

let container: HTMLDivElement;
let root: Root;

function renderPicker(actor: GroupChatActor, onJoin = vi.fn(), onLeave = vi.fn()) {
  act(() => {
    root.render(
      <GroupChatMemberPicker
        roomId="room-1"
        members={sampleMembers()}
        currentActor={actor}
        availableAssistants={availableAssistants}
        onJoin={onJoin}
        onLeave={onLeave}
      />
    );
  });
  return { onJoin, onLeave };
}

describe('GroupChatMemberPicker', () => {
  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
  });

  it('lists members with display name and role badges', () => {
    renderPicker({ kind: 'master' });
    const rows = Array.from(container.querySelectorAll('[data-bf-part="member"]'));
    expect(rows.length).toBe(2);
    expect(rows[0].textContent).toContain('Owner Claw');
    // role 徽标以 data-bf-state 标记 owner/member（i18n 文本在测试中为字面 key）。
    expect(rows[0].getAttribute('data-bf-state')).toBe('owner');
    expect(rows[1].textContent).toContain('Assistant One');
    expect(rows[1].getAttribute('data-bf-state')).toBe('member');
  });

  it('owner or master can remove a non-owner member', () => {
    const { onLeave } = renderPicker({ kind: 'master' });
    const leaveButtons = Array.from(container.querySelectorAll('[data-bf-part="leaveButton"]'));
    // Owner row 无踢人按钮（role === 'owner' 隐藏），member 行有。
    expect(leaveButtons.length).toBe(1);
    act(() => {
      (leaveButtons[0] as HTMLElement).dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(onLeave).toHaveBeenCalledWith('m-1');
  });

  it('non-owner member has no remove permission (UI hidden)', () => {
    renderPicker({ kind: 'claw', sessionId: 'm-1', agentType: 'Claw' });
    expect(container.querySelector('[data-bf-part="leaveButton"]')).toBeNull();
    expect(container.querySelector('[data-bf-part="addToggle"]')).toBeNull();
  });

  it('add flow lists addable assistants and joins on click', () => {
    const { onJoin } = renderPicker({ kind: 'master' });
    act(() => {
      (container.querySelector('[data-bf-part="addToggle"]') as HTMLElement).dispatchEvent(
        new MouseEvent('click', { bubbles: true }),
      );
    });

    const addItems = Array.from(container.querySelectorAll('[data-bf-part="addItem"]'));
    // m-1 已是成员 → 只列 m-2 / m-3。
    expect(addItems.length).toBe(2);
    expect(addItems[0].textContent).toContain('Assistant Two');

    act(() => {
      (addItems[0] as HTMLElement).dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(onJoin).toHaveBeenCalledWith('m-2');
  });

  it('owner Claw can manage members (role owner)', () => {
    const { onLeave } = renderPicker({ kind: 'claw', sessionId: 'm-owner', agentType: 'Claw' });
    expect(container.querySelector('[data-bf-part="addToggle"]')).toBeTruthy();
    expect(container.querySelector('[data-bf-part="leaveButton"]')).toBeTruthy();
    void onLeave;
  });
});
