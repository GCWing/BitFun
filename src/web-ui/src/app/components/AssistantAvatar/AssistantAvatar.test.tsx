// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import AssistantAvatar from './AssistantAvatar';
import { resolveAssistantAvatarPreset } from './assistantAvatarPresets';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

describe('AssistantAvatar', () => {
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

  it('renders a selected built-in preset as a stable circular identity', () => {
    act(() => {
      root.render(<AssistantAvatar presetId="orbit-nova" emoji="🧭" size={28} />);
    });

    const avatar = container.querySelector('[data-bf-component="assistant-avatar"]');
    expect(avatar?.getAttribute('data-bf-preset')).toBe('orbit-nova');
    expect(avatar?.getAttribute('data-bf-family')).toBe('orbit');
    expect(avatar?.getAttribute('style')).toContain('28px');
  });

  it('keeps the legacy emoji when a preset id is missing or unknown', () => {
    act(() => {
      root.render(<AssistantAvatar presetId="future-avatar" emoji="🧭" stableKey="assistant-1" />);
    });

    const avatar = container.querySelector('[data-bf-component="assistant-avatar"]');
    expect(avatar?.getAttribute('data-bf-family')).toBe('emoji');
    expect(avatar?.textContent).toContain('🧭');
  });

  it('uses a deterministic built-in fallback when no identity marker exists', () => {
    expect(resolveAssistantAvatarPreset(undefined, 'assistant-1').id)
      .toBe(resolveAssistantAvatarPreset(undefined, 'assistant-1').id);
    expect(resolveAssistantAvatarPreset(undefined, 'assistant-1').id)
      .not.toBe('');
  });
});
