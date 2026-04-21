import React, { useMemo, useState, useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Palette, ExternalLink, Check, AlertCircle, Loader2, ChevronDown, ChevronRight, Clock, RotateCcw } from 'lucide-react';
import type { ToolCardProps } from '../types/flow-chat';
import { BaseToolCard, ToolCardHeader } from './BaseToolCard';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { designTokensAPI, useDesignTokensStore } from '@/tools/design-canvas';
import { canonicalScopeKey, pickString } from '@/tools/design-canvas/tokensSchema';
import { ideControl } from '@/shared/services/ide-control';
import { toolAPI } from '@/infrastructure/api/service-api/ToolAPI';
import { createLogger } from '@/shared/utils/logger';
import { Button, IconButton, Tooltip } from '@/component-library';
import './DesignTokensProposalCard.scss';

const log = createLogger('DesignTokensProposalCard');

/** Matches the 600s hard timeout in `DesignTokens.propose` on the Rust side. */
const SELECTION_TIMEOUT_MS = 600_000;

function parseResult(raw: unknown): any {
  if (!raw) return null;
  if (typeof raw === 'string') {
    try { return JSON.parse(raw); } catch { return null; }
  }
  return raw;
}

const pick = pickString;

/** Token values may be bare numbers (e.g. "12") — ensure they carry a CSS unit. */
function withUnit(val: string | undefined, fallback: string): string {
  if (!val) return fallback;
  return /^\d+(\.\d+)?$/.test(val.trim()) ? `${val}px` : val;
}

interface PreviewProps {
  proposal: any;
  mode: 'native' | 'inverse';
}

const STREAMING_SWATCH_LIMIT = 5;

/**
 * Estimate the relative luminance of a CSS color string (hex / rgb only).
 * Used to decide whether a palette is authored as light-on-dark or
 * dark-on-light so we can render it faithfully on its native surface and
 * stress-test it on the opposite lightness.
 */
