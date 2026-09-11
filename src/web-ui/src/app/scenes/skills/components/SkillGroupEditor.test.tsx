// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SkillGroupEditor } from './SkillGroupEditor';

vi.mock('@/infrastructure/i18n/hooks/useI18n', () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));

describe('SkillGroupEditor exit', () => {
  let root: Root;
  let container: HTMLDivElement;

  beforeEach(() => {
    vi.useFakeTimers();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.useRealTimers();
  });

  it.each(['close', 'save'])('finishes the exit before the parent removes the draft after %s', async (action) => {
    const onClose = vi.fn();
    const onSave = vi.fn().mockResolvedValue(undefined);
    await act(async () => root.render(
      <SkillGroupEditor
        draft={{ group: { id: 'example', name: 'Example', skillKeys: [] }, original: null }}
        skills={[]} catalogReady saving={false}
        onClose={onClose} onSave={onSave} onCopy={() => undefined}
      />,
    ));
    const surface = document.querySelector<HTMLElement>('[data-testid="skill-group-editor"]')!;
    await act(async () => {
      if (action === 'save') surface.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
      else surface.querySelector<HTMLButtonElement>('[data-openbitfun-part="close"]')!.click();
    });
    expect(onSave).toHaveBeenCalledTimes(action === 'save' ? 1 : 0);
    expect(onClose).not.toHaveBeenCalled();
    expect(surface.dataset.state).toBe('exiting');
    expect(surface.querySelector('input')?.value).toBe('Example');
    act(() => vi.advanceTimersByTime(180));
    expect(surface.isConnected).toBe(false);
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
