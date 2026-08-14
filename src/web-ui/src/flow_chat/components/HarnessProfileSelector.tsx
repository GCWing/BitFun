import React, { useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Check, ChevronDown } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Tooltip } from '@/component-library';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { notificationService } from '@/shared/notification-system';
import { useAnchoredPopoverPosition } from '@/shared/utils/useAnchoredPopoverPosition';
import './HarnessProfileSelector.scss';

export type HarnessProfileId = 'minimal' | 'balanced' | 'ultimate';

interface HarnessProfileSelectorProps {
  /** Session still runs the legacy agent mode and cannot switch. */
  legacySession?: boolean;
  /** The Balanced harness is already the session's active mode. */
  active?: boolean;
  onActivateBalanced: () => void;
}

const PROFILE_IDS: HarnessProfileId[] = ['minimal', 'balanced', 'ultimate'];

/**
 * Compact Harness picker for the chat-input strip.
 *
 * The three-tier selection stays, but the popup is a plain list: one line per
 * profile — name plus a trailing state (check / coming soon / new sessions
 * only). No header, icons, descriptions, or footer.
 */
export const HarnessProfileSelector: React.FC<HarnessProfileSelectorProps> = ({
  legacySession = false,
  active = false,
  onActivateBalanced,
}) => {
  const { t } = useTranslation('flow-chat');
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const menuLayout = useAnchoredPopoverPosition({
    open,
    anchorRef: triggerRef,
    popoverRef: menuRef,
    preferredPlacement: 'top',
    gap: 6,
    layoutRevision: legacySession ? 1 : 0,
  });

  const close = useCallback(() => setOpen(false), []);

  useEffect(() => {
    if (!open) return;

    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target as Node | null;
      if (!target || triggerRef.current?.contains(target) || menuRef.current?.contains(target)) {
        return;
      }
      close();
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') close();
    };

    document.addEventListener('pointerdown', handlePointerDown);
    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('pointerdown', handlePointerDown);
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [close, open]);

  const handleSelect = useCallback((profileId: HarnessProfileId) => {
    if (profileId !== 'balanced') {
      const name = t(`chatInput.harness.profiles.${profileId}.name`);
      notificationService.info(t('chatInput.harness.comingSoonNotice', { name }), {
        duration: 3200,
      });
      close();
      return;
    }
    if (legacySession) {
      notificationService.info(t('chatInput.harness.legacySessionNotice'), { duration: 3800 });
      close();
      return;
    }
    onActivateBalanced();
    close();
  }, [close, legacySession, onActivateBalanced, t]);

  const triggerLabel = legacySession
    ? t('chatInput.harness.compatibilityShort')
    : t('chatInput.harness.profiles.balanced.name');
  const triggerTooltip = legacySession
    ? t('chatInput.harness.legacySessionNotice')
    : active
      ? t('chatInput.harness.selectorTooltip')
      : t('chatInput.harness.activateBalanced');

  return (
    <div className="bitfun-harness-selector" data-bf-component="harness-selector" data-bf-part="root">
      <Tooltip content={triggerTooltip}>
        <button
          ref={triggerRef}
          type="button"
          className="bitfun-harness-selector__trigger"
          data-bf-component="harness-selector"
          data-bf-part="trigger"
          data-bf-state={open ? 'open' : undefined}
          aria-haspopup="menu"
          aria-expanded={open}
          onClick={(event) => {
            event.stopPropagation();
            setOpen(value => !value);
          }}
          data-testid="harness-profile-selector"
        >
          <span className="bitfun-harness-selector__trigger-value">
            {triggerLabel}
            <ChevronDown size={11} strokeWidth={2} aria-hidden />
          </span>
        </button>
      </Tooltip>

      {open && createPortal(
        <div
          ref={menuRef}
          className="bitfun-harness-selector__menu"
          data-bf-component="harness-selector"
          data-bf-part="menu"
          data-bf-state="open"
          data-bf-placement={menuLayout?.placement ?? 'top'}
          role="menu"
          style={{
            top: `${menuLayout?.top ?? 0}px`,
            left: `${menuLayout?.left ?? 0}px`,
            visibility: menuLayout ? 'visible' : 'hidden',
          }}
          onMouseDown={event => event.stopPropagation()}
        >
          {PROFILE_IDS.map((id) => {
            const name = t(`chatInput.harness.profiles.${id}.name`);
            const connected = id === 'balanced' && !legacySession;
            return (
              <button
                key={id}
                type="button"
                role="menuitemradio"
                aria-checked={connected}
                className={`bitfun-harness-selector__profile${connected ? ' is-current' : ''}`}
                data-bf-component="harness-selector"
                data-bf-part="profile"
                data-bf-profile={id}
                data-bf-state={connected ? 'current' : id === 'balanced' ? 'new-session' : 'coming-soon'}
                onClick={() => handleSelect(id)}
                data-testid={`harness-profile-${id}`}
              >
                <span className="bitfun-harness-selector__profile-name">{name}</span>
                <span className="bitfun-harness-selector__profile-status">
                  {connected ? (
                    <Check size={13} strokeWidth={2.4} aria-hidden />
                  ) : id === 'balanced' ? (
                    t('chatInput.harness.newSessionOnly')
                  ) : (
                    t('chatInput.harness.comingSoon')
                  )}
                </span>
              </button>
            );
          })}
        </div>,
        getAppearanceOverlayHost(),
      )}
    </div>
  );
};

export default HarnessProfileSelector;