function relativeLuminance(input?: string): number | undefined {
  if (!input) return undefined;
  const v = input.trim();
  const hex = v.startsWith('#') ? v.slice(1) : '';
  if (hex) {
    const norm =
      hex.length === 3 || hex.length === 4
        ? hex.slice(0, 3).split('').map((c) => c + c).join('')
        : hex.slice(0, 6);
    if (!/^[0-9a-fA-F]{6}$/.test(norm)) return undefined;
    const r = parseInt(norm.slice(0, 2), 16) / 255;
    const g = parseInt(norm.slice(2, 4), 16) / 255;
    const b = parseInt(norm.slice(4, 6), 16) / 255;
    return 0.2126 * r + 0.7152 * g + 0.0722 * b;
  }
  const rgb = v.match(/rgba?\(\s*(\d+)[\s,]+(\d+)[\s,]+(\d+)/i);
  if (rgb) {
    const r = parseInt(rgb[1], 10) / 255;
    const g = parseInt(rgb[2], 10) / 255;
    const b = parseInt(rgb[3], 10) / 255;
    return 0.2126 * r + 0.7152 * g + 0.0722 * b;
  }
  return undefined;
}

const MiniSystemPreview: React.FC<PreviewProps> = ({ proposal, mode }) => {
  const { t } = useTranslation('flow-chat');
  const colors = proposal.colors || {};
  const typography = proposal.typography || {};
  const radius = proposal.radius || {};
  const shadow = proposal.shadow || {};
  const scale = typography.scale || {};

  const fontFamily = pick(typography, 'fontFamily', 'family') || 'Inter, system-ui, sans-serif';
  const primary = pick(colors, 'primary', 'accent', 'brand') || '#161616';
  const accent = pick(colors, 'accent', 'accentSecondary', 'secondary') || primary;
  const success = pick(colors, 'success', 'positive') || '#22c55e';
  const danger = pick(colors, 'danger', 'error', 'warning') || '#ef4444';
  const mdRadius = withUnit(pick(radius, 'md', 'base'), '8px');
  const smRadius = withUnit(pick(radius, 'sm', 'xs'), '4px');
  const fullRadius = withUnit(pick(radius, 'full', 'pill'), '999px');

  const realBackground = pick(colors, 'background', 'bg');
  const realSurface = pick(colors, 'surface', 'surfaceElevated') || realBackground;
  const realText = pick(colors, 'text');
  const realTextMuted = pick(colors, 'textMuted', 'textSecondary');
  const realBorder = pick(colors, 'border');

  const paletteIsLight = (relativeLuminance(realBackground) ?? 0.5) > 0.5;

  const mdShadow =
    pick(shadow, 'md', 'base', 'sm') ||
    (paletteIsLight ? '0 2px 8px rgba(0,0,0,0.06)' : '0 4px 14px rgba(0,0,0,0.32)');

  // Render the proposal faithfully on its authored surface; use a neutral
  // inverse for stress-testing on the opposite lightness.
  const isNative = mode === 'native';
  const bg = isNative
    ? realBackground || (paletteIsLight ? '#F7F7F5' : '#0C0D10')
    : paletteIsLight
      ? '#0C0D10'
      : '#F7F7F5';
  const surface = isNative
    ? realSurface || realBackground || (paletteIsLight ? '#FFFFFF' : '#14161A')
    : paletteIsLight
      ? '#14161A'
      : '#FFFFFF';
  const text = isNative
    ? realText || (paletteIsLight ? '#0C0D10' : '#F5F7FB')
    : paletteIsLight
      ? '#F5F7FB'
      : '#0C0D10';
  const textMuted = isNative
    ? realTextMuted || (paletteIsLight ? 'rgba(12,13,16,0.55)' : 'rgba(245,247,251,0.6)')
    : paletteIsLight
      ? 'rgba(245,247,251,0.6)'
      : 'rgba(12,13,16,0.55)';
  const border = isNative
    ? realBorder || (paletteIsLight ? 'rgba(12,13,16,0.08)' : 'rgba(255,255,255,0.1)')
    : paletteIsLight
      ? 'rgba(255,255,255,0.1)'
      : 'rgba(12,13,16,0.08)';

  const bodySize = pick(scale, 'body', 'base') || '13px';
  const captionSize = pick(scale, 'caption', 'small') || '11px';
  const titleSize = pick(scale, 'title', 'heading') || '16px';

  const style = {
    fontFamily,
    background: bg,
    color: text,
    '--mp-surface': surface,
    '--mp-border': border,
    '--mp-text': text,
    '--mp-text-muted': textMuted,
    '--mp-primary': primary,
    '--mp-radius-md': mdRadius,
    '--mp-radius-sm': smRadius,
    '--mp-radius-full': fullRadius,
    '--mp-shadow': mdShadow,
  } as React.CSSProperties;

  const modeLabel =
    mode === 'native'
      ? t('toolCards.designTokens.previewModeNative')
      : t('toolCards.designTokens.previewModeInverse');

  return (
    <div className={`dtp-preview dtp-preview--${mode}`} style={style}>
      <span className="dtp-preview__mode">{modeLabel}</span>

      <div className="dtp-preview__type" style={{ fontSize: titleSize }}>
        Aa
        <span className="dtp-preview__type-body" style={{ fontSize: bodySize }}>Ag</span>
        <span className="dtp-preview__type-caption" style={{ fontSize: captionSize }}>ag</span>
      </div>

      <div className="dtp-preview__row">
        <button type="button" className="dtp-preview__btn dtp-preview__btn--primary">
          {t('toolCards.designTokens.previewPrimary')}
        </button>
        <button type="button" className="dtp-preview__btn dtp-preview__btn--ghost">
          {t('toolCards.designTokens.previewGhost')}
        </button>
        <label className="dtp-preview__switch" aria-label={t('toolCards.designTokens.switchAriaLabel')}>
          <input type="checkbox" defaultChecked readOnly />
          <span className="dtp-preview__switch-track"><span className="dtp-preview__switch-thumb" /></span>
        </label>
      </div>

      <div className="dtp-preview__palette" aria-hidden="true">
        <span style={{ background: primary }} />
        <span style={{ background: accent }} />
        <span style={{ background: success }} />
        <span style={{ background: danger }} />
        <span style={{ background: surface, boxShadow: `inset 0 0 0 1px ${border}` }} />
      </div>
    </div>
  );
};

const StreamingProposalPreview: React.FC<{ proposal: any }> = React.memo(({ proposal }) => {
  const { t } = useTranslation('flow-chat');
  const colorEntries = Object.entries(proposal.colors || {}).slice(0, STREAMING_SWATCH_LIMIT) as Array<[string, string]>;

  return (
    <article className="design-tokens-proposal-card__item is-awaiting">
      <header className="design-tokens-proposal-card__head">
        <div className="design-tokens-proposal-card__name">
          <div className="design-tokens-proposal-card__name-row">
            <strong>{proposal.name || t('toolCards.designTokens.untitledProposal')}</strong>
          </div>
          <span className="design-tokens-proposal-card__mood">
            {proposal.mood || t('toolCards.designTokens.defaultMood')}
          </span>
        </div>
      </header>
      <div className="design-tokens-proposal-card__swatches">
        {colorEntries.map(([name, value]) => (
          <div key={name} className="design-tokens-proposal-card__swatch" title={`${name} · ${value}`}>
            <span className="design-tokens-proposal-card__swatch-chip" style={{ background: String(value) }} />
            <span className="design-tokens-proposal-card__swatch-name">{name}</span>
          </div>
        ))}
      </div>
      <div className="design-tokens-proposal-card__pending">
        <Loader2 size={14} className="is-spinning" />
        <span>{t('toolCards.designTokens.streamingHint')}</span>
      </div>
    </article>
  );
});
StreamingProposalPreview.displayName = 'StreamingProposalPreview';

export const DesignTokensProposalCard: React.FC<ToolCardProps> = ({ toolItem }) => {
  const { t } = useTranslation('flow-chat');
  const { workspacePath } = useCurrentWorkspace();
  const result = useMemo(() => parseResult(toolItem.toolResult?.result), [toolItem.toolResult?.result]);
  const { status, toolResult, toolCall, partialParams, isParamsStreaming } = toolItem;

  // Payload sources:
  // - While the tool is still running, read proposals from the call input (same pattern as AskUserQuestion).
  // - Once the tool completes, read the authoritative payload from the result (which includes committed_id).
  const resultPayload = result?.data;
  const resultTokens = resultPayload?.tokens;
  const resultPath = resultPayload?.path;
  const selectionStatus: string | undefined = resultPayload?.selection_status;

  const inputParams = (partialParams || toolCall?.input) as Record<string, any> | undefined;
  const inputProposals: any[] = Array.isArray(inputParams?.proposals) ? inputParams!.proposals : [];

  const isCompleted = status === 'completed';
  const isFailed = status === 'error' || toolResult?.success === false;
  const failure = toolResult?.error || result?.error || t('toolCards.designTokens.generationFailed');

  // Prefer the authoritative result when the tool has completed; otherwise fall back to the streaming input.
  const proposals: any[] = resultTokens?.proposals?.length ? resultTokens.proposals : inputProposals;
  const committedId: string | undefined = resultTokens?.committed_id || undefined;

  /** Same as AskUserQuestionCard: no selection until streaming tool args finish (avoids partial proposals). */
  const paramsReady = !isParamsStreaming;
  const awaitingSelection =
    !isCompleted && !isFailed && proposals.length > 0 && paramsReady;
  const awaitingPayload = !isCompleted && !isFailed && proposals.length === 0;

  // User-side local state: which proposal is currently highlighted, and submission progress.
  const [localSelectedId, setLocalSelectedId] = useState<string>('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [expandedIds, setExpandedIds] = useState<Record<string, boolean>>({});
  // Live countdown (seconds) until the backend's 600s oneshot timeout lapses.
  // Gives users a concrete "you have 9:48 left to pick" affordance instead of a
  // silent-then-dead card.
  const [remainingMs, setRemainingMs] = useState<number | null>(null);

  const artifactIdFromInput = toolItem.toolCall?.input?.artifact_id as string | undefined;
  const scopeKey = useMemo(
    () => canonicalScopeKey({ explicitPath: resultPath, workspacePath, artifactId: artifactIdFromInput }),
    [resultPath, workspacePath, artifactIdFromInput]
  );

  // Sync authoritative tokens doc into the shared store — was previously a
  // render-phase side effect which broke React's concurrent mode guarantees.
  useEffect(() => {
    if (scopeKey && resultTokens) {
      useDesignTokensStore.getState().upsert(scopeKey, resultTokens);
    }
  }, [scopeKey, resultTokens]);

  // Timeout countdown: start when the card enters the "awaiting selection"
  // state, clear as soon as the tool completes or errors.
  useEffect(() => {
    if (!awaitingSelection) {
      setRemainingMs(null);
      return;
    }
    const startAt = Date.now();
    setRemainingMs(SELECTION_TIMEOUT_MS);
    const handle = window.setInterval(() => {
      const left = SELECTION_TIMEOUT_MS - (Date.now() - startAt);
      setRemainingMs(left > 0 ? left : 0);
    }, 1000);
    return () => window.clearInterval(handle);
  }, [awaitingSelection]);

  const openStudio = useCallback(() => {
    ideControl.panel.open('design-tokens-studio', {
      position: 'right',
      config: {
        title: t('toolCards.designTokens.studioPanelTitle'),
        data: {
          artifactId: artifactIdFromInput,
          scopePath: scopeKey,
        },
        workspace_path: workspacePath,
      },
      options: { auto_focus: true, check_duplicate: true },
    });
  }, [scopeKey, artifactIdFromInput, workspacePath, t]);

  const submitChoice = useCallback(async (proposalId: string) => {
    if (isSubmitting || isParamsStreaming) return;
    const toolId = toolItem.id ?? toolItem.toolCall?.id;
    if (!toolId) {
      log.warn('Cannot submit choice without tool id');
      return;
    }
    setLocalSelectedId(proposalId);
    setIsSubmitting(true);
    try {
      await toolAPI.submitUserAnswers(toolId, { proposal_id: proposalId } as any);
    } catch (error) {
      log.error('Failed to submit token selection', { toolId, error });
      setIsSubmitting(false);
    }
  }, [isSubmitting, isParamsStreaming, toolItem.id, toolItem.toolCall?.id]);

  const recommit = useCallback(async (proposalId: string) => {
    await designTokensAPI.commit(proposalId, artifactIdFromInput, workspacePath);
  }, [artifactIdFromInput, workspacePath]);

  /**
   * Fired on timeout or cancelled states: ask the backend to re-open a
   * selection channel against the on-disk proposals, so the user can pick
   * again without the agent having to re-generate proposals.
   */
  const retryAwaitSelection = useCallback(async () => {
    try {
      await designTokensAPI.awaitSelection(artifactIdFromInput, workspacePath);
    } catch (err) {
      log.warn('retryAwaitSelection failed', err);
    }
  }, [artifactIdFromInput, workspacePath]);

  const toggleExpand = (id: string) => setExpandedIds((prev) => ({ ...prev, [id]: !prev[id] }));

  const header = (
    <ToolCardHeader
      icon={<Palette size={14} />}
      content={
        isFailed ? (
          <div className="design-tokens-proposal-card__failure-header">
            <AlertCircle size={14} />
            <span>{failure}</span>
          </div>
        ) : (
          <div className="design-tokens-proposal-card__title">
            <span>{t('toolCards.designTokens.cardTitle')}</span>
            {proposals.length > 0 && <span className="design-tokens-proposal-card__count">{proposals.length}</span>}
            {awaitingSelection && (
              <span className="design-tokens-proposal-card__awaiting-chip">
                <Loader2 size={10} className="is-spinning" /> {t('toolCards.designTokens.awaitingYourChoice')}
                {remainingMs !== null && (
                  <span className="design-tokens-proposal-card__countdown">
                    <Clock size={10} />
                    {Math.max(0, Math.floor(remainingMs / 60000))}:
                    {String(Math.max(0, Math.floor((remainingMs % 60000) / 1000))).padStart(2, '0')}
                  </span>
                )}
              </span>
            )}
            {isCompleted && committedId && (
              <span className="design-tokens-proposal-card__committed-chip">
                <Check size={14} strokeWidth={2.5} /> {t('toolCards.designTokens.adopted')}
              </span>
            )}
            {isCompleted && !committedId && selectionStatus && (
              <span className="design-tokens-proposal-card__warn-chip">
                {selectionStatus === 'timeout'
                  ? t('toolCards.designTokens.statusTimeout')
                  : selectionStatus === 'cancelled'
                    ? t('toolCards.designTokens.statusCancelled')
                    : selectionStatus === 'invalid'
                      ? t('toolCards.designTokens.statusInvalid')
                      : selectionStatus}
              </span>
            )}
          </div>
        )
      }
      extra={
        !isFailed ? (
          <Tooltip content={t('toolCards.designTokens.openTokensStudio')} placement="top">
            <span>
              <IconButton
                type="button"
                variant="ghost"
                size="xs"
                onClick={(e) => { e.stopPropagation(); openStudio(); }}
              >
                <ExternalLink size={12} />
              </IconButton>
            </span>
          </Tooltip>
        ) : null
      }
    />
  );

  const renderProposal = (proposal: any) => {
    if (isParamsStreaming) {
      return <StreamingProposalPreview key={proposal.id} proposal={proposal} />;
    }

    const colorEntries = Object.entries(proposal.colors || {}).slice(0, 8) as Array<[string, string]>;
    const typography = proposal.typography || {};
    const scale = typography.scale || {};
    const radius = proposal.radius || {};
    const shadow = proposal.shadow || {};
    const family = pick(typography, 'fontFamily', 'family') || 'System';
    const displaySize = pick(scale, 'display', 'headline', 'title') || '-';
    const bodySize = pick(scale, 'body', 'base') || '-';
    const radiusMd = pick(radius, 'md', 'base') || '-';
    const shadowMd = pick(shadow, 'md', 'base', 'sm') || '—';

    const isCommitted = committedId === proposal.id;
    const isPendingThis = awaitingSelection && isSubmitting && localSelectedId === proposal.id;
    // After commit: only the committed one is open; others collapse.
    // While awaiting: all open so the user can compare.
    const isCollapsed = isCompleted && committedId
      ? (!isCommitted && !expandedIds[proposal.id])
      : false;
    const canCollapseHeader = isCompleted && Boolean(committedId) && !isCommitted;

    const classes = [
      'design-tokens-proposal-card__item',
      isCommitted ? 'is-committed' : '',
      isCollapsed ? 'is-collapsed' : '',
      awaitingSelection ? 'is-awaiting' : '',
      localSelectedId === proposal.id && awaitingSelection ? 'is-selected-local' : '',
    ].filter(Boolean).join(' ');

    if (isCollapsed) {
      return (
        <article key={proposal.id} className={classes}>
          <button
            type="button"
            className="design-tokens-proposal-card__collapsed"
            onClick={() => toggleExpand(proposal.id)}
          >
            <span className="design-tokens-proposal-card__disclosure-rail" aria-hidden>
              <ChevronRight className="design-tokens-proposal-card__disclosure-icon" size={16} strokeWidth={2} />
            </span>
            <span className="design-tokens-proposal-card__collapsed-text">
              <span className="design-tokens-proposal-card__collapsed-name">{proposal.name}</span>
              <span className="design-tokens-proposal-card__collapsed-mood">{proposal.mood}</span>
            </span>
            <span className="design-tokens-proposal-card__collapsed-swatches">
              {colorEntries.slice(0, 5).map(([name, value]) => (
                <span key={name} style={{ background: String(value) }} title={`${name} · ${value}`} />
              ))}
            </span>
            <span className="design-tokens-proposal-card__collapsed-hint">{t('toolCards.designTokens.expand')}</span>
          </button>
        </article>
      );
    }

    return (
      <article key={proposal.id} className={classes}>
        <header className="design-tokens-proposal-card__head">
          {canCollapseHeader ? (
            <button
              type="button"
              className="design-tokens-proposal-card__head-toggle"
              onClick={() => toggleExpand(proposal.id)}
              title={t('toolCards.designTokens.collapse')}
            >
              <span className="design-tokens-proposal-card__disclosure-rail" aria-hidden>
                <ChevronDown className="design-tokens-proposal-card__disclosure-icon" size={16} strokeWidth={2} />
              </span>
              <div className="design-tokens-proposal-card__name">
                <div className="design-tokens-proposal-card__name-row">
                  <strong>{proposal.name}</strong>
                </div>
                <span className="design-tokens-proposal-card__mood">{proposal.mood}</span>
              </div>
            </button>
          ) : (
            <div className="design-tokens-proposal-card__name">
              <div className="design-tokens-proposal-card__name-row">
                {isCommitted && (
                  <span className="design-tokens-proposal-card__badge">
                    <Check size={14} strokeWidth={2.5} /> {t('toolCards.designTokens.adopted')}
                  </span>
                )}
                <strong>{proposal.name}</strong>
              </div>
              <span className="design-tokens-proposal-card__mood">{proposal.mood}</span>
            </div>
          )}
        </header>

        <div className="design-tokens-proposal-card__swatches">
          {colorEntries.map(([name, value]) => (
            <div key={name} className="design-tokens-proposal-card__swatch" title={`${name} · ${value}`}>
              <span className="design-tokens-proposal-card__swatch-chip" style={{ background: String(value) }} />
              <span className="design-tokens-proposal-card__swatch-name">{name}</span>
            </div>
          ))}
        </div>

        <dl className="design-tokens-proposal-card__system">
          <div>
            <dt>{t('toolCards.designTokens.font')}</dt>
            <dd title={family}>{family.split(',')[0].replace(/['"]/g, '')} · {displaySize}/{bodySize}</dd>
          </div>
          <div>
            <dt>{t('toolCards.designTokens.radius')}</dt>
            <dd>{radiusMd}</dd>
          </div>
          <div>
            <dt>{t('toolCards.designTokens.shadow')}</dt>
            <dd className="design-tokens-proposal-card__shadow-chip" style={{ boxShadow: shadowMd }} />
          </div>
        </dl>

        <div className="design-tokens-proposal-card__previews">
          <MiniSystemPreview proposal={proposal} mode="native" />
          <MiniSystemPreview proposal={proposal} mode="inverse" />
        </div>

        <div className="design-tokens-proposal-card__actions">
          {awaitingSelection ? (
            <Button
              size="small"
              variant="primary"
              type="button"
              onClick={() => submitChoice(proposal.id)}
              disabled={isSubmitting}
              isLoading={isPendingThis}
            >
              {isPendingThis ? (
                <>{t('toolCards.designTokens.submitting')}</>
              ) : (
                <>
                  <Check size={14} strokeWidth={2.5} />
                  {t('toolCards.designTokens.adoptThisSystem')}
                </>
              )}
            </Button>
          ) : isCompleted ? (
            isCommitted ? (
              <Button size="small" variant="ghost" type="button" disabled>
                <Check size={14} strokeWidth={2.5} /> {t('toolCards.designTokens.adopted')}
              </Button>
            ) : (
              <Button size="small" variant="ghost" type="button" onClick={() => recommit(proposal.id)}>
                <Check size={14} strokeWidth={2.5} /> {t('toolCards.designTokens.switchToThisSystem')}
              </Button>
            )
          ) : null}
          <Button size="small" variant="ghost" type="button" onClick={openStudio}>
            <ExternalLink size={12} />
            {t('toolCards.designTokens.openInStudio')}
          </Button>
        </div>
      </article>
    );
  };

  return (
    <BaseToolCard
      status={toolItem.status}
      isExpanded
      className="design-tokens-proposal-card"
      header={header}
      expandedContent={
        isFailed ? (
          <div className="design-tokens-proposal-card__error-body">
            <AlertCircle size={16} />
            <div>
              <div className="design-tokens-proposal-card__error-title">{t('toolCards.designTokens.errorTitle')}</div>
              <div className="design-tokens-proposal-card__error-copy">{failure}</div>
            </div>
          </div>
        ) : awaitingPayload ? (
          <div className="design-tokens-proposal-card__pending">
            <Loader2 size={14} className="is-spinning" />
            <span>
              {isParamsStreaming
                ? t('toolCards.designTokens.receivingDirections')
                : t('toolCards.designTokens.preparingDirections')}
            </span>
          </div>
        ) : (
          <>
            {proposals.length > 0 && Boolean(isParamsStreaming) && (
              <div className="design-tokens-proposal-card__list-streaming-hint" role="status">
                <Loader2 size={12} className="is-spinning" />
                <span>{t('toolCards.designTokens.streamingSelectHint')}</span>
              </div>
            )}
            <div className="design-tokens-proposal-card__list">
              {proposals.map((proposal: any) => renderProposal(proposal))}
            </div>
            {isCompleted && !committedId && (selectionStatus === 'timeout' || selectionStatus === 'cancelled') && (
              <div className="design-tokens-proposal-card__retry">
                <Button size="small" variant="ghost" type="button" onClick={retryAwaitSelection}>
                  <RotateCcw size={12} />
                  {t('toolCards.designTokens.reopenSelection')}
                </Button>
              </div>
            )}
          </>
        )
      }
      isFailed={isFailed}
    />
  );
};

export default DesignTokensProposalCard;
