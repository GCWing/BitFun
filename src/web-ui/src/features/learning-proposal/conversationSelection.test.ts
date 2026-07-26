import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { resolveConversationSelection } from './conversationSelection';

let JSDOMCtor: (new (html?: string) => { window: Window & typeof globalThis }) | null = null;

try {
  const jsdom = await import('jsdom');
  JSDOMCtor = jsdom.JSDOM as typeof JSDOMCtor;
} catch {
  JSDOMCtor = null;
}

const describeWithJsdom = JSDOMCtor ? describe : describe.skip;

describeWithJsdom('resolveConversationSelection', () => {
  let dom: { window: Window & typeof globalThis };

  beforeEach(() => {
    dom = new JSDOMCtor!(`<!doctype html><html><body>
      <main id="scope">
        <div class="virtual-item-wrapper" data-turn-id="turn-1" data-item-type="model-round" data-learning-source-kind="unknown" data-learning-item-id="round-1">
          <div data-turn-id="turn-1" data-round-id="round-1" data-learning-source-kind="assistant_text" data-learning-item-id="item-1">High-value correction</div>
          <div data-turn-id="turn-1" data-round-id="round-1" data-learning-source-kind="tool" data-learning-item-id="item-2">Tool output</div>
        </div>
        <div class="virtual-item-wrapper" data-turn-id="turn-2" data-item-type="user-message" data-learning-source-kind="user_message" data-learning-item-id="message-2">Second message</div>
        <div class="virtual-item-wrapper" data-turn-id="turn-3" data-item-type="model-round" data-learning-source-kind="unknown" data-learning-item-id="round-3">
          <div data-turn-id="turn-3" data-round-id="round-3" data-streaming="true">
            <div data-learning-source-kind="assistant_text" data-learning-item-id="item-3">Incomplete response</div>
          </div>
        </div>
        <div class="virtual-item-wrapper" data-turn-id="turn-4" data-item-type="user-steering-message">Steering correction</div>
        <div class="virtual-item-wrapper" data-turn-id="turn-5" data-item-type="turn-completion-notice" data-learning-source-kind="unknown" data-learning-item-id="turn-completion:turn-5">Turn completed</div>
      </main>
    </body></html>`);
    vi.stubGlobal('window', dom.window);
    vi.stubGlobal('document', dom.window.document);
    vi.stubGlobal('Node', dom.window.Node);
    vi.stubGlobal('HTMLElement', dom.window.HTMLElement);
  });

  afterEach(() => {
    dom.window.close();
    vi.unstubAllGlobals();
  });

  it('captures the stable source identifiers for a selection inside one flow item', () => {
    const source = document.querySelector<HTMLElement>('[data-learning-item-id="item-1"]')!;
    const range = document.createRange();
    range.selectNodeContents(source);
    const selection = window.getSelection();
    selection?.removeAllRanges();
    selection?.addRange(range);

    expect(resolveConversationSelection(selection, document.querySelector('#scope'))).toMatchObject({
      selectedText: 'High-value correction',
      turnId: 'turn-1',
      roundId: 'round-1',
      itemId: 'item-1',
      sourceKind: 'assistant_text',
    });
  });

  it('rejects a selection that spans more than one virtual message item', () => {
    const first = document.querySelector<HTMLElement>('[data-learning-item-id="item-1"]')!;
    const second = document.querySelector<HTMLElement>('[data-learning-item-id="message-2"]')!;
    const range = document.createRange();
    range.setStart(first.firstChild!, 0);
    range.setEnd(second.firstChild!, second.textContent!.length);
    const selection = window.getSelection();
    selection?.removeAllRanges();
    selection?.addRange(range);

    expect(resolveConversationSelection(selection, document.querySelector('#scope'))).toBeNull();
  });

  it('rejects a selection that spans two flow items in the same model round', () => {
    const first = document.querySelector<HTMLElement>('[data-learning-item-id="item-1"]')!;
    const second = document.querySelector<HTMLElement>('[data-learning-item-id="item-2"]')!;
    const range = document.createRange();
    range.setStart(first.firstChild!, 0);
    range.setEnd(second.firstChild!, second.textContent!.length);
    const selection = window.getSelection();
    selection?.removeAllRanges();
    selection?.addRange(range);

    expect(resolveConversationSelection(selection, document.querySelector('#scope'))).toBeNull();
  });

  it('uses the virtual item metadata when a selection spans nested nodes in one message', () => {
    const wrapper = document.querySelector<HTMLElement>('[data-turn-id="turn-2"]')!;
    wrapper.innerHTML = '<span>Second</span> <strong>message</strong>';
    const range = document.createRange();
    range.selectNodeContents(wrapper);
    const selection = window.getSelection();
    selection?.removeAllRanges();
    selection?.addRange(range);

    expect(resolveConversationSelection(selection, document.querySelector('#scope'))).toMatchObject({
      turnId: 'turn-2',
      itemId: 'message-2',
      sourceKind: 'user_message',
    });
  });

  it('rejects selected text from a model round that is still streaming', () => {
    const source = document.querySelector<HTMLElement>('[data-learning-item-id="item-3"]')!;
    const range = document.createRange();
    range.selectNodeContents(source);
    const selection = window.getSelection();
    selection?.removeAllRanges();
    selection?.addRange(range);

    expect(resolveConversationSelection(selection, document.querySelector('#scope'))).toBeNull();
  });

  it.each([
    ['user-steering-message', 'Steering correction'],
    ['turn-completion-notice', 'Turn completed'],
  ])('rejects unsupported %s wrapper provenance', (itemType, text) => {
    const source = document.querySelector<HTMLElement>(`[data-item-type="${itemType}"]`)!;
    const range = document.createRange();
    range.selectNodeContents(source);
    const selection = window.getSelection();
    selection?.removeAllRanges();
    selection?.addRange(range);

    expect(selection?.toString()).toBe(text);
    expect(resolveConversationSelection(selection, document.querySelector('#scope'))).toBeNull();
  });
});
