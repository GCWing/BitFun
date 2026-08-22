/**
 * QuoteSelect — 选中对话消息文字 → 浮动菜单「复制文字 / 添加到对话」
 *
 * 对齐 dsh @deepseek-ai/dsh-quote-select 与 qoderwork 的「选中 → 添加到对话」工作流。
 * 纯客户端组件：只监听事件 + 渲染浮动菜单（createPortal），自身不输出布局节点。
 *
 * 依赖的现有契约（均已核实，见 README §2）：
 *  - C1: user-message-item 行：data-bf-component + data-turn-id
 *  - C2: model-round-item 行：data-bf-component（无 data-turn-id）
 *  - 浮层宿主: getAppearanceOverlayHost()
 *  - 写入: quoteSelectStore.fillIntoComposer → C3/C5
 */
import React, { useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { useI18n } from '@/infrastructure/i18n';
import { quoteSelectStore, makeMarker } from './quoteSelectStore';
import { pickFromSelection, type QuotePick } from './quoteSelectPick';

export const QuoteSelect: React.FC = () => {
  const { t } = useI18n('flow-chat');
  const [pick, setPick] = useState<QuotePick | null>(null);
  const [copied, setCopied] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const selTimerRef = useRef<number | null>(null);

  const hideMenu = useCallback(() => {
    setPick(null);
    setCopied(false);
  }, []);

  const showOrHide = useCallback(() => {
    const next = pickFromSelection();
    setPick(next);
    if (!next) setCopied(false);
  }, []);

  useEffect(() => {
    const onMouseUp = (e: MouseEvent) => {
      if (e.button !== 0) return; // 不抢右键系统菜单
      if (menuRef.current?.contains(e.target as Node)) return;
      window.setTimeout(showOrHide, 0);
    };
    const onSelectionChange = () => {
      if (selTimerRef.current) window.clearTimeout(selTimerRef.current);
      selTimerRef.current = window.setTimeout(showOrHide, 250);
    };
    const onDocMouseDown = (e: MouseEvent) => {
      if (menuRef.current?.contains(e.target as Node)) return;
      hideMenu();
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') hideMenu();
    };
    const onHide = () => hideMenu();

    document.addEventListener('mouseup', onMouseUp);
    document.addEventListener('selectionchange', onSelectionChange);
    document.addEventListener('mousedown', onDocMouseDown);
    document.addEventListener('keydown', onKeyDown);
    document.addEventListener('contextmenu', onHide);
    window.addEventListener('scroll', onHide, true);
    window.addEventListener('resize', onHide);
    window.addEventListener('blur', onHide);

    return () => {
      document.removeEventListener('mouseup', onMouseUp);
      document.removeEventListener('selectionchange', onSelectionChange);
      document.removeEventListener('mousedown', onDocMouseDown);
      document.removeEventListener('keydown', onKeyDown);
      document.removeEventListener('contextmenu', onHide);
      window.removeEventListener('scroll', onHide, true);
      window.removeEventListener('resize', onHide);
      window.removeEventListener('blur', onHide);
      if (selTimerRef.current) window.clearTimeout(selTimerRef.current);
    };
  }, [hideMenu, showOrHide]);

  const handleCopy = useCallback(async () => {
    if (!pick) return;
    const done = () => {
      setCopied(true);
      window.setTimeout(() => {
        setPick((cur) => (cur === pick ? null : cur));
        setCopied(false);
      }, 900);
    };
    if (navigator.clipboard?.writeText) {
      navigator.clipboard.writeText(pick.text).then(done).catch(() => {
        legacyCopy(pick.text);
        done();
      });
    } else {
      legacyCopy(pick.text);
      done();
    }
  }, [pick]);

  const handleAdd = useCallback(() => {
    if (!pick) return;
    const who = pick.who === 'user' ? t('quote.sourceUser') : t('quote.sourceAssistant');
    const marker = makeMarker(who, pick.pos, pick.text);
    quoteSelectStore.getState().addQuote({ marker, text: pick.text, who, pos: pick.pos });
    quoteSelectStore.getState().fillIntoComposer(marker);
    hideMenu();
  }, [hideMenu, pick, t]);

  if (!pick) return null;

  // 浮动菜单定位：选区上方居中，越界翻转；按钮按下不抢焦点、不取消选中
  const host = getAppearanceOverlayHost() ?? document.body;
  const rect = pick.rect;
  const MENU_W = 220;
  const MENU_H = 38;
  let left = rect.left + rect.width / 2 - MENU_W / 2;
  left = Math.max(8, Math.min(left, window.innerWidth - MENU_W - 8));
  let top = rect.top - MENU_H - 8;
  if (top < 8) top = Math.min(rect.bottom + 8, window.innerHeight - MENU_H - 8);

  return createPortal(
    <div
      ref={menuRef}
      className="quote-select__menu"
      data-bf-component="quote-select"
      data-bf-part="menu"
      style={{ left: Math.round(left), top: Math.round(top) }}
      onMouseDown={(e) => e.preventDefault()}
      onMouseUp={(e) => e.stopPropagation()}
    >
      <button
        type="button"
        className={copied ? 'quote-select__menu-btn quote-select__menu-btn--copied' : 'quote-select__menu-btn'}
        data-bf-part="copy"
        onClick={handleCopy}
      >
        {copied ? '✓' : '📋'}
        <span>{copied ? t('quote.copied') : t('quote.copy')}</span>
      </button>
      <button
        type="button"
        className="quote-select__menu-btn"
        data-bf-part="addToConversation"
        onClick={handleAdd}
      >
        💬<span>{t('quote.addToConversation')}</span>
      </button>
    </div>,
    host,
  );
};

function legacyCopy(text: string): boolean {
  try {
    const ta = document.createElement('textarea');
    ta.value = text;
    ta.style.position = 'fixed';
    ta.style.opacity = '0';
    document.body.appendChild(ta);
    ta.select();
    const ok = document.execCommand('copy');
    document.body.removeChild(ta);
    return ok;
  } catch {
    return false;
  }
}

export default QuoteSelect;
