import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
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
      container.querySelector('.user-message-edit-composer__icon-button--confirm')?.dispatchEvent(
        new dom.window.MouseEvent('click', { bubbles: true }),
      );
    });

    expect(onSubmit).toHaveBeenCalledWith({
      version: 1,
      segments: [{ kind: 'text', text: ' Continue the investigation.' }],
    });
  });
});
