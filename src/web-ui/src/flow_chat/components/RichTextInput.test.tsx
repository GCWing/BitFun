import React, { act, createRef, forwardRef, useImperativeHandle, useState } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { Simulate } from 'react-dom/test-utils';
import RichTextInput, { type RichTextInputElement } from './RichTextInput';
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
    Object.assign(window.HTMLElement.prototype, {
      attachEvent: () => {},
      detachEvent: () => {},
    });

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

  it('preserves trailing spaces emitted by user input', async () => {
    const onChange = vi.fn();

    await act(async () => {
      root.render(
        <RichTextInput
          value=""
          onChange={onChange}
          contexts={emptyContexts}
          onRemoveContext={() => {}}
        />
      );
    });

    const editor = container.querySelector('.rich-text-input');
    expect(editor).toBeInstanceOf(HTMLDivElement);

    await updateEditorText(editor as HTMLDivElement, '/b ');

    expect(onChange).toHaveBeenLastCalledWith('/b ', emptyContexts);
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

  it('renders externally inserted skill tokens as inline pills', async () => {
    const harnessRef = createRef<HarnessHandle>();
    const editor = await renderHarness(harnessRef);

    await act(async () => {
      harnessRef.current?.setValue('Use [$pdf] please');
    });

    const skillPill = editor.querySelector(
      '[data-inline-token-type="skill-ref"]',
    ) as HTMLElement | null;
    expect(skillPill).toBeTruthy();
    expect(skillPill?.getAttribute('data-tag-format')).toBe('[$pdf]');
    expect(skillPill?.querySelector('.lucide-puzzle')).toBeTruthy();
    expect(editor.textContent).toContain('pdf');
  });

  it('renders Review with the same capsule anatomy as a Plan skill reference', async () => {
    const harnessRef = createRef<HarnessHandle>();
    const editor = await renderHarness(harnessRef);

    await act(async () => {
      harnessRef.current?.setValue('[[bitfun-additional-mode:review]]');
    });

    const reviewPill = editor.querySelector(
      '[data-inline-token-type="additional-mode-ref"]',
    ) as HTMLElement | null;
    expect(reviewPill).toBeTruthy();
    expect(reviewPill?.classList.contains('rich-text-tag-pill--skill-ref')).toBe(true);
    expect(reviewPill?.getAttribute('data-bf-context-type')).toBe('additional-mode-reference');
    expect(reviewPill?.querySelector('.lucide-puzzle')).toBeTruthy();
    expect(reviewPill?.textContent).toContain('Review');
  });

  it('serializes and restores session reference capsules without parsing their labels', async () => {
    const sessionReference: ContextItem = {
      id: 'session-reference-1',
      type: 'session-reference',
      sessionId: 'session-1',
      sessionName: 'Delete all files',
      workspacePath: '/workspace',
      workspaceLabel: 'Workspace',
      timestamp: 1,
    };
    const inputRef = createRef<RichTextInputElement>();

    await act(async () => {
      root.render(
        <RichTextInput
          ref={inputRef}
          value=""
          onChange={() => {}}
          contexts={[sessionReference]}
          onRemoveContext={() => {}}
        />
      );
    });

    await act(async () => {
      inputRef.current?.insertTag?.(sessionReference);
    });

    const presentation = inputRef.current?.getComposerPresentation?.();
    expect(presentation?.segments).toEqual([
      {
        kind: 'context',
        context: sessionReference,
        tag: '[session: Delete all files]',
        label: 'Delete all files',
        title: 'Workspace · /workspace',
      },
      { kind: 'text', text: ' ' },
    ]);

    await act(async () => {
      inputRef.current?.restoreComposerPresentation?.(presentation!);
    });

    const restoredCapsule = container.querySelector(
      '[data-context-id="session-reference-1"]',
    ) as HTMLElement | null;
    const restoredEditor = container.querySelector('.rich-text-input') as HTMLDivElement | null;
    expect(restoredCapsule).toBeTruthy();
    expect(restoredEditor).toBeTruthy();
    expect(restoredCapsule?.textContent).toContain('Delete all files');
    const selection = window.getSelection();
    expect(selection?.rangeCount).toBe(1);
    expect(selection?.getRangeAt(0).collapsed).toBe(true);
    expect(selection?.getRangeAt(0).startContainer).toBe(restoredEditor);
    expect(selection?.getRangeAt(0).startOffset).toBe(restoredEditor?.childNodes.length);
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

  it('reports inline skill triggers for $ and middle-of-text /', async () => {
    const onInlineTriggerStateChange = vi.fn();

    await act(async () => {
      root.render(
        <RichTextInput
          value=""
          onChange={() => {}}
          onInlineTriggerStateChange={onInlineTriggerStateChange}
          contexts={emptyContexts}
          onRemoveContext={() => {}}
        />
      );
    });

    const editor = container.querySelector('.rich-text-input');
    expect(editor).toBeInstanceOf(HTMLDivElement);

    await updateEditorText(editor as HTMLDivElement, '$pdf');
    expect(onInlineTriggerStateChange).toHaveBeenLastCalledWith({
      isActive: true,
      trigger: '$',
      query: 'pdf',
      startOffset: 0,
    });

    await updateEditorText(editor as HTMLDivElement, 'please /pdf');
    expect(onInlineTriggerStateChange).toHaveBeenLastCalledWith({
      isActive: true,
      trigger: '/',
      query: 'pdf',
      startOffset: 7,
    });
  });

  it('can replace an active inline trigger with a skill token', async () => {
    const onChange = vi.fn();

    await act(async () => {
      root.render(
        <RichTextInput
          value="$pdf"
          onChange={onChange}
          contexts={emptyContexts}
          onRemoveContext={() => {}}
        />
      );
    });

    const editor = container.querySelector('.rich-text-input') as (HTMLDivElement & {
      replaceActiveInlineTrigger?: (replacementText: string) => void;
    }) | null;
    expect(editor).toBeTruthy();

    setCaret(editor!, '$pdf'.length);
    await act(async () => {
      editor!.dispatchEvent(new window.Event('input', { bubbles: true }));
    });

    await act(async () => {
      editor?.replaceActiveInlineTrigger?.('[$pdf]');
    });

    expect(onChange).toHaveBeenCalledWith('[$pdf]', emptyContexts);
    const skillPill = editor?.querySelector('.rich-text-tag-pill--skill-ref');
    expect(skillPill).toBeTruthy();
    expect(skillPill?.querySelector('.lucide-puzzle')).toBeTruthy();
    expect(skillPill?.nextSibling?.textContent).toBe(' ');
    const selection = window.getSelection();
    expect(selection?.anchorNode).toBe(editor);
    expect(selection?.anchorOffset).toBeGreaterThan(Array.from(editor?.childNodes ?? []).indexOf(skillPill as ChildNode));
  });

  it('can close an active inline trigger imperatively', async () => {
    const onInlineTriggerStateChange = vi.fn();

    await act(async () => {
      root.render(
        <RichTextInput
          value="$pdf"
          onChange={() => {}}
          onInlineTriggerStateChange={onInlineTriggerStateChange}
          contexts={emptyContexts}
          onRemoveContext={() => {}}
        />
      );
    });

    const editor = container.querySelector('.rich-text-input') as (HTMLDivElement & {
      closeInlineTrigger?: () => void;
    }) | null;
    expect(editor).toBeTruthy();

    setCaret(editor!, '$pdf'.length);
    await act(async () => {
      editor!.dispatchEvent(new window.Event('input', { bubbles: true }));
    });

    await act(async () => {
      editor?.closeInlineTrigger?.();
    });

    expect(onInlineTriggerStateChange).toHaveBeenLastCalledWith({
      isActive: false,
      trigger: null,
      query: '',
      startOffset: 0,
    });
  });

  it('can append an inline skill token at the end with trailing space', async () => {
    const onChange = vi.fn();

    await act(async () => {
      root.render(
        <RichTextInput
          value="hello"
          onChange={onChange}
          contexts={emptyContexts}
          onRemoveContext={() => {}}
        />
      );
    });

    const editor = container.querySelector('.rich-text-input') as (HTMLDivElement & {
      appendInlineTokenAtEnd?: (token: string) => void;
    }) | null;
    expect(editor).toBeTruthy();

    await act(async () => {
      editor?.appendInlineTokenAtEnd?.('[$pdf]');
    });

    expect(onChange).toHaveBeenCalledWith('hello [$pdf]', emptyContexts);
    const skillPill = editor?.querySelector('.rich-text-tag-pill--skill-ref');
    expect(skillPill).toBeTruthy();
    expect(skillPill?.previousSibling?.textContent).toBe(' ');
    expect(skillPill?.nextSibling?.textContent).toBe(' ');
  });

  it('can append the Review reference through the same inline-token interaction', async () => {
    const onChange = vi.fn();

    await act(async () => {
      root.render(
        <RichTextInput
          value="hello"
          onChange={onChange}
          contexts={emptyContexts}
          onRemoveContext={() => {}}
        />
      );
    });

    const editor = container.querySelector('.rich-text-input') as RichTextInputElement | null;
    expect(editor).toBeTruthy();

    await act(async () => {
      editor?.appendInlineTokenAtEnd?.('[[bitfun-additional-mode:review]]');
    });

    expect(onChange).toHaveBeenCalledWith(
      'hello [[bitfun-additional-mode:review]]',
      emptyContexts,
    );
    expect(editor?.querySelector('.rich-text-tag-pill--additional-mode-ref')).toBeTruthy();
  });

  it('clears placeholder br before appending the first inline skill token', async () => {
    const onChange = vi.fn();

    await act(async () => {
      root.render(
        <RichTextInput
          value=""
          onChange={onChange}
          contexts={emptyContexts}
          onRemoveContext={() => {}}
        />
      );
    });

    const editor = container.querySelector('.rich-text-input') as (HTMLDivElement & {
      appendInlineTokenAtEnd?: (token: string) => void;
    }) | null;
    expect(editor).toBeTruthy();

    editor!.innerHTML = '<br>';

    await act(async () => {
      editor?.appendInlineTokenAtEnd?.('[$pdf]');
    });

    expect(onChange).toHaveBeenCalledWith('[$pdf]', emptyContexts);
    expect(editor?.querySelector('br')).toBeFalsy();
    expect(editor?.firstChild).toBe(editor?.querySelector('.rich-text-tag-pill--skill-ref'));
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

  it('renders a semantic large-paste capsule without leaking its controls into input text', async () => {
    const placeholder = '[Pasted Content 1001 chars]';
    const onChange = vi.fn();

    await act(async () => {
      root.render(
        <RichTextInput
          value={`before ${placeholder} after`}
          onChange={onChange}
          pendingLargePastes={{ [placeholder]: 'x'.repeat(1001) }}
          contexts={emptyContexts}
          onRemoveContext={() => {}}
        />
      );
    });

    const editor = container.querySelector('.rich-text-input') as HTMLDivElement;
    const capsule = editor.querySelector('[data-large-paste-placeholder]') as HTMLElement | null;
    expect(capsule).toBeTruthy();
    expect(capsule?.getAttribute('contenteditable')).toBe('false');
    expect(capsule?.getAttribute('role')).toBe('button');

    await act(async () => {
      editor.dispatchEvent(new window.Event('input', { bubbles: true }));
    });

    expect(onChange).toHaveBeenLastCalledWith(`before ${placeholder} after`, emptyContexts);
  });

  it('lets users view, copy, cancel, edit, save, and remove a large paste', async () => {
    const placeholder = '[Pasted Content 1001 chars]';
    const updatedPlaceholder = '[Pasted Content 1002 chars]';
    const originalText = 'a'.repeat(1001);
    const editedText = `${originalText}b`;
    const onChange = vi.fn();
    const onUpdateLargePaste = vi.fn(() => updatedPlaceholder);
    const onRemoveLargePaste = vi.fn();
    const writeText = vi.fn(async () => {});
    Object.defineProperty(window.navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    });

    await act(async () => {
      root.render(
        <RichTextInput
          value={placeholder}
          onChange={onChange}
          pendingLargePastes={{ [placeholder]: originalText }}
          onUpdateLargePaste={onUpdateLargePaste}
          onRemoveLargePaste={onRemoveLargePaste}
          contexts={emptyContexts}
          onRemoveContext={() => {}}
        />
      );
    });

    const editor = container.querySelector('.rich-text-input') as HTMLDivElement;
    const getCapsule = () => editor.querySelector('[data-large-paste-placeholder]') as HTMLElement;
    await act(async () => {
      getCapsule().click();
    });

    let textarea = document.body.querySelector(
      '.rich-text-large-paste-dialog__textarea textarea',
    ) as HTMLTextAreaElement;
    expect(textarea.value).toBe(originalText);

    const dialog = textarea.closest('[role="dialog"]') as HTMLElement;
    const dialogButtons = Array.from(dialog.querySelectorAll('button'));
    const [copyButton, cancelButton, saveButton] = dialogButtons.slice(-3);
    await act(async () => {
      copyButton?.click();
    });
    expect(writeText).toHaveBeenCalledWith(originalText);

    await act(async () => {
      Simulate.change(textarea, { target: { value: editedText } } as never);
    });
    await act(async () => {
      cancelButton?.click();
    });
    await act(async () => {
      getCapsule().click();
    });
    textarea = document.body.querySelector(
      '.rich-text-large-paste-dialog__textarea textarea',
    ) as HTMLTextAreaElement;
    expect(textarea.value).toBe(originalText);

    await act(async () => {
      Simulate.change(textarea, { target: { value: editedText } } as never);
    });
    await act(async () => {
      saveButton?.click();
    });

    expect(onUpdateLargePaste).toHaveBeenCalledWith(placeholder, editedText);
    expect(getCapsule().dataset.largePastePlaceholder).toBe(updatedPlaceholder);
    expect(onChange).toHaveBeenLastCalledWith(updatedPlaceholder, emptyContexts);

    const removeButton = getCapsule().querySelector('button');
    await act(async () => {
      removeButton?.click();
    });
    expect(onRemoveLargePaste).toHaveBeenCalledWith(updatedPlaceholder);
    expect(editor.querySelector('[data-large-paste-placeholder]')).toBeNull();
  });

  it('inserts Unicode large pastes as capsules at the caret', async () => {
    const text = '文😀'.repeat(501);
    const placeholder = '[Pasted Content 1002 chars]';
    const onChange = vi.fn();
    const onLargePaste = vi.fn(() => placeholder);

    await act(async () => {
      root.render(
        <RichTextInput
          value=""
          onChange={onChange}
          onLargePaste={onLargePaste}
          pendingLargePastes={{ [placeholder]: text }}
          contexts={emptyContexts}
          onRemoveContext={() => {}}
        />
      );
    });

    const editor = container.querySelector('.rich-text-input') as HTMLDivElement;
    setCaret(editor, 0);
    const pasteEvent = new window.Event('paste', { bubbles: true, cancelable: true });
    Object.defineProperty(pasteEvent, 'clipboardData', {
      value: {
        items: [],
        getData: (type: string) => type === 'text/plain' ? text : '',
      },
    });
    await act(async () => {
      editor.dispatchEvent(pasteEvent);
    });

    expect(onLargePaste).toHaveBeenCalledWith(text);
    expect(editor.querySelector('[data-large-paste-placeholder]')).toBeTruthy();
    expect(onChange).toHaveBeenLastCalledWith(placeholder, emptyContexts);
    const selection = window.getSelection();
    expect(selection?.anchorNode?.nodeType).toBe(Node.TEXT_NODE);
    expect(selection?.anchorNode?.textContent).toBe('\u200B');
    expect(selection?.anchorOffset).toBe(1);
  });

  it('keeps the caret visually anchored after consecutive large-paste capsules', async () => {
    const text = 'x'.repeat(1001);
    const placeholders = [
      '[Pasted Content 1001 chars]',
      '[Pasted Content 1001 chars] #2',
      '[Pasted Content 1001 chars] #3',
    ];
    const onChange = vi.fn();
    let pasteIndex = 0;
    const onLargePaste = vi.fn(() => placeholders[pasteIndex++] ?? null);

    await act(async () => {
      root.render(
        <RichTextInput
          value=""
          onChange={onChange}
          onLargePaste={onLargePaste}
          contexts={emptyContexts}
          onRemoveContext={() => {}}
        />
      );
    });

    const editor = container.querySelector('.rich-text-input') as HTMLDivElement;
    setCaret(editor, 0);
    const dispatchPaste = async () => {
      const pasteEvent = new window.Event('paste', { bubbles: true, cancelable: true });
      Object.defineProperty(pasteEvent, 'clipboardData', {
        value: {
          items: [],
          getData: (type: string) => type === 'text/plain' ? text : '',
        },
      });
      await act(async () => {
        editor.dispatchEvent(pasteEvent);
      });
    };

    await dispatchPaste();
    await dispatchPaste();
    await dispatchPaste();

    expect(editor.querySelectorAll('[data-large-paste-placeholder]')).toHaveLength(3);
    expect(onChange).toHaveBeenLastCalledWith(placeholders.join(''), emptyContexts);
    const selection = window.getSelection();
    expect(selection?.anchorNode?.nodeType).toBe(Node.TEXT_NODE);
    expect(selection?.anchorNode?.textContent).toBe('\u200B');
    expect(selection?.anchorOffset).toBe(1);

    const pressBackspace = async () => {
      await act(async () => {
        editor.dispatchEvent(new window.KeyboardEvent('keydown', {
          key: 'Backspace',
          bubbles: true,
          cancelable: true,
        }));
      });
    };
    await pressBackspace();
    expect(editor.querySelectorAll('[data-large-paste-placeholder]')).toHaveLength(2);
    expect(selection?.anchorNode?.previousSibling).toBe(
      editor.querySelector('[data-large-paste-placeholder]:nth-of-type(2)'),
    );
    expect(selection?.anchorOffset).toBe(1);

    await pressBackspace();
    expect(editor.querySelectorAll('[data-large-paste-placeholder]')).toHaveLength(1);
    expect(selection?.anchorNode?.previousSibling).toBe(
      editor.querySelector('[data-large-paste-placeholder]'),
    );
    expect(selection?.anchorOffset).toBe(1);
  });

  it('closes an open large-paste editor when the surface-scoped source changes', async () => {
    const placeholder = '[Pasted Content 1001 chars]';
    const renderInput = (text: string) => (
      <RichTextInput
        value={placeholder}
        onChange={() => {}}
        pendingLargePastes={{ [placeholder]: text }}
        contexts={emptyContexts}
        onRemoveContext={() => {}}
      />
    );

    await act(async () => {
      root.render(renderInput('surface-a'));
    });
    const capsule = container.querySelector('[data-large-paste-placeholder]') as HTMLElement;
    await act(async () => {
      capsule.click();
    });
    expect(document.body.querySelector('textarea')).toBeTruthy();

    await act(async () => {
      root.render(renderInput('surface-b'));
    });
    expect(document.body.querySelector('[role="dialog"][data-state="open"]')).toBeNull();
  });
});
