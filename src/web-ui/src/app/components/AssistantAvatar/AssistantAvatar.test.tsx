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

  it('maps a retired SVG preset id to the new Claw artwork', () => {
    act(() => {
      root.render(<AssistantAvatar presetId="orbit-nova" emoji="🧭" size={28} />);
    });

    const avatar = container.querySelector('[data-bf-component="assistant-avatar"]');
    const image = container.querySelector('.assistant-avatar__image') as HTMLImageElement;
    expect(avatar?.getAttribute('data-bf-preset')).toBe('claw-orbit');
    expect(avatar?.getAttribute('data-bf-family')).toBe('clawOrbit');
    expect(avatar?.getAttribute('style')).toContain('28px');
    expect(image.getAttribute('src')).toBe('/assets/assistant/claw-avatar-alt.webp');
  });

  it('keeps the legacy emoji when a preset id is missing or unknown', () => {
    act(() => {
      root.render(<AssistantAvatar presetId="future-avatar" emoji="🧭" stableKey="assistant-1" />);
    });

    const avatar = container.querySelector('[data-bf-component="assistant-avatar"]');
    expect(avatar?.getAttribute('data-bf-family')).toBe('emoji');
    expect(avatar?.textContent).toContain('🧭');
  });

  it('uses a deterministic new Claw fallback when no identity marker exists', () => {
    expect(resolveAssistantAvatarPreset(undefined, 'assistant-1').id)
      .toBe(resolveAssistantAvatarPreset(undefined, 'assistant-1').id);
    expect(resolveAssistantAvatarPreset(undefined).id).toBe('claw');
  });

  it('renders the new default avatar without discarding the accessible identity', () => {
    act(() => {
      root.render(
        <AssistantAvatar
          name="Claw"
          decorative={false}
        />,
      );
    });

    const avatar = container.querySelector('[data-bf-component="assistant-avatar"]');
    const image = container.querySelector('.assistant-avatar__image') as HTMLImageElement;
    expect(avatar?.getAttribute('data-bf-family')).toBe('claw');
    expect(avatar?.getAttribute('data-bf-preset')).toBe('claw');
    expect(avatar?.getAttribute('aria-label')).toBe('Claw avatar');
    expect(image.getAttribute('src')).toBe('/assets/assistant/claw-avatar.webp');
  });
});
