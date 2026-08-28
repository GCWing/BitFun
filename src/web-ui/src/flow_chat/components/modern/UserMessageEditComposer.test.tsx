import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { Simulate } from 'react-dom/test-utils';
import { JSDOM } from 'jsdom';
import { UserMessageEditComposer } from './UserMessageEditComposer';
import type { ComposerPresentation } from '../../utils/composerPresentation';

vi.mock('../FileMentionPicker', () => ({
  FileMentionPicker: () => null,
}));

const presentation: ComposerPresentation = {
  version: 1,
  segments: [
    {
      kind: 'context',
      context: {
        id: 'session-reference-1',
        type: 'session-reference',
        sessionId: 'session-1',
        sessionName: 'Delete all files',
        workspacePath: '/workspace',
        workspaceLabel: 'Workspace',
        timestamp: 1,
      },
      tag: '[session: Delete all files]',
      label: 'Delete all files',
      title: 'Workspace - /workspace',
    },
    { kind: 'text', text: ' Continue the investigation.' },
  ],
};

describe('UserMessageEditComposer', () => {
  let dom: JSDOM;
  let container: HTMLDivElement;
  let root: Root;

  const renderComposer = async (options: { rich?: boolean } = {}) => {
    const onChange = vi.fn();
    const onSubmit = vi.fn();
    const onCancel = vi.fn();

    await act(async () => {
      root.render(
        <UserMessageEditComposer
          value={options.rich
            ? '[session: Delete all files] Continue the investigation.'
            : 'Continue the investigation.'}
          submitLabel="Save"
          cancelLabel="Cancel"
          onChange={onChange}
          onSubmit={onSubmit}
          onCancel={onCancel}
          presentation={options.rich ? presentation : undefined}
        />,
      );
    });

    return { onChange, onSubmit, onCancel };
  };

  const dispatchKey = async (
    target: Element,
    key: string,
    init: KeyboardEventInit = {},
  ) => {
    const event = new dom.window.KeyboardEvent('keydown', {
      key,
      bubbles: true,
      cancelable: true,
      ...init,
    });
    await act(async () => {
      target.dispatchEvent(event);
    });
    return event;
  };

  beforeEach(() => {
    dom = new JSDOM('<!doctype html><html><body><div id="root"></div></body></html>', {
      pretendToBeVisual: true,
    });
    vi.stubGlobal('window', dom.window);
    vi.stubGlobal('document', dom.window.document);
    vi.stubGlobal('Node', dom.window.Node);
    vi.stubGlobal('HTMLElement', dom.window.HTMLElement);
    vi.stubGlobal('HTMLDivElement', dom.window.HTMLDivElement);
    vi.stubGlobal('HTMLSpanElement', dom.window.HTMLSpanElement);
    vi.stubGlobal('DocumentFragment', dom.window.DocumentFragment);
    vi.stubGlobal('Range', dom.window.Range);
    vi.stubGlobal('Selection', dom.window.Selection);
    vi.stubGlobal('NodeFilter', dom.window.NodeFilter);
    vi.stubGlobal('Event', dom.window.Event);
    vi.stubGlobal('InputEvent', dom.window.InputEvent);
    vi.stubGlobal('getSelection', dom.window.getSelection.bind(dom.window));
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    });
    vi.stubGlobal('cancelAnimationFrame', () => {});
    dom.window.requestAnimationFrame = globalThis.requestAnimationFrame;
    dom.window.cancelAnimationFrame = globalThis.cancelAnimationFrame;

    container = dom.window.document.getElementById('root') as HTMLDivElement;
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    dom.window.close();
    vi.unstubAllGlobals();
  });

  it('keeps Enter with the IME during tracked composition and submits afterward', async () => {
    const { onSubmit } = await renderComposer();
    const textarea = container.querySelector('textarea');
    expect(textarea).toBeTruthy();

    await act(async () => {
      Simulate.compositionStart(textarea!);
    });
    const imeEnter = await dispatchKey(textarea!, 'Enter');

    expect(imeEnter.defaultPrevented).toBe(false);
    expect(onSubmit).not.toHaveBeenCalled();

    await act(async () => {
      Simulate.compositionEnd(textarea!);
    });
    const submitEnter = await dispatchKey(textarea!, 'Enter');

    expect(submitEnter.defaultPrevented).toBe(true);
    expect(onSubmit).toHaveBeenCalledTimes(1);
  });

  it.each([
    ['native isComposing', { isComposing: true }],
    ['native keyCode 229', { keyCode: 229 }],
  ] as const)('keeps Enter with the IME for %s', async (_label, init) => {
    const { onSubmit } = await renderComposer();
    const textarea = container.querySelector('textarea');
    expect(textarea).toBeTruthy();

    const event = await dispatchKey(textarea!, 'Enter', init);

    expect(event.defaultPrevented).toBe(false);
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it('keeps Escape with the IME during tracked composition and cancels afterward', async () => {
    const { onCancel } = await renderComposer();
    const textarea = container.querySelector('textarea');
    expect(textarea).toBeTruthy();

    await act(async () => {
      Simulate.compositionStart(textarea!);
    });
    const imeEscape = await dispatchKey(textarea!, 'Escape');

    expect(imeEscape.defaultPrevented).toBe(false);
    expect(onCancel).not.toHaveBeenCalled();

    await act(async () => {
      Simulate.compositionEnd(textarea!);
    });
    const cancelEscape = await dispatchKey(textarea!, 'Escape');

    expect(cancelEscape.defaultPrevented).toBe(true);
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it.each([
    ['native isComposing', { isComposing: true }],
    ['native keyCode 229', { keyCode: 229 }],
  ] as const)('keeps Escape with the IME for %s', async (_label, init) => {
    const { onCancel } = await renderComposer();
    const textarea = container.querySelector('textarea');
    expect(textarea).toBeTruthy();

    const event = await dispatchKey(textarea!, 'Escape', init);

    expect(event.defaultPrevented).toBe(false);
    expect(onCancel).not.toHaveBeenCalled();
  });

  it('keeps rich editor Enter and Escape handling inside its IME boundary', async () => {
    const { onSubmit, onCancel } = await renderComposer({ rich: true });
    const editor = container.querySelector('.rich-text-input');
    expect(editor).toBeTruthy();

    const enter = await dispatchKey(editor!, 'Enter', { keyCode: 229 });
    const escape = await dispatchKey(editor!, 'Escape', { keyCode: 229 });

    expect(enter.defaultPrevented).toBe(false);
    expect(escape.defaultPrevented).toBe(false);
    expect(onSubmit).not.toHaveBeenCalled();
    expect(onCancel).not.toHaveBeenCalled();
  });

  it('restores and removes reference capsules atomically', async () => {
    const onChange = vi.fn();
    const onSubmit = vi.fn();

    await act(async () => {
      root.render(
        <UserMessageEditComposer
          value="[session: Delete all files] Continue the investigation."
          submitLabel="Save"
          cancelLabel="Cancel"
          onChange={onChange}
          onSubmit={onSubmit}
          onCancel={() => {}}
          presentation={presentation}
        />,
      );
    });

    const editor = container.querySelector('.rich-text-input') as HTMLDivElement | null;
    const capsule = container.querySelector('[data-context-id="session-reference-1"]');
    expect(editor).toBeTruthy();
    expect(capsule).toBeTruthy();
    expect(editor?.textContent).not.toContain('[session:');

    await act(async () => {
      capsule?.querySelector('button')?.dispatchEvent(
        new dom.window.MouseEvent('click', { bubbles: true }),
      );
    });

    expect(container.querySelector('[data-context-id="session-reference-1"]')).toBeNull();
    expect(onChange).toHaveBeenLastCalledWith('Continue the investigation.');

    await act(async () => {
      container.querySelector('button[aria-label="Save"]')?.dispatchEvent(
        new dom.window.MouseEvent('click', { bubbles: true }),
      );
    });

    expect(onSubmit).toHaveBeenCalledWith({
      version: 1,
      segments: [{ kind: 'text', text: ' Continue the investigation.' }],
    });
  });
});
