import React, { useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import {
  Check,
  CircleHelp,
  Grid2X2,
  Grid3X3,
  Scan,
  Square,
  type LucideIcon,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Tooltip } from '@/component-library';
import { HarnessCreativeIcon } from '@/component-library/icons';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { notificationService } from '@/shared/notification-system';
import { useAnchoredPopoverPosition } from '@/shared/utils/useAnchoredPopoverPosition';
import type { HarnessProfileId as RuntimeHarnessProfileId } from '@/infrastructure/api/service-api/AgentAPI';
import './HarnessProfileSelector.scss';

export type HarnessProfileId = RuntimeHarnessProfileId;
export type KnownHarnessProfileId = 'minimal' | 'balanced' | 'ultimate' | 'creative';
export type SelectableHarnessProfileId = 'minimal' | 'balanced';

interface HarnessProfileSelectorProps {
  /** Session still runs the legacy agent mode and cannot switch. */
  legacySession?: boolean;
  /** The Session has accepted its first runtime Turn, so its Harness is fixed. */
  sessionStarted?: boolean;
  selectedProfile: HarnessProfileId;
  disabled?: boolean;
  onSelectProfile: (profileId: SelectableHarnessProfileId) => void | Promise<void>;
}

const PROFILE_IDS: KnownHarnessProfileId[] = ['minimal', 'balanced', 'ultimate', 'creative'];
type DensityHarnessProfileId = Exclude<KnownHarnessProfileId, 'creative'>;

/** The density step represented by each Harness Profile. */
const PROFILE_GEARS: Record<DensityHarnessProfileId, 1 | 2 | 3> = {
  minimal: 1,
  balanced: 2,
  ultimate: 3,
};

const PROFILE_DENSITY_ICONS: Record<DensityHarnessProfileId, LucideIcon> = {
  minimal: Scan,
  balanced: Grid2X2,
  ultimate: Grid3X3,
};

function isDensityProfile(profile: KnownHarnessProfileId): profile is DensityHarnessProfileId {
  return profile !== 'creative';
}

function isSelectableProfile(
  profile: KnownHarnessProfileId,
): profile is SelectableHarnessProfileId {
  return profile === 'minimal' || profile === 'balanced';
}

function isProfileInDevelopment(
  profile: KnownHarnessProfileId,
): profile is Exclude<KnownHarnessProfileId, SelectableHarnessProfileId> {
  return !isSelectableProfile(profile);
}

/**
 * The execution gears use progressively denser frames around the same Agent
 * core. Creative branches from that scale with its own BitFun grid-and-brush
 * mark, redrawn from the supplied reference.
 */
function HarnessDensityMark({
  profile,
  compact = false,
}: {
  profile?: KnownHarnessProfileId;
  compact?: boolean;
}): React.ReactElement {
  const densityProfile = profile && isDensityProfile(profile) ? profile : undefined;
  const DensityIcon = densityProfile ? PROFILE_DENSITY_ICONS[densityProfile] : CircleHelp;

  return (
    <span
      className="bitfun-harness-selector__density-mark"
      data-harness-profile={profile ?? 'unknown'}
      data-harness-density={densityProfile ? PROFILE_GEARS[densityProfile] : 0}
      data-size={compact ? 'compact' : 'option'}
      aria-hidden
    >
      {profile === 'creative' ? (
        <HarnessCreativeIcon
          className="bitfun-harness-selector__density-frame"
          size={compact ? 15 : 26}
        />
      ) : (
        <DensityIcon
          className="bitfun-harness-selector__density-frame"
          size={compact ? 15 : 26}
          strokeWidth={compact ? 1.55 : 1.4}
        />
      )}
      {densityProfile && (
        <Square
          className="bitfun-harness-selector__density-core"
          size={compact ? 5 : 8}
          strokeWidth={0}
          fill="currentColor"
        />
      )}
    </span>
  );
}

/**
 * Harness picker embedded in the composer capsule.
 *
 * It sits opposite the model control: the left end of the capsule says how the
 * next turn runs, the right end says what runs it. The three execution gears
 * share one Agent core; Creative uses its registered grid-and-brush mark.
 */
export const HarnessProfileSelector: React.FC<HarnessProfileSelectorProps> = ({
  legacySession = false,
  sessionStarted = false,
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

  const handleSelect = useCallback((profileId: KnownHarnessProfileId) => {
    if (legacySession) {
      notificationService.info(t('chatInput.harness.legacySessionNotice'), { duration: 3800 });
      close();
      return;
    }
    if (profileId === selectedProfile && !isProfileInDevelopment(profileId)) {
      close();
      return;
    }
    if (isProfileInDevelopment(profileId)) {
      const name = t(`chatInput.harness.profiles.${profileId}.name`);
      notificationService.info(t('chatInput.harness.comingSoonNotice', { name }), {
        duration: 3200,
      });
      close();
      return;
    }
    if (sessionStarted) {
      notificationService.info(t('chatInput.harness.sessionStartedNotice'), { duration: 3800 });
      close();
      return;
    }
    void onSelectProfile(profileId);
    close();
  }, [close, legacySession, onSelectProfile, selectedProfile, sessionStarted, t]);

  const knownSelectedProfile = PROFILE_IDS.find(id => id === selectedProfile);
  const selectedProfileAvailable = Boolean(
    knownSelectedProfile && isSelectableProfile(knownSelectedProfile),
  );
  const gear = knownSelectedProfile && isDensityProfile(knownSelectedProfile)
    ? PROFILE_GEARS[knownSelectedProfile]
    : 0;
  const triggerLabel = legacySession
    ? t('chatInput.harness.compatibilityShort')
    : knownSelectedProfile
      ? t(`chatInput.harness.profiles.${knownSelectedProfile}.name`)
      : t('chatInput.harness.unsupportedProfile', { id: selectedProfile });
  const triggerTooltip = legacySession
    ? t('chatInput.harness.legacySessionNotice')
    : sessionStarted
      ? t('chatInput.harness.sessionStartedNotice')
      : selectedProfileAvailable
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
          data-harness-locked={sessionStarted ? 'true' : undefined}
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
          {!legacySession && (
            <HarnessDensityMark profile={knownSelectedProfile} compact />
          )}
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
          data-harness-locked={sessionStarted ? 'true' : undefined}
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
            const connected =
              isSelectableProfile(id)
              && id === selectedProfile
              && !legacySession;
            const state = connected
              ? 'current'
              : isProfileInDevelopment(id)
                ? 'coming-soon'
                : sessionStarted
                  ? 'new-session-only'
                  : 'available';
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
                data-bf-state={state}
                onClick={() => handleSelect(id)}
                data-testid={`harness-profile-${id}`}
              >
                <HarnessDensityMark profile={id} />
                <span className="bitfun-harness-selector__profile-copy">
                  <span className="bitfun-harness-selector__profile-name">{name}</span>
                  <span className="bitfun-harness-selector__profile-promise">
                    {t(`chatInput.harness.profiles.${id}.promise`)}
                  </span>
                </span>
                <span className="bitfun-harness-selector__profile-status">
                  {connected ? (
                    <Check size={13} strokeWidth={2.4} aria-hidden />
                  ) : isProfileInDevelopment(id) ? (
                    t('chatInput.harness.comingSoon')
                  ) : sessionStarted ? (
                    t('chatInput.harness.newSessionOnly')
                  ) : (
                    null
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
