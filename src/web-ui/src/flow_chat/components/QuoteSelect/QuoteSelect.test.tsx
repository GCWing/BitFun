/**
 * QuoteSelect 单元测试（vitest + jsdom，对齐仓库现有测试风格）
 *
 * 覆盖：
 *  - makeMarker：格式 / 截断 / 位置文案
 *  - pickFromSelection：消息行内选中 / 消息外选中 / 折叠选区 / 空文本
 *  - store.fillIntoComposer：事件负载 = marker + 当前草稿
 *  - store.syncDraft：marker 消失 → 卡片摘除
 *  - store.removeQuote：删卡 → 草稿移除 marker 并压缩空行
 */
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/react';
import { quoteSelectStore, makeMarker, MAX_QUOTE_LEN } from './quoteSelectStore';
import { pickFromSelection } from './quoteSelectPick';
import { QuoteDock } from './QuoteDock';

// ---- globalEventBus mock（避免依赖真实事件总线） ----
vi.mock('@/infrastructure/event-bus', () => ({
  globalEventBus: {
    emit: vi.fn(),
    on: vi.fn(),
    off: vi.fn(),
  },
}));

import { globalEventBus } from '@/infrastructure/event-bus';

// ---- i18n mock ----
vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({ t: (k: string, vars?: Record<string, unknown>) => {
    const table: Record<string, string> = {
      'quote.copy': '复制文字',
      'quote.copied': '已复制',
      'quote.addToConversation': '添加到对话',
      'quote.sourceUser': '你的原话',
      'quote.sourceAssistant': 'AI 的回答',
      'quote.remove': '移除这条引用',
      'quote.from': '引用',
      'quote.msgNo': `第 ${String(vars?.pos ?? '')} 条消息`,
    };
    return table[k] ?? k;
  } }),
}));

describe('makeMarker', () => {
  it('生成带来源与位置的引用标记', () => {
    expect(makeMarker('你的原话', 3, 'hello')).toBe(
      '【引用自对话第 3 条消息（你的原话）】「hello」',
    );
  });

  it('无位置时标注对话历史', () => {
    expect(makeMarker('AI 的回答', null, 'hi')).toBe(
      '【引用自对话历史（AI 的回答）】「hi」',
    );
  });

  it('超过 2000 字截断并标注', () => {
    const long = 'x'.repeat(MAX_QUOTE_LEN + 100);
    const marker = makeMarker('你的原话', 1, long);
    expect(marker).toContain('…（原文更长，已截断）');
    expect(marker.length).toBeLessThan(long.length + 100);
  });
});

describe('pickFromSelection', () => {
  function makeMessageRow(component: string, turnId: string | null): HTMLElement {
    const pane = document.createElement('div');
    pane.setAttribute('data-bf-component', 'chat-pane');
    const row = document.createElement('div');
    row.setAttribute('data-bf-component', component);
    if (turnId) row.setAttribute('data-turn-id', turnId);
    const content = document.createElement('div');
    content.setAttribute('data-bf-part', 'content');
    content.textContent = '选中文本';
    row.appendChild(content);
    pane.appendChild(row);
    document.body.appendChild(pane);
    return row;
  }

  function mockSelection(anchorNode: Node | null, text: string, rect: DOMRect) {
    const sel = {
      isCollapsed: !text,
      rangeCount: text ? 1 : 0,
      toString: () => text,
      getRangeAt: () => ({ startContainer: anchorNode, getBoundingClientRect: () => rect }),
      anchorNode,
    };
    Object.defineProperty(window, 'getSelection', { value: () => sel, configurable: true });
  }

  afterEach(() => {
    document.body.innerHTML = '';
    vi.restoreAllMocks();
  });

  it('在用户消息行内选中 → who=user、turnId 正确、序号=1', () => {
    const row = makeMessageRow('user-message-item', 'turn-42');
    mockSelection(row.querySelector('[data-bf-part="content"]'), '选中文本', new DOMRect(0, 0, 100, 20));
    const pick = pickFromSelection();
    expect(pick).not.toBeNull();
    expect(pick!.who).toBe('user');
    expect(pick!.turnId).toBe('turn-42');
    expect(pick!.pos).toBe(1);
  });

  it('在 AI 消息行内选中 → who=assistant', () => {
    const row = makeMessageRow('model-round-item', null);
    mockSelection(row, '回答文本', new DOMRect(0, 0, 100, 20));
    const pick = pickFromSelection();
    expect(pick!.who).toBe('assistant');
    expect(pick!.turnId).toBeNull();
  });

  it('选区在消息行外 → null', () => {
    const outside = document.createElement('div');
    document.body.appendChild(outside);
    mockSelection(outside, '外部文本', new DOMRect(0, 0, 100, 20));
    expect(pickFromSelection()).toBeNull();
  });

  it('折叠选区 / 空文本 → null', () => {
    mockSelection(document.body, '', new DOMRect(0, 0, 0, 0));
    expect(pickFromSelection()).toBeNull();
  });
});

