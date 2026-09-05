import React, { useEffect, useRef, useState } from 'react';
import { MobileButton, MobileIconButton } from '@openbitfun/ui/mobile';
import { useI18n } from '../i18n';
import { useTheme } from '../theme';

interface ChatHeaderProps {
  deviceName?: string;
  displayName: string;
  gitBranch?: string;
  isStreaming: boolean;
  onBack: () => void;
  onCancel: () => void | Promise<void>;
  sessionId: string;
  wideLayout: boolean;
  workspaceName?: string;
}

function truncateMiddle(value: string, maxLength: number): string {
  if (value.length <= maxLength) return value;
  const side = Math.floor((maxLength - 1) / 2);
  return `${value.slice(0, side)}…${value.slice(-side)}`;
}

export default function ChatHeader({
  deviceName,
  displayName,
  gitBranch,
  isStreaming,
  onBack,
  onCancel,
  sessionId,
  wideLayout,
  workspaceName,
}: ChatHeaderProps) {
  const { t } = useI18n();
  const { toggleTheme } = useTheme();
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setMenuOpen(false);
  }, [sessionId]);

  useEffect(() => {
    if (!menuOpen) return;
    const handlePointerDown = (event: PointerEvent) => {
      if (!menuRef.current?.contains(event.target as Node)) setMenuOpen(false);
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setMenuOpen(false);
    };
    document.addEventListener('pointerdown', handlePointerDown);
    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('pointerdown', handlePointerDown);
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [menuOpen]);

  const subtitle = deviceName || workspaceName;

  return (
    <div className="chat-page__header">
      <div className="chat-page__header-row">
        <MobileIconButton
          appearance="floating"
          aria-hidden={wideLayout || undefined}
          aria-label={t('common.back')}
          className="chat-page__back"
          icon={(
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" aria-hidden="true">
              <path d="M5 8H19M5 16H15" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
            </svg>
          )}
          onClick={onBack}
          tabIndex={wideLayout ? -1 : undefined}
        />
        <div className="chat-page__header-center">
          <span className="chat-page__title" title={displayName}>{displayName}</span>
          {subtitle && (
            <div className="chat-page__header-workspace" title={deviceName || workspaceName}>
              <span className="chat-page__workspace-name">{subtitle}</span>
              {!deviceName && gitBranch && (
                <span className="chat-page__workspace-branch" title={gitBranch}>
                  <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                    <line x1="6" x2="6" y1="3" y2="15" />
                    <circle cx="18" cy="6" r="3" />
                    <circle cx="6" cy="18" r="3" />
                    <path d="M18 9a9 9 0 0 1-9 9" />
                  </svg>
                  {truncateMiddle(gitBranch, 28)}
                </span>
              )}
            </div>
          )}
        </div>
        <div className="chat-page__header-right" ref={menuRef}>
          <MobileIconButton
            appearance="floating"
            aria-expanded={menuOpen}
            aria-haspopup="menu"
            aria-label={t('common.more')}
            className="chat-page__theme-btn"
            icon={(
              <svg width="22" height="22" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                <circle cx="5" cy="12" r="1.8" />
                <circle cx="12" cy="12" r="1.8" />
                <circle cx="19" cy="12" r="1.8" />
              </svg>
            )}
            onClick={() => setMenuOpen((open) => !open)}
          />
          {menuOpen && (
            <div className="chat-page__header-menu" role="menu">
              {isStreaming && (
                <MobileButton appearance="plain" block role="menuitem" onClick={() => { setMenuOpen(false); void onCancel(); }}>
                  {t('common.stop')}
                </MobileButton>
              )}
              <MobileButton appearance="plain" block role="menuitem" onClick={() => { setMenuOpen(false); toggleTheme(); }}>
                {t('common.toggleTheme')}
              </MobileButton>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
