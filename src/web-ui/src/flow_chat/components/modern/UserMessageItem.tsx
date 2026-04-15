/**
 * User message item component.
 * Renders user input messages.
 */

import React, { useState, useCallback, useRef, useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Copy, Check, RotateCcw, Loader2, ArrowDownToLine, X, User, Orbit } from 'lucide-react';
import type { DialogTurn } from '../../types/flow-chat';
import type { TriggerSource } from '@/shared/types/session-history';
import { useFlowChatContext } from './FlowChatContext';
import { useActiveSession } from '../../store/modernFlowChatStore';
import { flowChatStore } from '../../store/FlowChatStore';
import { snapshotAPI } from '@/infrastructure/api';
import { notificationService } from '@/shared/notification-system';
import { globalEventBus } from '@/infrastructure/event-bus';
import { ReproductionStepsBlock, Tooltip, confirmDanger } from '@/component-library';
import { createLogger } from '@/shared/utils/logger';
import './UserMessageItem.scss';

const log = createLogger('UserMessageItem');

/** Returns true when the turn was triggered by a non-human source. */
function isSystemTrigger(triggerSource: TriggerSource | undefined): boolean {
  return !!triggerSource && triggerSource !== 'desktop_ui';
}

/** Maps a TriggerSource to a CSS modifier suffix. */
function triggerSourceModifier(triggerSource: TriggerSource | undefined): string {
  switch (triggerSource) {
    case 'agent_session': return 'agent-session';
    case 'scheduled_job': return 'scheduled-job';
    case 'bot': return 'bot';
    case 'cli': return 'cli';
    case 'desktop_api':
    case 'remote_relay': return 'remote';
    default: return '';
  }
}

/** Maps a TriggerSource to a display label (non-agent_session; those use the Agentic OS nav icon). */
function triggerSourceLabel(triggerSource: TriggerSource | undefined): string {
  switch (triggerSource) {
    case 'scheduled_job': return 'Scheduled';
    case 'bot': return 'Bot';
    case 'cli': return 'CLI';
    case 'desktop_api': return 'API';
    case 'remote_relay': return 'Remote';
    default: return 'System';
  }
}


interface UserMessageItemProps {
  message: DialogTurn['userMessage'];
  turnId: string;
}

function formatRoundTimestamp(locale: string, ms: number): string {
  try {
    return new Intl.DateTimeFormat(locale, {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
      hour12: false,
    }).format(ms);
  } catch {
    return new Date(ms).toLocaleString();
  }
}

/** Splits text into segments and wraps matching parts with <mark>. */
function highlightText(text: string, query: string): React.ReactNode {
  if (!query.trim()) return text;
  const q = query.trim();
  const parts = text.split(new RegExp(`(${q.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')})`, 'gi'));
  if (parts.length <= 1) return text;
  return (
    <>
      {parts.map((part, i) =>
        part.toLowerCase() === q.toLowerCase() ? (
          <mark key={i} className="user-message-item__search-highlight">{part}</mark>
        ) : (
          part
        ),
      )}
    </>
  );
}