describe('quoteSelectStore', () => {
  beforeEach(() => {
    quoteSelectStore.setState({ quotes: [], draft: '' });
    vi.mocked(globalEventBus.emit).mockClear();
  });

  it('fillIntoComposer：marker 在上、草稿在下，走 replace 模式', () => {
    // 模拟 chat-input:get-state 同步回填
    vi.mocked(globalEventBus.emit).mockImplementation((event: string, payload: any) => {
      if (event === 'chat-input:get-state' && payload?.getValue) {
        payload.getValue('已有草稿');
      }
    });
    quoteSelectStore.getState().fillIntoComposer('【引用】「hi」');
    const emit = vi.mocked(globalEventBus.emit);
    const fillCall = emit.mock.calls.find(([e]) => e === 'fill-chat-input');
    expect(fillCall).toBeDefined();
    expect(fillCall![1]).toEqual({
      content: '【引用】「hi」\n\n已有草稿',
      mode: 'replace',
    });
  });

  it('syncDraft：marker 从草稿消失 → 卡片摘除', () => {
    const s = quoteSelectStore.getState();
    s.addQuote({ marker: '【引用】「hi」', text: 'hi', who: '你的原话', pos: 1 });
    expect(quoteSelectStore.getState().quotes).toHaveLength(1);
    quoteSelectStore.getState().syncDraft('完全不包含 marker 的内容');
    expect(quoteSelectStore.getState().quotes).toHaveLength(0);
  });

  it('removeQuote：从草稿移除 marker 并压缩多余空行', () => {
    const s = quoteSelectStore.getState();
    s.addQuote({ marker: 'M1', text: 'hi', who: '你的原话', pos: 1 });
    const id = quoteSelectStore.getState().quotes[0].id;
    quoteSelectStore.setState({ draft: 'M1\n\n\n\n后续' });
    quoteSelectStore.getState().removeQuote(id);
    const emit = vi.mocked(globalEventBus.emit);
    const fillCall = emit.mock.calls.find(([e]) => e === 'fill-chat-input');
    // removeQuote 保留 marker 移除后的前导空行（对齐 dsh：仅压缩 3+ 连续空行）
    expect(fillCall![1].content).toBe('\n\n后续');
  });
});

describe('QuoteDock', () => {
  beforeEach(() => {
    quoteSelectStore.setState({ quotes: [], draft: '' });
  });

  afterEach(() => {
    cleanup();
  });

  it('有引用时渲染卡片，无引用时返回 null', () => {
    const { container } = render(<QuoteDock draft="" />);
    expect(container.firstChild).toBeNull();
    cleanup(); // 先卸载，避免 store 更新触发旧实例重渲染导致重复节点

    quoteSelectStore.getState().addQuote({
      marker: 'M1',
      text: '原文内容',
      who: '你的原话',
      pos: 2,
    });
    const { getByText } = render(<QuoteDock draft="M1" />);
    expect(getByText('原文内容')).toBeDefined();
    expect(getByText(/第 2 条消息/)).toBeDefined();
  });

  it('点击 ✕ 移除引用并同步草稿', () => {
    quoteSelectStore.getState().addQuote({
      marker: 'M1',
      text: '原文',
      who: 'AI 的回答',
      pos: 1,
    });
    quoteSelectStore.setState({ draft: 'M1' });
    const { getAllByRole } = render(<QuoteDock draft="M1" />);
    fireEvent.click(getAllByRole('button')[0]);
    expect(quoteSelectStore.getState().quotes).toHaveLength(0);
  });
});
