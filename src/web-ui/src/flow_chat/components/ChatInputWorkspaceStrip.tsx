/**
 * Workspace label + Git branch (left) and optional usage report control (right).
 */

import React, { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { GitBranch, Activity } from 'lucide-react';
import { Tooltip, IconButton } from '@/component-library';
import { useGitState } from '@/tools/git/hooks/useGitState';
import './ChatInputWorkspaceStrip.scss';

export interface ChatInputWorkspaceStripProps {
  /** Repo root for git status; may come from session when global workspace is unset. */
  repositoryPath: string;
  /** Resolved display name (workspace title or folder basename). */
  workspaceLabel: string;
  /** Session usage report (/usage) — icon on the right when visible. */
  usageReport?: {
    visible: boolean;
    onOpen: () => void;
  };
}

export const ChatInputWorkspaceStrip: React.FC<ChatInputWorkspaceStripProps> = ({
  repositoryPath,
  workspaceLabel,
  usageReport,
}) => {
  const { t } = useTranslation('flow-chat');
  const trimmedPath = repositoryPath.trim();
  const label = workspaceLabel.trim();

  const { currentBranch, isRepository } = useGitState({
    repositoryPath: trimmedPath,
    layers: ['basic'],
    isActive: true,
  });

  const showUsage = usageReport?.visible && !!usageReport.onOpen;

  const branchTooltipContent = useMemo(
    () =>
      isRepository && currentBranch?.trim()
        ? currentBranch.trim()
        : t('workspaceStrip.branchTooltipUnavailable'),
    [currentBranch, isRepository, t],
  );

  if (!label && !showUsage) {
    return null;
  }

  const branchLabel =
    isRepository && currentBranch?.trim()
      ? currentBranch.trim()
      : '—';

  const workspaceTooltipContent = trimmedPath || label;

  const split = !!label && showUsage;
  const usageOnly = !label && showUsage;

  return (
    <div
      className={[
        'bitfun-chat-input-workspace-strip',
        split && 'bitfun-chat-input-workspace-strip--split',
        usageOnly && 'bitfun-chat-input-workspace-strip--usage-only',
      ]
        .filter(Boolean)
        .join(' ')}
      data-testid="chat-input-workspace-strip"
    >
      {label ? (
        <div className="bitfun-chat-input-workspace-strip__main">
          <Tooltip content={workspaceTooltipContent} placement="top">
            <span className="bitfun-chat-input-workspace-strip__chip bitfun-chat-input-workspace-strip__chip--workspace">
              <span className="bitfun-chat-input-workspace-strip__workspace">{label}</span>
            </span>
          </Tooltip>
          <span className="bitfun-chat-input-workspace-strip__sep" aria-hidden>
            {' / '}
          </span>
          <Tooltip content={branchTooltipContent} placement="top">
            <span className="bitfun-chat-input-workspace-strip__chip bitfun-chat-input-workspace-strip__chip--branch">
              <GitBranch
                className="bitfun-chat-input-workspace-strip__branch-icon"
                size={11}
                strokeWidth={2}
                aria-hidden
              />
              <span className="bitfun-chat-input-workspace-strip__branch">{branchLabel}</span>
            </span>
          </Tooltip>
        </div>
      ) : null}

      {showUsage ? (
        <div className="bitfun-chat-input-workspace-strip__usage">
          <Tooltip content={t('usage.runtime.tooltip')}>
            <IconButton
              className="bitfun-chat-input-workspace-strip__usage-btn"
              variant="ghost"
              size="xs"
              type="button"
              aria-label={t('usage.runtime.open')}
              onClick={e => {
                e.stopPropagation();
                usageReport.onOpen();
              }}
            >
              <Activity size={14} strokeWidth={2} aria-hidden />
            </IconButton>
          </Tooltip>
        </div>
      ) : null}
    </div>
  );
};

ChatInputWorkspaceStrip.displayName = 'ChatInputWorkspaceStrip';
