/**
 * quoteSelectPick — 选区提取（纯函数，与组件分离以符合 react-refresh 规则）
 *
 * 依赖的现有契约：
 *  - C1: user-message-item 行（data-bf-component + data-turn-id）
 *  - C2: model-round-item 行（data-bf-component）
 */
export const QUOTE_MESSAGE_SELECTOR = [
  '[data-bf-component="user-message-item"]',
  '[data-bf-component="model-round-item"]',
].join(', ');

export interface QuotePick {
  text: string;
  /** user = 你的原话；assistant = AI 的回答 */
  who: 'user' | 'assistant';
  turnId: string | null;
  /** 消息在可见列表中的序号（1-based，DOM 兜底；精确序号见 README 风险项 R2） */
  pos: number | null;
  rect: DOMRect;
}

/** 从当前选区提取引用信息；选区不在消息行内 / 折叠 / 空文本时返回 null */
export function pickFromSelection(): QuotePick | null {
  const sel = window.getSelection();
  if (!sel || sel.isCollapsed || sel.rangeCount === 0) return null;
  const text = sel.toString();
  if (!text || !text.trim()) return null;

  const range = sel.getRangeAt(0);
  const start = sel.anchorNode;
  if (!start || !start.parentElement) return null;

  const row =
    start.nodeType === 1
      ? (start as Element).closest(QUOTE_MESSAGE_SELECTOR)
      : start.parentElement.closest(QUOTE_MESSAGE_SELECTOR);
  if (!row) return null;

  const component = row.getAttribute('data-bf-component');
  const scroll = row.closest('[data-bf-component="chat-pane"]');
  let pos: number | null = null;
  if (scroll) {
    const rows = Array.from(scroll.querySelectorAll(QUOTE_MESSAGE_SELECTOR));
    const idx = rows.indexOf(row);
    if (idx !== -1) pos = idx + 1;
  }

  return {
    text: text.trim(),
    who: component === 'user-message-item' ? 'user' : 'assistant',
    turnId: row.getAttribute('data-turn-id'),
    pos,
    rect: range.getBoundingClientRect(),
  };
}
