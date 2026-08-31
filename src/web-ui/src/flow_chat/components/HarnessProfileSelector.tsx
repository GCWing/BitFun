import React, { useCallback, useEffect, useId, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import {
  Bot,
  Check,
  ChevronLeft,
  ChevronRight,
  Grid2X2,
  Grid3X3,
  Scan,
  Square,
  type LucideIcon,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Menu, MenuItem, MenuSection, MenuSeparator } from '@bitfun/ui';
import { Tooltip } from '@/component-library';
import { HarnessCreativeIcon } from '@/component-library/icons';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { notificationService } from '@/shared/notification-system';
import { useAnchoredPopoverPosition } from '@/shared/utils/useAnchoredPopoverPosition';
import { useSideAnchoredPopoverPosition } from '@/shared/utils/useSideAnchoredPopoverPosition';
import './HarnessProfileSelector.scss';

export type HarnessProfileId = KnownHarnessProfileId | (string & {});
export type KnownHarnessProfileId =
  | 'minimal'
  | 'balanced'
  | 'ultimate'
  | 'creative'
  | 'other';
export type SelectableHarnessProfileId = 'minimal' | 'balanced' | 'ultimate' | 'creative';

export interface HarnessAgentOption {
  id: string;
  name: string;
  available?: boolean;
}

export type HarnessNewSessionSelection =
  | { kind: 'profile'; id: SelectableHarnessProfileId }
  | { kind: 'agent'; id: string };

export type HarnessProfileSelectorPresentation = 'standalone' | 'menu-item';

interface HarnessProfileSelectorProps {
  /** Session still runs a legacy fixed mode and cannot switch. */
  legacySession?: boolean;
  /** The Session has accepted its first runtime Turn, so Harness and main Agent are fixed. */
  sessionStarted?: boolean;
  selectedProfile: HarnessProfileId;
  selectedAgentId?: string;
  otherAgents?: HarnessAgentOption[];
  disabled?: boolean;
  /**
   * `standalone` anchors the picker to its own floating trigger. `menu-item`
   * turns that trigger into a row inside a parent action menu and opens the
   * secondary picker beside it in the shared overlay layer.
   */
  presentation?: HarnessProfileSelectorPresentation;
  onSelectProfile: (profileId: SelectableHarnessProfileId) => void | Promise<void>;
  onSelectAgent?: (agentId: string) => void | Promise<void>;
  onStartNewSession?: (
    selection: HarnessNewSessionSelection,
  ) => void | Promise<void>;
  /** Lets a parent action menu close after a terminal selection. */
  onSelectionComplete?: () => void;
}

const PROFILE_IDS: KnownHarnessProfileId[] = [
  'minimal',
  'balanced',
  'ultimate',
  'creative',
  'other',
];
type DensityHarnessProfileId = 'minimal' | 'balanced' | 'ultimate';

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
  return profile === 'minimal' || profile === 'balanced' || profile === 'ultimate';
}

function isSelectableProfile(
  profile: KnownHarnessProfileId,
): profile is SelectableHarnessProfileId {
  return isDensityProfile(profile) || profile === 'creative';
}

function sameAgent(left: string | null | undefined, right: string | null | undefined): boolean {
  return left?.trim().toLowerCase() === right?.trim().toLowerCase();
}

/** Menu rows and the compact add-menu trigger share one mode mark. */
function HarnessProfileMark({
  profile,
}: {
  profile: KnownHarnessProfileId;
}): React.ReactElement {
  const densityProfile = isDensityProfile(profile) ? profile : undefined;
  const DensityIcon = densityProfile ? PROFILE_DENSITY_ICONS[densityProfile] : null;

  return (
    <span
      className="bitfun-harness-selector__density-mark"
      data-harness-profile={profile}
      data-harness-density={densityProfile ? PROFILE_GEARS[densityProfile] : 0}
      aria-hidden
    >
      {profile === 'creative' ? (
        <HarnessCreativeIcon
          className="bitfun-harness-selector__density-frame"
          size={16}
        />
      ) : profile === 'other' ? (
        <Bot
          className="bitfun-harness-selector__density-frame"
          size={15}
          strokeWidth={1.45}
        />
      ) : DensityIcon ? (
        <DensityIcon
          className="bitfun-harness-selector__density-frame"
          size={16}
          strokeWidth={1.4}
        />
      ) : null}
      {densityProfile && (
        <Square
          className="bitfun-harness-selector__density-core"
          size={5}
          strokeWidth={0}
          fill="currentColor"
        />
      )}
    </span>
  );
}