export const UserMessageItem = React.memo<UserMessageItemProps>(
  ({ message, turnId }) => {
    const { t, i18n } = useTranslation('flow-chat');
    const { config, sessionId, activeSessionOverride, searchQuery } = useFlowChatContext();
    const activeSessionFromStore = useActiveSession();
    const activeSession = activeSessionOverride ?? activeSessionFromStore;
    const [copied, setCopied] = useState(false);
    const [expanded, setExpanded] = useState(false);
    const [isTruncated, setIsTruncated] = useState(false);
    const [isRollingBack, setIsRollingBack] = useState(false);
    const [lightboxImage, setLightboxImage] = useState<string | null>(null);
    const containerRef = useRef<HTMLDivElement>(null);
    const contentRef = useRef<HTMLSpanElement>(null);
    const messageContent = typeof message?.content === 'string' ? message.content : String(message?.content || '');
    const messageImages = useMemo(() => message?.images ?? [], [message?.images]);

    const turnIndex = activeSession?.dialogTurns.findIndex(t => t.id === turnId) ?? -1;
    const dialogTurn = turnIndex >= 0 ? activeSession?.dialogTurns[turnIndex] : null;
    const turnStartMs = dialogTurn?.startTime ?? message?.timestamp ?? 0;
    const sessionStartMs = activeSession?.createdAt ?? turnStartMs;

    const roundMarkerText = useMemo(() => {
      const locale = i18n.language || undefined;
      if (turnIndex === 0) {
        return t('message.sessionStartMarker', {
          time: formatRoundTimestamp(locale ?? 'en-US', sessionStartMs),
        });
      }
      return formatRoundTimestamp(locale ?? 'en-US', turnStartMs);
    }, [turnIndex, sessionStartMs, turnStartMs, t, i18n.language]);

    const roundMarkerIso = useMemo(() => {
      const ms = turnIndex === 0 ? sessionStartMs : turnStartMs;
      return new Date(ms).toISOString();
    }, [turnIndex, sessionStartMs, turnStartMs]);
    const isFailed = dialogTurn?.status === 'error';
    const isSystem = isSystemTrigger(message?.triggerSource);
    const canRollback = !!sessionId && turnIndex >= 0 && !isRollingBack && !isSystem;

    const { displayText, reproductionSteps } = useMemo(() => {
      const reproductionRegex = /<reproduction_steps>([\s\S]*?)<\/reproduction_steps\s*>?/g;
      const reproductionMatch = reproductionRegex.exec(messageContent);
      const reproduction = reproductionMatch ? reproductionMatch[1].trim() : null;

      let cleaned = messageContent.replace(reproductionRegex, '').trim();

      // Strip [Image: ...] context lines when images are shown as thumbnails.
      if (messageImages.length > 0) {
        cleaned = cleaned
          .replace(/\[Image:.*?\]\n(?:Path:.*?\n|Image ID:.*?\n)?/g, '')
          .trim();
      }

      return { displayText: cleaned, reproductionSteps: reproduction };
    }, [messageContent, messageImages]);

    /** Human user row: wrap preview in typographic double quotes. */
    const quotedDisplayText = useMemo(
      () => `\u201c${displayText}\u201d`,
      [displayText],
    );

    // Copy the user message.
    const handleCopy = useCallback(async (e: React.MouseEvent) => {
      e.stopPropagation(); // Prevent toggle via bubbling.
      try {
        await navigator.clipboard.writeText(messageContent);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
      } catch (error) {
        log.error('Failed to copy', error);
      }
    }, [messageContent]);

    const handleRollback = useCallback(async (e: React.MouseEvent) => {
      e.stopPropagation();
      if (!canRollback || !sessionId) return;

      const index = turnIndex + 1;
      const confirmed = await confirmDanger(
        t('message.rollbackDialogTitle', { index }),
        (
          <>
            <p className="confirm-dialog__message-intro">{t('message.rollbackDialogIntro')}</p>
            <ul className="confirm-dialog__bullet-list">
              <li>{t('message.rollbackDialogBulletFiles')}</li>
              <li>{t('message.rollbackDialogBulletHistory')}</li>
            </ul>
          </>
        )
      );
      if (!confirmed) return;

      setIsRollingBack(true);
      try {
        const restoredFiles = await snapshotAPI.rollbackToTurn(sessionId, turnIndex, true);

        // 1) Truncate local dialog turns from this index.
        flowChatStore.truncateDialogTurnsFrom(sessionId, turnIndex);

        // 2) Refresh file tree and open editors.
        const { globalEventBus } = await import('@/infrastructure/event-bus');
        globalEventBus.emit('file-tree:refresh');
        restoredFiles.forEach(filePath => {
          globalEventBus.emit('editor:file-changed', { filePath });
        });

        notificationService.success(t('message.rollbackSuccess'));
      } catch (error) {
        log.error('Rollback failed', error);
        notificationService.error(`${t('message.rollbackFailed')}: ${error instanceof Error ? error.message : String(error)}`);
      } finally {
        setIsRollingBack(false);
      }
    }, [canRollback, sessionId, t, turnIndex]);
    
    // Detect whether the single-line preview is actually truncated.
    useEffect(() => {
      const el = contentRef.current;
      if (!el) return;
      const check = () => setIsTruncated(el.scrollWidth > el.clientWidth);
      check();
      const ro = new ResizeObserver(check);
      ro.observe(el);
      return () => ro.disconnect();
    }, [displayText]);

    const handleToggleExpand = useCallback(() => {
      if (!isTruncated && !expanded) return;
      setExpanded(prev => !prev);
    }, [isTruncated, expanded]);
    
    // Fill content into the input (failed state only).
    const handleFillToInput = useCallback((e: React.MouseEvent) => {
      e.stopPropagation();
      globalEventBus.emit('fill-chat-input', {
        content: messageContent
      });
    }, [messageContent]);
    
    // Collapse when clicking outside.
    useEffect(() => {
      if (!expanded) return;
      
      const handleClickOutside = (e: MouseEvent) => {
        if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
          setExpanded(false);
        }
      };
      
      document.addEventListener('mousedown', handleClickOutside);
      return () => {
        document.removeEventListener('mousedown', handleClickOutside);
      };
    }, [expanded]);

    // Avoid zero-size errors by rendering a placeholder instead of null.
    if (!message) {
      return <div style={{ minHeight: '1px' }} />;
    }
    
    const systemModifier = isSystem ? triggerSourceModifier(message.triggerSource) : '';
    const rootClassName = [
      'user-message-item',
      expanded ? 'user-message-item--expanded' : '',
      isFailed ? 'user-message-item--failed' : '',
      isSystem ? 'user-message-item--system' : 'user-message-item--human',
      isSystem && systemModifier ? `user-message-item--${systemModifier}` : '',
    ].filter(Boolean).join(' ');

    if (isSystem) {
      return (
        <>
        <div className="user-message-item__round-marker">
          <time className="user-message-item__round-time" dateTime={roundMarkerIso}>
            {roundMarkerText}
          </time>
        </div>
        <div ref={containerRef} className={rootClassName}>
          <div
            className="user-message-item__system-row"
            onClick={handleToggleExpand}
            style={{ cursor: (isTruncated || expanded) ? 'pointer' : 'default' }}
            title={(isTruncated || expanded) ? (expanded ? t('message.clickToCollapse') : t('message.clickToExpand')) : undefined}
          >
            {message.triggerSource === 'agent_session' ? (
              <span
                className="user-message-item__agentic-os-icon"
                aria-label={t('session.dispatcher')}
                title={t('session.dispatcher')}
              >
                <Orbit size={14} strokeWidth={2} />
              </span>
            ) : (
              <span className="user-message-item__source-label">
                {triggerSourceLabel(message.triggerSource)}
              </span>
            )}
            <span ref={contentRef} className="user-message-item__system-content">
              {highlightText(displayText, searchQuery ?? '')}
            </span>
            <button
              className={`user-message-item__copy-btn ${copied ? 'copied' : ''}`}
              onClick={e => { e.stopPropagation(); handleCopy(e); }}
              title={copied ? t('message.copyFailed') : t('message.copy')}
            >
              {copied ? <Check size={14} /> : <Copy size={14} />}
            </button>
          </div>
          {expanded && (
            <div className="user-message-item__expanded-body">
              {highlightText(displayText, searchQuery ?? '')}
            </div>
          )}
        </div>
        </>
      );
    }

    return (
      <>
      <div className="user-message-item__round-marker">
        <time className="user-message-item__round-time" dateTime={roundMarkerIso}>
          {roundMarkerText}
        </time>
      </div>
      <div 
        ref={containerRef}
        className={rootClassName}
      >
        {/* Single-line row — same layout as system-triggered messages, with user icon */}
        <div
          className="user-message-item__system-row"
          onClick={handleToggleExpand}
          style={{ cursor: (isTruncated || expanded) ? 'pointer' : 'default' }}
          title={(isTruncated || expanded) ? (expanded ? t('message.clickToCollapse') : t('message.clickToExpand')) : undefined}
        >
          <span className="user-message-item__user-icon" aria-label={t('message.user')}>
            <User size={14} strokeWidth={2} />
          </span>
          <span ref={contentRef} className="user-message-item__system-content">
            {highlightText(quotedDisplayText, searchQuery ?? '')}
          </span>
          <div className="user-message-item__actions" onClick={e => e.stopPropagation()}>
            <button
              className={`user-message-item__copy-btn ${copied ? 'copied' : ''}`}
              onClick={handleCopy}
              title={copied ? t('message.copyFailed') : t('message.copy')}
            >
              {copied ? <Check size={14} /> : <Copy size={14} />}
            </button>
            {isFailed ? (
              <Tooltip content={t('message.fillToInput')}>
                <button
                  className="user-message-item__copy-btn"
                  onClick={handleFillToInput}
                >
                  <ArrowDownToLine size={14} />
                </button>
              </Tooltip>
            ) : (
              <Tooltip content={canRollback ? t('message.rollbackTo', { index: turnIndex + 1 }) : t('message.cannotRollback')}>
                <button
                  className="user-message-item__rollback-btn"
                  onClick={handleRollback}
                  disabled={!canRollback}
                >
                  {isRollingBack ? (
                    <Loader2 size={14} className="user-message-item__rollback-spinner" />
                  ) : (
                    <RotateCcw size={14} />
                  )}
                </button>
              </Tooltip>
            )}
          </div>
        </div>

        {/* Expanded full content */}
        {expanded && (
          <div className="user-message-item__expanded-body">
            {highlightText(quotedDisplayText, searchQuery ?? '')}
          </div>
        )}

        {message.images && message.images.length > 0 && (
          <div className="user-message-item__images">
            {message.images.map(img => {
              const src = img.dataUrl || (img.imagePath ? `https://asset.localhost/${encodeURIComponent(img.imagePath)}` : undefined);
              return src ? (
                <div key={img.id} className="user-message-item__image-thumb" onClick={(e) => { e.stopPropagation(); setLightboxImage(src); }}>
                  <img src={src} alt={img.name} />
                </div>
              ) : null;
            })}
          </div>
        )}

        {reproductionSteps && (
          <div className="user-message-item__blocks">
            {reproductionSteps && <ReproductionStepsBlock steps={reproductionSteps} />}
          </div>
        )}

        {lightboxImage && (
          <div className="user-message-item__lightbox" onClick={() => setLightboxImage(null)}>
            <button className="user-message-item__lightbox-close" onClick={() => setLightboxImage(null)}>
              <X size={20} />
            </button>
            <img src={lightboxImage} alt="Preview" onClick={(e) => e.stopPropagation()} />
          </div>
        )}
      </div>
      </>
    );
  }
);

UserMessageItem.displayName = 'UserMessageItem';

