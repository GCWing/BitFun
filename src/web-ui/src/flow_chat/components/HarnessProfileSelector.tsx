import React, { useCallback, useEffect, useRef, useState } from 'react';
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
import { Tooltip } from '@/component-library';
import { HarnessCreativeIcon } from '@/component-library/icons';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { notificationService } from '@/shared/notification-system';
import { useAnchoredPopoverPosition } from '@/shared/utils/useAnchoredPopoverPosition';
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

interface HarnessProfileSelectorProps {
  /** Session still runs a legacy fixed mode and cannot switch. */
  legacySession?: boolean;
  /** The Session has accepted its first runtime Turn, so Harness and main Agent are fixed. */
  sessionStarted?: boolean;
  selectedProfile: HarnessProfileId;
  selectedAgentId?: string;
  directiveLabel?: string;
  otherAgents?: HarnessAgentOption[];
  disabled?: boolean;
  onSelectProfile: (profileId: SelectableHarnessProfileId) => void | Promise<void>;
  onSelectAgent?: (agentId: string) => void | Promise<void>;
  onStartNewSession?: (
    selection: HarnessNewSessionSelection,
  ) => void | Promise<void>;
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

/** Icons stay in the menu; the ChatInput trigger remains text-only. */
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
          size={26}
        />
      ) : profile === 'other' ? (
        <Bot
          className="bitfun-harness-selector__density-frame"
          size={24}
          strokeWidth={1.45}
        />
      ) : DensityIcon ? (
        <DensityIcon
          className="bitfun-harness-selector__density-frame"
          size={26}
          strokeWidth={1.4}
        />
      ) : null}
      {densityProfile && (
        <Square
          className="bitfun-harness-selector__density-core"
          size={8}
          strokeWidth={0}
          fill="currentColor"
        />
      )}
    </span>
  );
}

/**
 * Before the first Turn this is the Session execution picker. Afterwards it
 * becomes a lightweight Session signature; alternative choices are disclosed only
 * through the explicit new-Session action. Per-task directives remain in the
 * adjacent add menu.
 */
