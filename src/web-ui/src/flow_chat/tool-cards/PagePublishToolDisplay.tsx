/**
 * PagePublish tool card — shows publish slug / version / URLs.
 */
import React, { useCallback, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ExternalLink, Rocket } from 'lucide-react';
import { CubeLoading } from '../../component-library';
import type { ToolCardProps } from '../types/flow-chat';
import { BaseToolCard, ToolCardHeader } from './BaseToolCard';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import { remoteConnectAPI } from '@/infrastructure/api/service-api/RemoteConnectAPI';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';

async function openPagePath(path: string | undefined) {
  if (!path) return;
  const hint = await remoteConnectAPI.accountGetCredentialHint();
  const relay = hint?.relay_url?.replace(/\/$/, '') ?? '';
  const href = path.startsWith('http')
    ? path
    : relay
      ? `${relay}${path.startsWith('/') ? '' : '/'}${path}`
      : path;
  if (href.startsWith('http')) {
    await systemAPI.openExternal(href);
  }
}

export const PagePublishDisplay: React.FC<ToolCardProps> = ({ toolItem }) => {
  const { t } = useTranslation('flow-chat');
  const { status, toolResult, partialParams, isParamsStreaming, toolCall } = toolItem;
  const [isExpanded, setIsExpanded] = useState(false);

  const toolId = toolItem.id ?? toolCall?.id;
  const { cardRootRef, applyExpandedState } = useToolCardHeightContract({
    toolId,
    toolName: toolItem.toolName,
  });

  const slug = useMemo(() => {
    if (isParamsStreaming) return (partialParams?.slug as string | undefined) || '';
    return (
      (toolCall?.input as Record<string, unknown> | undefined)?.slug as string | undefined
    ) || '';
  }, [isParamsStreaming, partialParams, toolCall?.input]);

  const versionId = useMemo(() => {
    if (isParamsStreaming) return '';
    return (toolResult?.result?.version_id as string | undefined) || '';
  }, [isParamsStreaming, toolResult?.result]);

  const urlPath =
    (toolResult?.result?.url as string | undefined) ||
    (toolResult?.result?.url_path as string | undefined);
  const previewPath =
    (toolResult?.result?.preview_url as string | undefined) ||
    (toolResult?.result?.preview_url_path as string | undefined);
  const deployed = toolResult?.result?.deployed === true;
  const success = toolResult?.success === true;
  const isLoading = status === 'running' || status === 'streaming' || status === 'preparing';
  const isFailed =
    status === 'error' ||
    (status === 'completed' && toolResult != null && toolResult.success === false);

  const hasExpandableDetails =
    isFailed || (status === 'completed' && success && Boolean(slug || versionId));

  const toggleExpanded = useCallback(() => {
    applyExpandedState(isExpanded, !isExpanded, setIsExpanded);
  }, [applyExpandedState, isExpanded]);

  const handleCardClick = useCallback(
    (e: React.MouseEvent) => {
      if (!hasExpandableDetails) return;
      const target = e.target as HTMLElement;
      if (target.closest('.page-publish-action-buttons')) return;
      toggleExpanded();
    },
    [hasExpandableDetails, toggleExpanded]
  );

  const getErrorMessage = () => {
    if (toolResult && 'error' in toolResult && toolResult.error) {
      return String(toolResult.error);
    }
    return t('toolCards.pagePublish.publishFailed');
  };

  const commandText = useMemo(() => {
    if (isLoading) {
      return slug || t('toolCards.pagePublish.publishingShort');
    }
    return slug || t('toolCards.pagePublish.untitled');
  }, [isLoading, slug, t]);

  const renderStatusIcon = () => {
    if (isLoading) {
      return <CubeLoading size="small" />;
    }
    return null;
  };

  const renderHeader = () => (
    <ToolCardHeader
      icon={<Rocket size={16} />}
      action={`${t('toolCards.pagePublish.title')}:`}
      content={
        <span className="command-text" data-testid="chat-page-publish-title">
          {commandText}
          {versionId ? ` @ ${versionId}` : ''}
        </span>
      }
      statusIcon={renderStatusIcon()}
    />
  );

  const renderExpandedSuccess = () => (
    <div className="page-publish-result">
      {slug && (
        <div>
          {t('toolCards.pagePublish.labelSlug')}: {slug}
        </div>
      )}
      {versionId && (
        <div>
          {t('toolCards.pagePublish.labelVersion')}: {versionId}
        </div>
      )}
      {deployed && urlPath && (
        <div>
          {t('toolCards.pagePublish.labelPath')}: {urlPath}
        </div>
      )}
      {!deployed && previewPath && (
        <div>
          {t('toolCards.pagePublish.labelPreview')}: {previewPath}
        </div>
      )}
      <div className="page-publish-action-buttons">
        {deployed && urlPath && (
          <button
            type="button"
            data-testid="chat-page-publish-open-prod-btn"
            onClick={() => void openPagePath(urlPath)}
          >
            <ExternalLink size={12} />
            <span>{t('toolCards.pagePublish.openProduction')}</span>
          </button>
        )}
        {previewPath && (
          <button
            type="button"
            data-testid="chat-page-publish-open-preview-btn"
            onClick={() => void openPagePath(previewPath)}
          >
            <ExternalLink size={12} />
            <span>{t('toolCards.pagePublish.openPreview')}</span>
          </button>
        )}
      </div>
    </div>
  );

  const renderExpandedError = () => (
    <div className="error-content">
      <div className="error-message">{getErrorMessage()}</div>
    </div>
  );

  const renderDetailsWhenExpanded = (): React.ReactNode => {
    if (isFailed) return renderExpandedError();
    if (success) return renderExpandedSuccess();
    return null;
  };

  return (
    <div
      ref={cardRootRef}
      data-testid="chat-page-publish-card"
      data-tool-card-id={toolId ?? ''}
      data-status={status}
      data-expanded={isExpanded ? 'true' : 'false'}
    >
      <BaseToolCard
        status={status}
        isExpanded={isExpanded}
        onClick={hasExpandableDetails ? handleCardClick : undefined}
        className="page-publish-tool-display"
        header={renderHeader()}
        expandedContent={isExpanded ? renderDetailsWhenExpanded() : null}
        headerExpandAffordance={hasExpandableDetails}
      />
    </div>
  );
};
