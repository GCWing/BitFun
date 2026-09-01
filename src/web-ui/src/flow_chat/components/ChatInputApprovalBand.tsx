/**
 * The compact surface for answering a runtime permission request.
 *
 * This used to be a card floating over the transcript, positioned by measuring
 * the composer's height. It covered the very output the reader needed in order
 * to decide, and it carried its own textarea for the rejection reason while a
 * perfectly good one sat directly underneath it. So the band lives in the
 * composer stack instead: the request reads directly above the text field that
 * answers it, and the reason is whatever the reader has typed there. Embedded
 * child-session panels also reuse the band because they have no composer of
 * their own; those surfaces intentionally omit the optional typed reason.
 */

import React, { useState } from 'react';
import { Button } from '@bitfun/ui';
import { Check, ShieldAlert, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Tooltip } from '@bitfun/ui';
import type {
  PermissionReplyKind,
  PermissionRequest,
} from '@/infrastructure/api/service-api/AgentAPI';
import { CopyableTextPreview } from './CopyableTextPreview';
import './ChatInputApprovalBand.scss';

export interface ChatInputApprovalBandProps {
  /** Requests of one round, the first of which is the one being answered. */
  requests: PermissionRequest[];
  /** Pending requests across the session, including later rounds. */
  totalPendingCount?: number;
  /**
   * The composer's current text. A non-empty composer is offered as the
   * rejection reason rather than being treated as one silently.
   */
  rejectReason?: string;
  /** Clears the composer once its text has been spent as a reason. */
  onRejectReasonConsumed?: () => void;
  onRespond: (requestId: string, reply: PermissionReplyKind, feedback?: string) => Promise<void>;
  onRespondBatch: (requestId: string, reply: PermissionReplyKind, feedback?: string) => Promise<void>;
}

const PERMISSION_ACTION_LABEL_KEYS: Record<string, string> = {
  read: 'permission.actions.read',
  edit: 'permission.actions.edit',
  bash: 'permission.actions.bash',
  git: 'permission.actions.git',
  computer_use: 'permission.actions.computerUse',
  websearch: 'permission.actions.webSearch',
  webfetch: 'permission.actions.webFetch',
  mcp: 'permission.actions.mcp',
  task: 'permission.actions.task',
  skill: 'permission.actions.skill',
  page_publish: 'permission.actions.pagePublish',
  page_deploy: 'permission.actions.pageDeploy',
  custom_tool: 'permission.actions.customTool',
  external_directory: 'permission.actions.externalDirectory',
};

const PAGE_VISIBILITY_LABEL_KEYS: Record<string, string> = {
  private: 'permission.visibility.private',
  relay: 'permission.visibility.relay',
  public: 'permission.visibility.public',
};

function permissionActionLabel(action: string, t: (key: string) => string): string {
  return t(PERMISSION_ACTION_LABEL_KEYS[action] ?? 'permission.actions.other');
}

function metadataString(
  metadata: Record<string, unknown> | undefined,
  key: string,
): string | undefined {
  const value = metadata?.[key];
  return typeof value === 'string' && value.trim() ? value.trim() : undefined;
}

/**
 * What the reader stands to lose by allowing. Page operations describe
 * themselves precisely; everything else carries whatever the tool declared.
 */
function permissionRisk(
  request: PermissionRequest | undefined,
  t: (key: string, values?: Record<string, string>) => string,
): string | undefined {
  if (!request) return undefined;
  const metadata = request.displayMetadata;
  const operation = metadataString(metadata, 'pageOperation');
  const slug = metadataString(metadata, 'pageSlug');
  if (operation && slug) {
    if (operation === 'deploy') {
      return t('permission.risks.pageDeploy', {
        slug,
        version: metadataString(metadata, 'pageVersion') ?? '',
      });
    }
    const visibility = metadataString(metadata, 'pageVisibility') ?? 'private';
    const translatedVisibility = t(
      PAGE_VISIBILITY_LABEL_KEYS[visibility] ?? PAGE_VISIBILITY_LABEL_KEYS.private,
    );
    return t(
      operation === 'publish' ? 'permission.risks.pagePublish' : 'permission.risks.pageSave',
      { slug, visibility: translatedVisibility },
    );
  }
  return [metadata?.riskDescription, metadata?.risk].find(
    (value): value is string => typeof value === 'string' && value.trim().length > 0,
  );
}

