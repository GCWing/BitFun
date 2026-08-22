/**
 * quoteSelectStore — 「选中→引用」的共享状态与输入框写入逻辑
 *
 * 依赖的现有契约（均已核实，见 README §2）：
 *  - C3: globalEventBus.emit('fill-chat-input', { content, mode })  写输入框
 *  - C5: globalEventBus.emit('chat-input:get-state', { getValue })  读输入框
 *
 * 行为对齐 dsh @deepseek-ai/dsh-quote-select：
 *  - 引用 marker 在上，已有草稿在下
 *  - 草稿变化时，marker 被删则摘除对应卡片
 *  - 删卡片时，从草稿中移除 marker 并压缩多余空行
 */
import { create } from 'zustand';
import { globalEventBus } from '@/infrastructure/event-bus';

export const MAX_QUOTE_LEN = 2000;

export interface Quote {
  id: string;
  marker: string;
  text: string;
  who: string;
  pos?: number | null;
}

export function makeMarker(whoDesc: string, pos: number | null | undefined, text: string): string {
  const body =
    text.length > MAX_QUOTE_LEN
      ? `${text.slice(0, MAX_QUOTE_LEN)}…（原文更长，已截断）`
      : text;
  // 注意：dsh 原版在无位置时输出「对话对话历史」（模板拼接 bug），此处修正
  const head = pos
    ? `【引用自对话第 ${pos} 条消息（${whoDesc}）】`
    : `【引用自对话历史（${whoDesc}）】`;
  return `${head}「${body}」`;
}

export interface QuoteSelectState {
  quotes: Quote[];
  draft: string;
  addQuote: (q: Omit<Quote, 'id'>) => void;
  removeQuote: (id: string) => void;
  syncDraft: (draft: string) => void;
  clearAll: () => void;
  /** 把 marker 写入输入框（引用在上、草稿在下），并保留光标可编辑 */
  fillIntoComposer: (marker: string) => void;
}

/** 通过 chat-input:get-state 同步读取当前草稿（现有实现为同步回填闭包） */
function readCurrentDraft(): string {
  let value = '';
  try {
    globalEventBus.emit('chat-input:get-state', {
      getValue: (v: string) => {
        value = v;
      },
    });
  } catch {
    // 事件无监听者时忽略，按空草稿处理
  }
  return value;
}

export const quoteSelectStore = create<QuoteSelectState>((set, get) => ({
  quotes: [],
  draft: '',

  addQuote: (q) =>
    set((s) => ({
      quotes: [...s.quotes, { ...q, id: globalThis.crypto?.randomUUID?.() ?? `${Date.now()}` }],
    })),

  removeQuote: (id) => {
    const q = get().quotes.find((x) => x.id === id);
    set((s) => ({ quotes: s.quotes.filter((x) => x.id !== id) }));
    if (!q) return;
    const next = (get().draft ?? '').split(q.marker).join('').replace(/\n{3,}/g, '\n\n');
    globalEventBus.emit('fill-chat-input', { content: next, mode: 'replace' });
  },

  syncDraft: (draft) =>
    set((s) => {
      if (s.quotes.length === 0) return { draft };
      const keep = s.quotes.filter((q) => draft.includes(q.marker));
      return { draft, quotes: keep.length === s.quotes.length ? s.quotes : keep };
    }),

  clearAll: () => set({ quotes: [], draft: '' }),

  fillIntoComposer: (marker) => {
    const current = readCurrentDraft();
    const next = current.trim() ? `${marker}\n\n${current}` : marker;
    globalEventBus.emit('fill-chat-input', { content: next, mode: 'replace' });
  },
}));
