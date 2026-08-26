// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { SubagentAvatar } from './SubagentAvatar';
import { resolveSubagentAvatarPresentation } from './avatarResolver';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

describe('SubagentAvatar', () => {
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

  it('renders the allocated raster avatar and lifecycle state', () => {
    act(() => {
      root.render(
        <SubagentAvatar
          identity={{
            rootSessionId: 'root',
            sessionId: 'child',
            avatarId: 'robot-08',
            nameId: 'name-18',
          }}
          name="Berry"
          size={28}
          status="running"
        />,
      );
    });

    const avatar = container.querySelector('[data-bf-component="subagent-avatar"]');
    expect(avatar?.getAttribute('data-bf-avatar-id')).toBe('robot-08');
    expect(avatar?.getAttribute('data-bf-avatar-color-id')).toBe('cyan');
    expect(avatar?.getAttribute('data-bf-name-id')).toBe('name-18');
    expect(avatar?.getAttribute('data-bf-state')).toBe('running');
    expect(avatar?.getAttribute('style')).toContain('28px');
    expect(container.querySelector('img')?.getAttribute('src')).toContain('robot-08');
  });

  it('renders a stable avatar from the session ID before a name is assigned', () => {
    act(() => {
      root.render(
        <SubagentAvatar
          sessionId="restored-child-session"
          size={22}
          status="completed"
        />,
      );
    });

    const avatar = container.querySelector('[data-bf-component="subagent-avatar"]');
    const presentation = resolveSubagentAvatarPresentation('restored-child-session');
    expect(avatar?.getAttribute('data-bf-avatar-id')).toBe(presentation.avatarId);
    expect(avatar?.getAttribute('data-bf-avatar-color-id')).toBe(presentation.colorId);
    expect(avatar?.getAttribute('style')).toContain(
      `--subagent-avatar-hue-shift: ${presentation.hueShiftDegrees}deg`,
    );
    expect(avatar?.hasAttribute('data-bf-name-id')).toBe(false);
  });
});
