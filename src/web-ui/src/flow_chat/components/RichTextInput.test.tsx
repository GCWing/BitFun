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
    (window.document as Document & { execCommand?: typeof document.execCommand }).execCommand = (
      command,
      _showUi,
      value,
    ) => {
      if (command !== 'insertText') {
        return false;
      }

      const selection = window.getSelection();
      if (!selection || selection.rangeCount === 0) {
        return false;
      }

      const range = selection.getRangeAt(0);
      range.deleteContents();
      const textNode = document.createTextNode(String(value ?? ''));
      range.insertNode(textNode);

      const nextRange = document.createRange();
      nextRange.setStart(textNode, textNode.textContent?.length ?? 0);
      nextRange.collapse(true);
      selection.removeAllRanges();
      selection.addRange(nextRange);
      return true;
    };

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

  function setCaret(editor: HTMLDivElement, offset: number) {
    const selection = window.getSelection();
    const range = document.createRange();
    const textNode = (editor.firstChild as Text | null) ?? document.createTextNode('');

    if (!editor.firstChild) {
      editor.appendChild(textNode);
    }

    range.setStart(textNode, offset);
    range.collapse(true);
    selection?.removeAllRanges();
    selection?.addRange(range);
  }

  async function updateEditorText(editor: HTMLDivElement, text: string, offset = text.length) {
    await act(async () => {
      editor.textContent = text;
      setCaret(editor, offset);
      editor.dispatchEvent(new window.Event('input', { bubbles: true }));
    });
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

  it('opens file mention only at the start or after whitespace', async () => {
    const onMentionStateChange = vi.fn();

    await act(async () => {
      root.render(
        <RichTextInput
          value=""
          onChange={() => {}}
          onMentionStateChange={onMentionStateChange}
          contexts={emptyContexts}
          onRemoveContext={() => {}}
        />
      );
    });

    const editor = container.querySelector('.rich-text-input');
    expect(editor).toBeInstanceOf(HTMLDivElement);

    await updateEditorText(editor as HTMLDivElement, 'email@test');
    expect(onMentionStateChange).not.toHaveBeenCalled();

    await updateEditorText(editor as HTMLDivElement, 'ask @test');
    expect(onMentionStateChange).toHaveBeenLastCalledWith({
      isActive: true,
      query: 'test',
      startOffset: 4,
    });

    await updateEditorText(editor as HTMLDivElement, '@root');
    expect(onMentionStateChange).toHaveBeenLastCalledWith({
      isActive: true,
      query: 'root',
      startOffset: 0,
    });
  });

  it('inserts a separating space when opening mention from a mid-word caret', async () => {
    const onMentionStateChange = vi.fn();

    await act(async () => {
      root.render(
        <RichTextInput
          value="hello"
          onChange={() => {}}
          onMentionStateChange={onMentionStateChange}
          contexts={emptyContexts}
          onRemoveContext={() => {}}
        />
      );
    });

    const editor = container.querySelector('.rich-text-input');
    expect(editor).toBeInstanceOf(HTMLDivElement);

    setCaret(editor as HTMLDivElement, 'hello'.length);

    await act(async () => {
      ((editor as HTMLDivElement) as HTMLDivElement & { openMention?: () => void }).openMention?.();
    });

    expect(editor?.textContent).toBe('hello @');
    expect(onMentionStateChange).toHaveBeenLastCalledWith({
      isActive: true,
      query: '',
      startOffset: 6,
    });
  });
});
