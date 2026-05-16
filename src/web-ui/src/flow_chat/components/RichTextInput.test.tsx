import React, { act, createRef, forwardRef, useImperativeHandle, useState } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import RichTextInput from './RichTextInput';
import type { ContextItem } from '../../shared/types/context';

type HarnessHandle = {
  setValue: (value: string) => void;
};

const emptyContexts: ContextItem[] = [];

let JSDOMCtor: (new (
  html?: string,
  options?: { pretendToBeVisual?: boolean }
) => { window: Window & typeof globalThis }) | null = null;

try {
  const jsdom = await import('jsdom');
  JSDOMCtor = jsdom.JSDOM as typeof JSDOMCtor;
} catch {
  JSDOMCtor = null;
}

const ControlledHarness = forwardRef<HarnessHandle>(function ControlledHarness(_, ref) {
  const [value, setValue] = useState('hello');

  useImperativeHandle(ref, () => ({
    setValue,
  }), []);

  return (
    <RichTextInput
      value={value}
      onChange={(nextValue) => setValue(nextValue)}
      contexts={emptyContexts}
      onRemoveContext={() => {}}
    />
  );
});

const describeWithJsdom = JSDOMCtor ? describe : describe.skip;

describeWithJsdom('RichTextInput external sync', () => {
  let dom: { window: Window & typeof globalThis };
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    dom = new JSDOMCtor!('<!doctype html><html><body></body></html>', {
      pretendToBeVisual: true,
    });

    const { window } = dom;
    vi.stubGlobal('window', window);
    vi.stubGlobal('document', window.document);
    vi.stubGlobal('navigator', window.navigator);
    vi.stubGlobal('Node', window.Node);
    vi.stubGlobal('Text', window.Text);
    vi.stubGlobal('Element', window.Element);
    vi.stubGlobal('HTMLElement', window.HTMLElement);
    vi.stubGlobal('HTMLDivElement', window.HTMLDivElement);
    vi.stubGlobal('HTMLSpanElement', window.HTMLSpanElement);
    vi.stubGlobal('DocumentFragment', window.DocumentFragment);
    vi.stubGlobal('Range', window.Range);
    vi.stubGlobal('Selection', window.Selection);
    vi.stubGlobal('NodeFilter', window.NodeFilter);
    vi.stubGlobal('Event', window.Event);
    vi.stubGlobal('InputEvent', window.InputEvent);
    vi.stubGlobal('getSelection', window.getSelection.bind(window));
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);

    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    });
    vi.stubGlobal('cancelAnimationFrame', () => {});
    window.requestAnimationFrame = globalThis.requestAnimationFrame;
    window.cancelAnimationFrame = globalThis.cancelAnimationFrame;

    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    dom.window.close();
    vi.unstubAllGlobals();
  });

  async function renderHarness(ref: React.RefObject<HarnessHandle>) {
    await act(async () => {
      root.render(<ControlledHarness ref={ref} />);
    });

    const editor = container.querySelector('.rich-text-input');
    expect(editor).toBeInstanceOf(HTMLDivElement);
    return editor as HTMLDivElement;
  }

  it('keeps the existing DOM node when parent echoes local input', async () => {
    const harnessRef = createRef<HarnessHandle>();
    const editor = await renderHarness(harnessRef);

    expect(editor.textContent).toBe('hello');
    const originalTextNode = editor.firstChild;
    expect(originalTextNode).toBeInstanceOf(Text);

    await act(async () => {
      (originalTextNode as Text).textContent = 'hello!';
      editor.dispatchEvent(new window.Event('input', { bubbles: true }));
    });

    expect(editor.textContent).toBe('hello!');
    expect(editor.firstChild).toBe(originalTextNode);
  });

  it('replaces the DOM node when value changes externally', async () => {
    const harnessRef = createRef<HarnessHandle>();
    const editor = await renderHarness(harnessRef);

    const originalTextNode = editor.firstChild;
    expect(originalTextNode).toBeInstanceOf(Text);

    await act(async () => {
      harnessRef.current?.setValue('server rewrite');
    });

    expect(editor.textContent).toBe('server rewrite');
    expect(editor.firstChild).not.toBe(originalTextNode);
  });

  it('keeps Escape owned by IME composition', async () => {
    const onKeyDown = vi.fn();

    await act(async () => {
      root.render(
        <RichTextInput
          value=""
          onChange={() => {}}
          onKeyDown={onKeyDown}
          contexts={emptyContexts}
          onRemoveContext={() => {}}
        />
      );
    });

    const editor = container.querySelector('.rich-text-input');
    expect(editor).toBeInstanceOf(HTMLDivElement);

    await act(async () => {
      editor!.dispatchEvent(new window.KeyboardEvent('keydown', {
        key: 'Escape',
        keyCode: 229,
        bubbles: true,
      }));
    });

    expect(onKeyDown).not.toHaveBeenCalled();
  });
});