export const HarnessProfileSelector: React.FC<HarnessProfileSelectorProps> = ({
  legacySession = false,
  sessionStarted = false,
  selectedProfile,
  selectedAgentId,
  directiveLabel,
  otherAgents = [],
  disabled = false,
  onSelectProfile,
  onSelectAgent,
  onStartNewSession,
}) => {
  const { t } = useTranslation('flow-chat');
  const fixedSession = legacySession || sessionStarted;
  const [open, setOpen] = useState(false);
  const [page, setPage] = useState<'summary' | 'profiles' | 'agents'>(
    fixedSession ? 'summary' : 'profiles',
  );
  const previousFixedSessionRef = useRef(fixedSession);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const menuLayout = useAnchoredPopoverPosition({
    open,
    anchorRef: triggerRef,
    popoverRef: menuRef,
    preferredPlacement: 'top',
    alignment: 'start',
    gap: 8,
    layoutRevision: `${fixedSession ? 1 : 0}:${page}:${otherAgents.length}`,
  });

  const close = useCallback(() => {
    setOpen(false);
    setPage(fixedSession ? 'summary' : 'profiles');
  }, [fixedSession]);

  useEffect(() => {
    const becameFixed = fixedSession && !previousFixedSessionRef.current;
    previousFixedSessionRef.current = fixedSession;
    if (becameFixed) {
      setPage('summary');
    }
  }, [fixedSession]);

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
      close();
      return;
    }
    if (profileId === selectedProfile) {
      close();
      return;
    }
    if (isSelectableProfile(profileId)) {
      void onSelectProfile(profileId);
    }
    close();
  }, [close, fixedSession, onSelectProfile, onStartNewSession, selectedProfile]);

  const handleSelectAgent = useCallback((agent: HarnessAgentOption) => {
    if (agent.available === false) {
      notificationService.info(t('chatInput.harness.agentUnavailable', { name: agent.name }), {
        duration: 3200,
      });
      return;
    }
    if (fixedSession) {
      void onStartNewSession?.({ kind: 'agent', id: agent.id });
      close();
      return;
    }
    const connected = selectedProfile === 'other' && sameAgent(agent.id, selectedAgentId);
    if (!connected) {
      void onSelectAgent?.(agent.id);
    }
    close();
  }, [close, fixedSession, onSelectAgent, onStartNewSession, selectedAgentId, selectedProfile, t]);

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
  const triggerLabel = directiveLabel
    ? `${primaryLabel} · ${directiveLabel}`
    : primaryLabel;
  const triggerTooltip = legacySession
    ? t('chatInput.harness.legacySessionNotice')
    : !selectedProfileAvailable
      ? t('chatInput.harness.unsupportedProfileNotice', { id: selectedProfile })
      : sessionStarted
        ? directiveLabel
          ? t('chatInput.harness.fixedTooltipWithDirective', {
              name: primaryLabel,
              directive: directiveLabel,
            })
          : t('chatInput.harness.fixedTooltip', { name: primaryLabel })
        : directiveLabel
          ? t('chatInput.harness.selectorTooltipWithDirective', {
            name: primaryLabel,
            directive: directiveLabel,
          })
          : t('chatInput.harness.selectorTooltip', { name: primaryLabel });
  const triggerState = [open ? 'open' : '', fixedSession ? 'fixed' : '']
    .filter(Boolean)
    .join(' ') || undefined;
  const creatingNewSession = fixedSession && page !== 'summary';

  return (
    <div className="bitfun-harness-selector" data-bf-component="harness-selector" data-bf-part="root">
      <Tooltip content={triggerTooltip}>
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
          onClick={(event) => {
            event.stopPropagation();
            setOpen(value => {
              if (!value) setPage(fixedSession ? 'summary' : 'profiles');
              return !value;
            });
          }}
          data-testid="harness-profile-selector"
        >
          <span className="bitfun-harness-selector__trigger-value">{triggerLabel}</span>
        </button>
      </Tooltip>

      {open && createPortal(
        <div
          ref={menuRef}
          className="bitfun-harness-selector__menu"
          data-bf-component="harness-selector"
          data-bf-part="menu"
          data-bf-state="open"
          data-bf-page={page}
          data-bf-placement={menuLayout?.placement ?? 'top'}
          data-harness-locked={sessionStarted ? 'true' : undefined}
          data-harness-fixed={fixedSession ? 'true' : undefined}
          role="menu"
          style={{
            top: `${menuLayout?.top ?? 0}px`,
            left: `${menuLayout?.left ?? 0}px`,
            visibility: menuLayout ? 'visible' : 'hidden',
          }}
          onMouseDown={event => event.stopPropagation()}
        >
          {page === 'summary' ? (
            <>
              <div
                className="bitfun-harness-selector__session-summary"
                data-bf-component="harness-selector"
                data-bf-part="sessionSummary"
                data-testid="harness-session-summary"
                role="presentation"
              >
                <span className="bitfun-harness-selector__session-value">{primaryLabel}</span>
                <Check size={13} strokeWidth={2.4} aria-hidden />
                {directiveLabel ? (
                  <span className="bitfun-harness-selector__session-directive">
                    {t('chatInput.harness.nextMessageDirective', { directive: directiveLabel })}
                  </span>
                ) : null}
              </div>
              {onStartNewSession ? (
                <>
                  <div className="bitfun-harness-selector__divider" aria-hidden />
                  <button
                    type="button"
                    role="menuitem"
                    className="bitfun-harness-selector__new-session"
                    data-bf-component="harness-selector"
                    data-bf-part="newSession"
                    onClick={() => setPage('profiles')}
                    data-testid="harness-start-new-session"
                  >
                    <span>{t('chatInput.harness.startNewSession')}</span>
                    <ChevronRight size={14} strokeWidth={1.8} aria-hidden />
                  </button>
                </>
              ) : null}
            </>
          ) : page === 'profiles' ? (
            <>
              {fixedSession ? (
                <>
                  <button
                    type="button"
                    role="menuitem"
                    className="bitfun-harness-selector__back"
                    onClick={() => setPage('summary')}
                    data-testid="harness-new-session-back"
                  >
                    <ChevronLeft size={14} strokeWidth={1.8} aria-hidden />
                    <span>{t('chatInput.harness.newSessionModesTitle')}</span>
                  </button>
                  <div className="bitfun-harness-selector__divider" aria-hidden />
                </>
              ) : null}
              {PROFILE_IDS.map((id) => {
                const name = t(`chatInput.harness.profiles.${id}.name`);
                const connected = !creatingNewSession
                  && id === selectedProfile
                  && !legacySession;
                const state = connected
                  ? 'current'
                  : 'available';
                return (
                  <button
                    key={id}
                    type="button"
                    role={creatingNewSession ? 'menuitem' : 'menuitemradio'}
                    aria-checked={creatingNewSession ? undefined : connected}
                    className={`bitfun-harness-selector__profile${connected ? ' is-current' : ''}`}
                    data-bf-component="harness-selector"
                    data-bf-part="profile"
                    data-bf-profile={id}
                    data-bf-state={state}
                    onClick={() => handleSelectProfile(id)}
                    data-testid={`harness-profile-${id}`}
                  >
                    <HarnessProfileMark profile={id} />
                    <span className="bitfun-harness-selector__profile-copy">
                      <span className="bitfun-harness-selector__profile-name">{name}</span>
                    </span>
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
                  </button>
                );
              })}
            </>
          ) : (
            <>
              <button
                type="button"
                role="menuitem"
                className="bitfun-harness-selector__back"
                onClick={() => setPage('profiles')}
                data-testid="harness-agent-back"
              >
                <ChevronLeft size={14} strokeWidth={1.8} aria-hidden />
                <span>{t('chatInput.harness.otherAgentsTitle')}</span>
              </button>
              <div className="bitfun-harness-selector__divider" aria-hidden />
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
                  <button
                    key={agent.id}
                    type="button"
                    role={creatingNewSession ? 'menuitem' : 'menuitemradio'}
                    aria-checked={creatingNewSession ? undefined : connected}
                    className={`bitfun-harness-selector__agent${connected ? ' is-current' : ''}`}
                    data-bf-component="harness-selector"
                    data-bf-part="agent"
                    data-bf-agent-id={agent.id}
                    data-bf-state={state}
                    onClick={() => handleSelectAgent(agent)}
                    data-testid={`harness-agent-${agent.id}`}
                  >
                    <span className="bitfun-harness-selector__agent-mark" aria-hidden>
                      <Bot size={18} strokeWidth={1.55} />
                    </span>
                    <span className="bitfun-harness-selector__profile-copy">
                      <span className="bitfun-harness-selector__profile-name">{agent.name}</span>
                    </span>
                    <span className="bitfun-harness-selector__profile-status">
                      {connected ? <Check size={13} strokeWidth={2.4} aria-hidden /> : null}
                      {agent.available === false
                        ? t('chatInput.harness.unavailable')
                        : null}
                    </span>
                  </button>
                );
              })}
            </>
          )}
        </div>,
        getAppearanceOverlayHost(),
      )}
    </div>
  );
};

export default HarnessProfileSelector;
