import React, { useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Check } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Tooltip } from '@/component-library';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { notificationService } from '@/shared/notification-system';
import { useAnchoredPopoverPosition } from '@/shared/utils/useAnchoredPopoverPosition';
import type { HarnessProfileId as RuntimeHarnessProfileId } from '@/infrastructure/api/service-api/AgentAPI';
import './HarnessProfileSelector.scss';

export type HarnessProfileId = RuntimeHarnessProfileId;
export type SelectableHarnessProfileId = 'minimal' | 'balanced' | 'ultimate';

interface HarnessProfileSelectorProps {
  /** Session still runs the legacy agent mode and cannot switch. */
  legacySession?: boolean;
  selectedProfile: HarnessProfileId;
  disabled?: boolean;
  onSelectProfile: (profileId: SelectableHarnessProfileId) => void | Promise<void>;
}

const PROFILE_IDS: SelectableHarnessProfileId[] = ['minimal', 'balanced', 'ultimate'];

/**
 * How many of the three gauge segments a gear fills. The gauge is the primary
 * reading of the control: intensity is expressed by shape, so the label only
 * names what the shape already says and can be dropped on a narrow composer.
 */
const PROFILE_GEARS: Record<SelectableHarnessProfileId, 1 | 2 | 3> = {
  minimal: 1,
  balanced: 2,
  ultimate: 3,
};

const GAUGE_SEGMENTS = [1, 2, 3] as const;

function HarnessGauge({ gear }: { gear: 0 | 1 | 2 | 3 }): React.ReactElement {
  return (
    <span className="bitfun-harness-selector__gauge" data-gear={gear} aria-hidden>
      {GAUGE_SEGMENTS.map(segment => (
        <span
          key={segment}
          className="bitfun-harness-selector__gauge-bar"
          data-filled={segment <= gear ? 'true' : 'false'}
        />
      ))}
    </span>
  );
}

/**
 * Harness picker embedded in the composer capsule.
 *
 * It sits opposite the model control: the left end of the capsule says how the
 * next turn runs, the right end says what runs it. The popup stays a plain
 * three-row list — gauge, name, and the promise the gear makes.
 */
export const HarnessProfileSelector: React.FC<HarnessProfileSelectorProps> = ({
  legacySession = false,
  selectedProfile,
  disabled = false,
  onSelectProfile,
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
    alignment: 'start',
    gap: 8,
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

  const handleSelect = useCallback((profileId: SelectableHarnessProfileId) => {
    if (profileId === 'ultimate') {
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
    void onSelectProfile(profileId);
    close();
  }, [close, legacySession, onSelectProfile, t]);

  const knownSelectedProfile = PROFILE_IDS.find(id => id === selectedProfile);
  const gear = knownSelectedProfile ? PROFILE_GEARS[knownSelectedProfile] : 0;
  const triggerLabel = legacySession
    ? t('chatInput.harness.compatibilityShort')
    : knownSelectedProfile
      ? t(`chatInput.harness.profiles.${knownSelectedProfile}.name`)
      : t('chatInput.harness.unsupportedProfile', { id: selectedProfile });
  const triggerTooltip = legacySession
    ? t('chatInput.harness.legacySessionNotice')
    : knownSelectedProfile
      ? t('chatInput.harness.selectorTooltip', { name: triggerLabel })
      : t('chatInput.harness.unsupportedProfileNotice', { id: selectedProfile });

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
          data-harness-gear={gear}
          data-harness-legacy={legacySession ? 'true' : undefined}
          aria-haspopup="menu"
          aria-expanded={open}
          aria-label={triggerTooltip}
          disabled={disabled}
          onClick={(event) => {
            event.stopPropagation();
            setOpen(value => !value);
          }}
          data-testid="harness-profile-selector"
        >
          <HarnessGauge gear={gear} />
          <span className="bitfun-harness-selector__trigger-value">
            {triggerLabel}
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
            const connected = id === selectedProfile && !legacySession;
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
                data-bf-state={connected ? 'current' : id === 'ultimate' ? 'coming-soon' : 'available'}
                onClick={() => handleSelect(id)}
                data-testid={`harness-profile-${id}`}
              >
                <HarnessGauge gear={PROFILE_GEARS[id]} />
                <span className="bitfun-harness-selector__profile-copy">
                  <span className="bitfun-harness-selector__profile-name">{name}</span>
                  <span className="bitfun-harness-selector__profile-promise">
                    {t(`chatInput.harness.profiles.${id}.promise`)}
                  </span>
                </span>
                <span className="bitfun-harness-selector__profile-status">
                  {connected ? (
                    <Check size={13} strokeWidth={2.4} aria-hidden />
                  ) : id !== 'ultimate' ? (
                    null
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
