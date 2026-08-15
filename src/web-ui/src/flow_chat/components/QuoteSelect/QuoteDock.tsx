/**
 * QuoteDock — 输入框上方的可移除引用卡片
 *
 * 渲染进 ChatInput 新增的 composerDock 扩展点（见 chatinput-patch.md）。
 * 与草稿内容双向同步：
 *  - draft 中 marker 被删（发送清空 / 手动删除）→ 卡片自动摘除
 *  - 点卡片 ✕ → 从草稿移除 marker 并压缩多余空行
 */
import React, { useEffect } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { quoteSelectStore } from './quoteSelectStore';

export const QuoteDock: React.FC<{ draft: string }> = ({ draft }) => {
  const { t } = useI18n('flow-chat');
  const quotes = quoteSelectStore((s) => s.quotes);

  useEffect(() => {
    quoteSelectStore.getState().syncDraft(draft);
  }, [draft]);

  if (quotes.length === 0) return null;

  return (
    <div className="quote-select__dock" data-bf-component="quote-select" data-bf-part="dock">
      {quotes.map((q) => (
        <div key={q.id} className="quote-select__chip" data-bf-part="chip">
          <span className="quote-select__chip-ico" aria-hidden>❝</span>
          <span className="quote-select__chip-body">
            <span className="quote-select__chip-label">
              {t('quote.from')} · {q.who}
              {q.pos ? ` · ${t('quote.msgNo', { pos: q.pos })}` : ''}
            </span>
            <span className="quote-select__chip-text">{q.text.replace(/\s+/g, ' ')}</span>
          </span>
          <button
            type="button"
            className="quote-select__chip-x"
            data-bf-part="removeChip"
            title={t('quote.remove')}
            aria-label={t('quote.remove')}
            onClick={() => quoteSelectStore.getState().removeQuote(q.id)}
          >
            ✕
          </button>
        </div>
      ))}
    </div>
  );
};

export default QuoteDock;
