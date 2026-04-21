/**
 * DesignArtifact tool card — a minimal "index card" that lives in the chat
 * stream and points the user to the right-side Design Canvas tab. This card
 * never renders the full design preview; Design artifacts are edited and
 * previewed in the Design Canvas panel.
 */

import React, { useCallback, useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Palette, ExternalLink, FileStack, GitBranch } from 'lucide-react';
import type { ToolCardProps } from '../types/flow-chat';
import { BaseToolCard, ToolCardHeader } from './BaseToolCard';
import {
  useDesignArtifactStore,
  type DesignArtifactManifest,
  type ArtifactEventKind,
} from '@/tools/design-canvas';
import { ideControl } from '@/shared/services/ide-control';
import { createLogger } from '@/shared/utils/logger';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import './DesignArtifactIndexCard.scss';

const log = createLogger('DesignArtifactIndexCard');

interface ResultPayload {
  success?: boolean;
  artifact_event?: ArtifactEventKind;
  manifest?: DesignArtifactManifest;
  manifests?: DesignArtifactManifest[];
  error?: string;
}

function parseResult(raw: unknown): ResultPayload | null {
  if (!raw) return null;
  if (typeof raw === 'string') {
    try {
      return JSON.parse(raw) as ResultPayload;
    } catch {
      return null;
    }
  }
  if (typeof raw === 'object') {
    return raw as ResultPayload;
  }
  return null;
}

export const DesignArtifactIndexCard: React.FC<ToolCardProps> = ({ toolItem }) => {
  const { t } = useTranslation('flow-chat');
  const { status, toolCall, toolResult } = toolItem;
  const { workspacePath } = useCurrentWorkspace();
  const resultPayload = useMemo(() => parseResult(toolResult?.result), [toolResult?.result]);
  const upsertManifest = useDesignArtifactStore((s) => s.upsertManifest);
  const upsertManifests = useDesignArtifactStore((s) => s.upsertManifests);

  const action = (toolCall?.input?.action as string) || 'create';
  const manifest = resultPayload?.manifest;
  const manifests = resultPayload?.manifests;
  const event = resultPayload?.artifact_event ?? 'ok';
  const isFailed = status === 'error' || resultPayload?.success === false;
  const failure = resultPayload?.error || t('toolCards.designArtifact.operationFailed');

  const actionLabel = useCallback(
    (a: string) => t(`toolCards.designArtifact.actions.${a}`, { defaultValue: a }),
    [t]
  );
  const eventLabel = useCallback(
    (e: string) => t(`toolCards.designArtifact.events.${e}`, { defaultValue: e }),
    [t]
  );
  const streamingPath =
    (toolItem.partialParams?.path as string | undefined) ||
    (toolCall?.input?.path as string | undefined) ||
    (toolCall?.input?.entry as string | undefined);

  useEffect(() => {
    if (status !== 'completed' || !resultPayload?.success) return;
    if (manifest) {
      upsertManifest(manifest, event);
    }
    if (manifests && manifests.length > 0) {
      upsertManifests(manifests);
    }
  }, [status, resultPayload?.success, manifest, manifests, event, upsertManifest, upsertManifests]);

  const openInCanvas = useCallback(() => {
    if (!manifest) return;
    try {
      ideControl.panel.open('design-artifact', {
        position: 'right',
        config: {
          title: manifest.title,
          data: { artifactId: manifest.id, manifest },
          workspace_path: workspacePath,
        },
        options: { auto_focus: true, check_duplicate: true },
      });
    } catch (err) {
      log.warn('Failed to open design-artifact tab', err);
    }
  }, [manifest, workspacePath]);

  const filesCount = manifest?.files?.length ?? 0;
  const versionsCount = manifest?.versions?.length ?? 0;
  const subtitle = manifest
    ? t('toolCards.designArtifact.manifestSubtitle', {
        kind: manifest.kind,
        filesCount,
        versionsCount,
      })
    : action === 'list'
      ? t('toolCards.designArtifact.listSubtitle', { count: manifests?.length ?? 0 })
      : actionLabel(action);

  const header = (
    <ToolCardHeader
      icon={<Palette size={14} />}
      iconClassName="design-artifact-index-card__icon"
      content={
        <div className="design-artifact-index-card__header">
          <div className="design-artifact-index-card__title-row">
            <span className="design-artifact-index-card__title">
              {manifest?.title || t('toolCards.designArtifact.defaultTitle')}
            </span>
            {manifest?.id && (
              <code className="design-artifact-index-card__id">{manifest.id}</code>
            )}
            <span className={`design-artifact-index-card__event design-artifact-index-card__event--${event}`}>
              {eventLabel(event)}
            </span>
          </div>
          <div className="design-artifact-index-card__subtitle">
            {isFailed ? failure : subtitle}
          </div>
        </div>
      }
      extra={
        manifest ? (
          <button
            type="button"
            className="design-artifact-index-card__open-btn"
            onClick={(e) => {
              e.stopPropagation();
              openInCanvas();
            }}
          >
            <ExternalLink size={12} />
            <span>{t('toolCards.designArtifact.openInCanvas')}</span>
          </button>
        ) : null
      }
    />
  );

  const details = manifest ? (
    <div className="design-artifact-index-card__details">
      <div className="design-artifact-index-card__stat">
        <FileStack size={12} />
        <span>{manifest.entry || '—'}</span>
      </div>
      {status !== 'completed' && streamingPath && (
        <div className="design-artifact-index-card__stat">
          <span>{t('toolCards.designArtifact.writing')}</span>
          <code>{streamingPath}</code>
        </div>
      )}
      {manifest.current_version && (
        <div className="design-artifact-index-card__stat">
          <GitBranch size={12} />
          <code>{manifest.current_version.slice(0, 8)}</code>
        </div>
      )}
    </div>
  ) : null;

  return (
    <BaseToolCard
      status={status}
      isExpanded={Boolean(manifest) && !isFailed}
      className="design-artifact-index-card"
      header={header}
      expandedContent={details}
      isFailed={isFailed}
    />
  );
};

export default DesignArtifactIndexCard;