/**
 * Before the first Turn this is the Session execution picker. Afterwards it
 * becomes a lightweight Session signature whose menu choices start a new
 * Session directly. ChatInput presents the signature as a disclosure row
 * inside its add menu; other consumers may keep the standalone trigger.
 */
export const HarnessProfileSelector: React.FC<HarnessProfileSelectorProps> = ({
  legacySession = false,
  sessionStarted = false,
  selectedProfile,
  selectedAgentId,
  otherAgents = [],
  disabled = false,
  presentation = 'standalone',
  onSelectProfile,
  onSelectAgent,
  onStartNewSession,
  onSelectionComplete,
}) => {
  const { t } = useTranslation('flow-chat');
  const fixedSession = legacySession || sessionStarted;
  const [open, setOpen] = useState(false);
  const [page, setPage] = useState<'profiles' | 'agents'>('profiles');
  const menuId = useId();
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const menuLayout = useAnchoredPopoverPosition({
    open: open && presentation === 'standalone',
    anchorRef: triggerRef,
    popoverRef: menuRef,
    preferredPlacement: 'top',
    alignment: 'start',
    gap: 8,
    layoutRevision: `${fixedSession ? 1 : 0}:${page}:${otherAgents.length}`,
  });
  const sideMenuLayout = useSideAnchoredPopoverPosition({
    open: open && presentation === 'menu-item',
    anchorRef: triggerRef,
    popoverRef: menuRef,
    layoutRevision: `${fixedSession ? 1 : 0}:${page}:${otherAgents.length}`,
  });

  const close = useCallback(() => {
    setOpen(false);
    setPage('profiles');
  }, []);

  const finishSelection = useCallback(() => {
    close();
    onSelectionComplete?.();
  }, [close, onSelectionComplete]);

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

  const handleSelectProfile = useCallback((profileId: KnownHarnessProfileId) => {
    if (profileId === 'other') {
      setPage('agents');
      return;
    }
    if (fixedSession) {
      if (isSelectableProfile(profileId)) {
        void onStartNewSession?.({ kind: 'profile', id: profileId });
      }
      finishSelection();
      return;
    }
    if (profileId === selectedProfile) {
      finishSelection();
      return;
    }
    if (isSelectableProfile(profileId)) {
      void onSelectProfile(profileId);
    }
    finishSelection();
  }, [finishSelection, fixedSession, onSelectProfile, onStartNewSession, selectedProfile]);

  const handleSelectAgent = useCallback((agent: HarnessAgentOption) => {
    if (agent.available === false) {
      notificationService.info(t('chatInput.harness.agentUnavailable', { name: agent.name }), {
        duration: 3200,
      });
      return;
    }
    if (fixedSession) {
      void onStartNewSession?.({ kind: 'agent', id: agent.id });
      finishSelection();
      return;
    }
    const connected = selectedProfile === 'other' && sameAgent(agent.id, selectedAgentId);
    if (!connected) {
      void onSelectAgent?.(agent.id);
    }
    finishSelection();
  }, [finishSelection, fixedSession, onSelectAgent, onStartNewSession, selectedAgentId, selectedProfile, t]);

  const knownSelectedProfile = PROFILE_IDS.find(id => id === selectedProfile);
  const selectedAgent = otherAgents.find(agent => sameAgent(agent.id, selectedAgentId));
  const selectedProfileAvailable = Boolean(
    knownSelectedProfile
      && (
        isSelectableProfile(knownSelectedProfile)
        || (
          knownSelectedProfile === 'other'
          && selectedAgent
          && selectedAgent.available !== false
        )
      ),
  );
  const primaryLabel = legacySession
    ? t('chatInput.harness.compatibilityShort')
    : knownSelectedProfile === 'other'
      ? selectedAgent?.name || selectedAgentId || t('chatInput.harness.profiles.other.name')
      : knownSelectedProfile
        ? t(`chatInput.harness.profiles.${knownSelectedProfile}.name`)
        : t('chatInput.harness.unsupportedProfile', { id: selectedProfile });
  const triggerLabel = primaryLabel;
  const triggerTooltip = legacySession
    ? t('chatInput.harness.legacySessionNotice')
    : !selectedProfileAvailable
      ? t('chatInput.harness.unsupportedProfileNotice', { id: selectedProfile })
      : sessionStarted
        ? t('chatInput.harness.fixedTooltip', { name: primaryLabel })
        : t('chatInput.harness.selectorTooltip', { name: primaryLabel });
  const triggerState = [open ? 'open' : '', fixedSession ? 'fixed' : '']
    .filter(Boolean)
    .join(' ') || undefined;
  const creatingNewSession = fixedSession;
  const handleTriggerClick = (event: React.MouseEvent<HTMLButtonElement>) => {
    event.stopPropagation();
    setOpen(value => {
      if (!value) setPage('profiles');
      return !value;
    });
  };
  const handleTriggerKeyDown = (event: React.KeyboardEvent<HTMLButtonElement>) => {
    if (presentation !== 'menu-item' || event.key !== 'ArrowRight') return;
    event.preventDefault();
    event.stopPropagation();
    if (!open) setPage('profiles');
    setOpen(true);
  };
  const handleMenuKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== 'Escape' && !(presentation === 'menu-item' && event.key === 'ArrowLeft')) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    close();
    requestAnimationFrame(() => triggerRef.current?.focus());
  };

  return (
    <div
      className={`bitfun-harness-selector bitfun-harness-selector--${presentation}`}
      data-bf-component="harness-selector"
      data-bf-part="root"
      data-bf-presentation={presentation}
    >
      <Tooltip content={triggerTooltip}>
        {presentation === 'menu-item' ? (
          <MenuItem
            ref={triggerRef}
            data-bf-component="harness-selector"
            data-bf-part="trigger"
            data-bf-state={triggerState}
            data-harness-legacy={legacySession ? 'true' : undefined}
            data-harness-locked={sessionStarted ? 'true' : undefined}
            data-harness-fixed={fixedSession ? 'true' : undefined}
            aria-haspopup="menu"
            aria-expanded={open}
            aria-controls={menuId}
            aria-label={triggerTooltip}
            disabled={disabled}
            onClick={handleTriggerClick}
            onKeyDown={handleTriggerKeyDown}
            leading={knownSelectedProfile
              ? <HarnessProfileMark profile={knownSelectedProfile} />
              : <Bot size={15} strokeWidth={1.45} aria-hidden />}
            metadata={(
              <ChevronRight
                className="bitfun-harness-selector__trigger-chevron"
                size={14}
                strokeWidth={1.8}
                aria-hidden
              />
            )}
            data-testid="harness-profile-selector"
          >
            {triggerLabel}
          </MenuItem>
        ) : (
          <button
            ref={triggerRef}
            type="button"
            className="bitfun-harness-selector__trigger"
            data-bf-component="harness-selector"
            data-bf-part="trigger"
            data-bf-state={triggerState}
            data-harness-legacy={legacySession ? 'true' : undefined}
            data-harness-locked={sessionStarted ? 'true' : undefined}
            data-harness-fixed={fixedSession ? 'true' : undefined}
            aria-haspopup="menu"
            aria-expanded={open}
            aria-label={triggerTooltip}
            disabled={disabled}
            onClick={handleTriggerClick}
            data-testid="harness-profile-selector"
          >
            <span className="bitfun-harness-selector__trigger-value">{triggerLabel}</span>
          </button>
        )}
      </Tooltip>

      {open && createPortal(
        <Menu
          ref={menuRef}
          id={menuId}
          className="bitfun-harness-selector__menu"
          data-bf-component="harness-selector"
          data-bf-part="menu"
          data-bf-state="open"
          data-bf-page={page}
          data-bf-placement={presentation === 'menu-item'
            ? 'side'
            : menuLayout?.placement ?? 'top'}
          data-harness-locked={sessionStarted ? 'true' : undefined}
          data-harness-fixed={fixedSession ? 'true' : undefined}
          style={presentation === 'menu-item'
            ? {
                top: sideMenuLayout?.top ?? 0,
                left: sideMenuLayout?.left ?? 0,
                visibility: sideMenuLayout ? 'visible' : 'hidden',
              }
            : {
                top: `${menuLayout?.top ?? 0}px`,
                left: `${menuLayout?.left ?? 0}px`,
                visibility: menuLayout ? 'visible' : 'hidden',
              }}
          autoFocusFirstItem
          onMouseDown={event => event.stopPropagation()}
          onKeyDown={handleMenuKeyDown}
        >
          <MenuSection
            title={fixedSession ? (
              <span
                data-bf-component="harness-selector"
                data-bf-part="newSessionNotice"
                data-testid="harness-new-session-notice"
              >
                {t('chatInput.harness.selectionCreatesNewSession')}
              </span>
            ) : undefined}
          >
            {page === 'profiles' ? (
              <>
                {PROFILE_IDS.map((id) => {
                  const name = t(`chatInput.harness.profiles.${id}.name`);
                  const connected = !creatingNewSession
                    && id === selectedProfile
                    && !legacySession;
                  const state = connected
                    ? 'current'
                    : 'available';
                  return (
                    <span
                      key={id}
                      className="bitfun-harness-selector__row-contract"
                      data-bf-component="harness-selector"
                      data-bf-part="profile"
                      data-bf-profile={id}
                      data-bf-state={state}
                    >
                      <MenuItem
                        role={creatingNewSession ? 'menuitem' : 'menuitemradio'}
                        checked={!creatingNewSession && connected}
                        data-bf-profile={id}
                        data-bf-state={state}
                        leading={<HarnessProfileMark profile={id} />}
                        metadata={(
                          <span className="bitfun-harness-selector__profile-status">
                            {connected ? <Check size={13} strokeWidth={2.4} aria-hidden /> : null}
                            {id === 'other' ? (
                              <>
                                <span className="bitfun-harness-selector__agent-count">
                                  {otherAgents.length}
                                </span>
                                <ChevronRight size={14} strokeWidth={1.8} aria-hidden />
                              </>
                            ) : null}
                          </span>
                        )}
                        onClick={() => handleSelectProfile(id)}
                        data-testid={`harness-profile-${id}`}
                      >
                        {name}
                      </MenuItem>
                    </span>
                  );
                })}
              </>
            ) : (
              <>
                <MenuItem
                  leading={<ChevronLeft size={14} strokeWidth={1.8} aria-hidden />}
                  onClick={() => setPage('profiles')}
                  data-testid="harness-agent-back"
                >
                  {t('chatInput.harness.otherAgentsTitle')}
                </MenuItem>
                <MenuSeparator />
                {otherAgents.length === 0 ? (
                  <div className="bitfun-harness-selector__empty">
                    {t('chatInput.harness.otherAgentsEmpty')}
                  </div>
                ) : otherAgents.map(agent => {
                  const connected = !creatingNewSession
                    && selectedProfile === 'other'
                    && sameAgent(agent.id, selectedAgentId);
                  const state = connected
                    ? 'current'
                    : agent.available === false
                      ? 'unavailable'
                      : 'available';
                  return (
                    <span
                      key={agent.id}
                      className="bitfun-harness-selector__row-contract"
                      data-bf-component="harness-selector"
                      data-bf-part="agent"
                      data-bf-agent-id={agent.id}
                      data-bf-state={state}
                    >
                      <MenuItem
                        role={creatingNewSession ? 'menuitem' : 'menuitemradio'}
                        checked={!creatingNewSession && connected}
                        data-bf-agent-id={agent.id}
                        data-bf-state={state}
                        leading={<Bot size={15} strokeWidth={1.55} aria-hidden />}
                        metadata={(
                          <span className="bitfun-harness-selector__profile-status">
                            {connected ? <Check size={13} strokeWidth={2.4} aria-hidden /> : null}
                            {agent.available === false
                              ? t('chatInput.harness.unavailable')
                              : null}
                          </span>
                        )}
                        onClick={() => handleSelectAgent(agent)}
                        data-testid={`harness-agent-${agent.id}`}
                      >
                        {agent.name}
                      </MenuItem>
                    </span>
                  );
                })}
              </>
            )}
          </MenuSection>
        </Menu>,
        getAppearanceOverlayHost(),
      )}
    </div>
  );
};

export default HarnessProfileSelector;