type ApprovalScope = 'this' | 'all';

export const ChatInputApprovalBand: React.FC<ChatInputApprovalBandProps> = ({
  requests,
  totalPendingCount,
  rejectReason = '',
  onRejectReasonConsumed,
  onRespond,
  onRespondBatch,
}) => {
  const { t } = useTranslation('flow-chat');
  const [scope, setScope] = useState<ApprovalScope>('this');
  const [responding, setResponding] = useState(false);
  const [error, setError] = useState(false);

  const request = requests[0];
  const pendingCount = Math.max(totalPendingCount ?? requests.length, requests.length);
  // Answering "and everything after this" only means something when there is
  // something after it, either in this round or a later one.
  const canAnswerAll = pendingCount > 1;
  const effectiveScope: ApprovalScope = canAnswerAll ? scope : 'this';
  const reason = rejectReason.trim();

  if (!request) return null;

  const risk = permissionRisk(request, t);
  const alwaysAllowTooltip = request.saveResources?.length
    ? request.projectPath?.trim()
      ? t('permission.allowAlwaysTooltip', { projectPath: request.projectPath.trim() })
      : t('permission.allowAlwaysTooltipCurrentProject')
    : t('permission.allowAlwaysTooltipNoGrant');

  const answer = async (reply: PermissionReplyKind, withReason: boolean) => {
    setResponding(true);
    setError(false);
    const feedback = reply === 'reject' && withReason && reason ? reason : undefined;
    try {
      // 'always' is deliberately never batched: see the button's comment.
      if (effectiveScope === 'all' && reply !== 'always') {
        await onRespondBatch(request.requestId, reply, feedback);
      } else {
        await onRespond(request.requestId, reply, feedback);
      }
      if (feedback) {
        onRejectReasonConsumed?.();
      }
    } catch {
      setError(true);
    } finally {
      setResponding(false);
    }
  };

  const resourceSummary = request.resources.join(', ');
  const answersAll = effectiveScope === 'all';
  const allowLabel = answersAll
    ? t('permission.allowCurrentAndFollowing')
    : t('permission.allowOnce');
  const rejectLabel = answersAll
    ? t('permission.rejectCurrentAndFollowing')
    : t('permission.reject');

  return (
    <div
      data-bf-component="permission-request-panel"
      data-bf-part="root"
      data-bf-state={[responding && 'responding', error && 'error'].filter(Boolean).join(' ')}
      className="bitfun-chat-input-approval"
      role="group"
      aria-label={t('permission.title')}
      data-testid="chat-input-approval-band"
      data-approval-scope={effectiveScope}
    >
      <div
        data-bf-component="permission-request-panel"
        data-bf-part="request"
        className="bitfun-chat-input-approval__request"
      >
        <ShieldAlert
          className="bitfun-chat-input-approval__icon"
          size={14}
          strokeWidth={2.1}
          aria-hidden
        />
        <span className="bitfun-chat-input-approval__action">
          {permissionActionLabel(request.action, t)}
        </span>
        <span className="bitfun-chat-input-approval__separator" aria-hidden>·</span>
        <CopyableTextPreview
          as="code"
          text={resourceSummary}
          emptyText=""
          className="bitfun-chat-input-approval__resource copyable-text-preview--theme-font"
          tooltipContent={request.resources.join('\n') || undefined}
          tooltipPlacement="top"
        />
        {request.delegation ? (
          <span className="bitfun-chat-input-approval__owner">
            {t('permission.subagentOwner', { subagent: request.delegation.subagentType })}
          </span>
        ) : (
          <span className="bitfun-chat-input-approval__owner">{request.source.identity}</span>
        )}
        {canAnswerAll ? (
          <Tooltip content={t('permission.batchCount', { count: pendingCount })} placement="top">
            <span
              className="bitfun-chat-input-approval__count"
              data-testid="chat-input-approval-pending-count"
            >
              +{pendingCount - 1}
            </span>
          </Tooltip>
        ) : null}
      </div>

      {/* The risk is the reason to read the band at all, so it keeps its own
          line rather than hiding in a tooltip. */}
      {error ? (
        <p
          data-bf-component="permission-request-panel"
          data-bf-part="error"
          className="bitfun-chat-input-approval__note bitfun-chat-input-approval__note--error"
          role="alert"
        >
          {t('permission.responseFailed')}
        </p>
      ) : risk ? (
        <p
          data-bf-component="permission-request-panel"
          data-bf-part="risk"
          className="bitfun-chat-input-approval__note"
        >
          {risk}
        </p>
      ) : null}

      <div
        data-bf-component="permission-request-panel"
        data-bf-part="actions"
        className="bitfun-chat-input-approval__actions"
      >
        {canAnswerAll ? (
          <div
            data-bf-component="permission-request-panel"
            data-bf-part="scope"
            className="bitfun-chat-input-approval__scope"
            role="radiogroup"
            aria-label={t('permission.scopeLabel')}
          >
            {(['this', 'all'] as const).map(option => (
              <button
                key={option}
                type="button"
                role="radio"
                aria-checked={effectiveScope === option}
                className={[
                  'bitfun-chat-input-approval__scope-option',
                  effectiveScope === option && 'bitfun-chat-input-approval__scope-option--active',
                ]
                  .filter(Boolean)
                  .join(' ')}
                disabled={responding}
                data-testid={`chat-input-approval-scope-${option}`}
                onClick={() => setScope(option)}
              >
                {option === 'this' ? t('permission.scopeThis') : t('permission.scopeAll')}
              </button>
            ))}
          </div>
        ) : null}

        <span className="bitfun-chat-input-approval__spacer" />

        {/* Rejecting is the safe answer, so it leads and never depends on
            anything else being in the right state. */}
        <Button
          type="button"
          variant="outline"
          size="sm"
          leadingIcon={<X size={13} strokeWidth={2.2} />}
          disabled={responding}
          data-testid="chat-input-approval-reject"
          onClick={() => void answer('reject', false)}
        >
          {rejectLabel}
        </Button>
        {/* The composer is the reason field. It is offered rather than assumed,
            so a half-typed next message cannot become a rejection reason and
            typing one cannot block the allow buttons. */}
        {reason ? (
          <Tooltip content={t('permission.rejectWithReasonTooltip')} placement="top">
            <Button
              type="button"
              variant="outline"
              size="sm"
              leadingIcon={<X size={13} strokeWidth={2.2} />}
              disabled={responding}
              data-testid="chat-input-approval-reject-with-reason"
              onClick={() => void answer('reject', true)}
            >
              {t('permission.rejectWithReason')}
            </Button>
          </Tooltip>
        ) : null}
        <Button
          type="button"
          variant="fill"
          size="sm"
          leadingIcon={<Check size={13} strokeWidth={2.2} />}
          disabled={responding}
          data-testid="chat-input-approval-allow"
          onClick={() => void answer('once', false)}
        >
          {allowLabel}
        </Button>
        {/* "Always" writes a saved grant, so it is only offered when this
            request has a scope to save, and only for the request in front of
            the reader — a saved grant is not something to apply in bulk to
            requests they have not read. */}
        {request.saveResources?.length && !answersAll ? (
          <Tooltip content={alwaysAllowTooltip} placement="top">
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={responding}
              data-testid="chat-input-approval-allow-always"
              onClick={() => void answer('always', false)}
            >
              {t('permission.allowAlways')}
            </Button>
          </Tooltip>
        ) : null}
      </div>
    </div>
  );
};

ChatInputApprovalBand.displayName = 'ChatInputApprovalBand';
