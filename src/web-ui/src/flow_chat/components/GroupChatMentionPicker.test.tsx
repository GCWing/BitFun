// @vitest-environment jsdom
import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import GroupChatMentionPicker, { GROUP_CHAT_ALL_ITEM } from './GroupChatMentionPicker';
import type { GroupChatActor, GroupChatMember } from '../types/flow-chat';

let container: HTMLDivElement;
let root: Root;

function members(): GroupChatMember[] {
  return [
    {
      sessionId: 'm-1',
      role: 'owner',
      joinedAt: 1,
      agentType: 'Claw',
      displayName: 'Assistant One',
    },
    {
      sessionId: 'm-2',
      role: 'member',
      joinedAt: 1,
      agentType: 'Claw',
      displayName: 'Assistant Two',
    },
  ];
}

function renderPicker(props: {
  isOpen: boolean;
  searchQuery: string;
  onSelect?: (target: GroupChatActor) => void;
  onClose?: () => void;
}) {
  act(() => {
    root.render(
      <GroupChatMentionPicker
        isOpen={props.isOpen}
        searchQuery={props.searchQuery}
        members={members()}
        onSelect={props.onSelect ?? (() => {})}
        onClose={props.onClose ?? (() => {})}
      />
    );
  });
}

function clickItem(label: string) {
  const items = Array.from(container.querySelectorAll('[data-bf-part="item"]'));
  const target = items.find((item) => item.textContent?.includes(label));
  expect(target).toBeTruthy();
  act(() => {
    (target as HTMLElement).dispatchEvent(
      new MouseEvent('click', { bubbles: true }),
    );
  });
}

describe('GroupChatMentionPicker', () => {
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

  it('returns null when closed', () => {
    renderPicker({ isOpen: false, searchQuery: '' });
    expect(container.querySelector('[data-bf-component="group-chat-mention-picker"]')).toBeNull();
  });

  it('lists members plus the @all fixed item at the top', () => {
    renderPicker({ isOpen: true, searchQuery: '' });

    const items = Array.from(container.querySelectorAll('[data-bf-part="item"]'));
    expect(items.length).toBe(3);
    expect(items[0].textContent).toContain(GROUP_CHAT_ALL_ITEM);
    expect(items[1].textContent).toContain('Assistant One');
    expect(items[2].textContent).toContain('Assistant Two');
  });

  it('selecting a member returns {kind:"claw", sessionId}', () => {
    const onSelect = vi.fn();
    renderPicker({ isOpen: true, searchQuery: '', onSelect });

    clickItem('Assistant One');

    expect(onSelect).toHaveBeenCalledWith({
      kind: 'claw',
      sessionId: 'm-1',
      agentType: 'Claw',
    });
  });

  it('selecting @all returns {kind:"all"} explicitly (P1-3)', () => {
    const onSelect = vi.fn();
    renderPicker({ isOpen: true, searchQuery: '', onSelect });

    clickItem(GROUP_CHAT_ALL_ITEM);

    expect(onSelect).toHaveBeenCalledWith({ kind: 'all' });
  });

  it('filters members by search query', () => {
    renderPicker({ isOpen: true, searchQuery: '@two' });

    const items = Array.from(container.querySelectorAll('[data-bf-part="item"]'));
    // @all 固定项 + 过滤后的 Assistant Two
    expect(items.length).toBe(2);
    expect(items[0].textContent).toContain(GROUP_CHAT_ALL_ITEM);
    expect(items[1].textContent).toContain('Assistant Two');
  });

  it('keyboard navigation selects with Enter', () => {
    const onSelect = vi.fn();
    renderPicker({ isOpen: true, searchQuery: '', onSelect });

    // 默认选中 @all（index 0）；ArrowDown → 第一个成员
    act(() => {
      document.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    });
    act(() => {
      document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    });

    expect(onSelect).toHaveBeenCalledWith({
      kind: 'claw',
      sessionId: 'm-1',
      agentType: 'Claw',
    });
  });

  it('Escape closes the picker', () => {
    const onClose = vi.fn();
    renderPicker({ isOpen: true, searchQuery: '', onClose });

    act(() => {
      document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    });

    expect(onClose).toHaveBeenCalled();
  });
});
