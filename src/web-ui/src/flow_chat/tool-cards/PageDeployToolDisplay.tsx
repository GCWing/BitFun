/**
 * PageDeploy tool card — shows deploy slug / version result.
 */
import React, { useCallback, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ExternalLink, Rocket } from 'lucide-react';
import { CubeLoading } from '../../component-library';
import type { ToolCardProps } from '../types/flow-chat';
import { BaseToolCard, ToolCardHeader } from './BaseToolCard';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import { pageAPI } from '@/infrastructure/api/service-api/PageAPI';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import { notificationService } from '@/shared/notification-system';

async function openPage(slug: string, knownGeneration?: string) {
  if (!slug) return;
  const generation = knownGeneration || (await pageAPI.listPages())
    .find((page) => page.slug === slug)?.generation;
  if (generation == null) throw new Error('Page no longer exists');
  const link = await pageAPI.createOpenLink(slug, generation);
  await systemAPI.openExternal(link.open_url);
}

export const PageDeployDisplay: React.FC<ToolCardProps> = ({ toolItem }) => {
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
    if (isParamsStreaming) return (partialParams?.version_id as string | undefined) || '';
    return (
      (toolCall?.input as Record<string, unknown> | undefined)?.version_id as
        | string
        | undefined
    ) || '';
  }, [isParamsStreaming, partialParams, toolCall?.input]);

  const deployedVersion =
    (toolResult?.result?.deployed_version_id as string | undefined) || versionId;
  const generation = toolResult?.result?.generation as string | undefined;
  const urlPath =
    (toolResult?.result?.url as string | undefined) ||
    (toolResult?.result?.url_path as string | undefined);
  const success = toolResult?.success === true;
  const isLoading = status === 'running' || status === 'streaming' || status === 'preparing';
  const isFailed =
    status === 'error' ||
    (status === 'completed' && toolResult != null && toolResult.success === false);

  const hasExpandableDetails =
    isFailed || (status === 'completed' && success && Boolean(slug || deployedVersion));

  const toggleExpanded = useCallback(() => {
    applyExpandedState(isExpanded, !isExpanded, setIsExpanded);
  }, [applyExpandedState, isExpanded]);

  const handleCardClick = useCallback(
    (e: React.MouseEvent) => {
      if (!hasExpandableDetails) return;
      const target = e.target as HTMLElement;
      if (target.closest('.page-deploy-action-buttons')) return;
      toggleExpanded();
    },
    [hasExpandableDetails, toggleExpanded]
  );

  const getErrorMessage = () => {
    if (toolResult && 'error' in toolResult && toolResult.error) {
      return String(toolResult.error);
    }
    return t('toolCards.pageDeploy.deployFailed');
  };

  const commandText = useMemo(() => {
    if (isLoading) {
      return slug || t('toolCards.pageDeploy.deployingShort');
    }
    return slug || t('toolCards.pageDeploy.untitled');
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
      action={`${t('toolCards.pageDeploy.title')}:`}
      content={
        <span className="command-text" data-testid="chat-page-deploy-title">
          {commandText}
          {deployedVersion ? ` @ ${deployedVersion}` : ''}
        </span>
      }
      statusIcon={renderStatusIcon()}
    />
  );

  const renderExpandedSuccess = () => (
    <div className="page-deploy-result">
      {slug && (
        <div>
          {t('toolCards.pageDeploy.labelSlug')}: {slug}
        </div>
      )}
      {deployedVersion && (
        <div>
          {t('toolCards.pageDeploy.labelVersion')}: {deployedVersion}
        </div>
      )}
      {urlPath && (
        <div>
          {t('toolCards.pageDeploy.labelPath')}: {urlPath}
        </div>
      )}
      {urlPath && (
        <div className="page-deploy-action-buttons">
          <button
            type="button"
            data-testid="chat-page-deploy-open-btn"
            onClick={() => void openPage(slug, generation).catch(() => {
              notificationService.error(t('toolCards.pageDeploy.openFailed'));
            })}
          >
            <ExternalLink size={12} />
            <span>{t('toolCards.pageDeploy.openProduction')}</span>
          </button>
        </div>
      )}
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
      data-testid="chat-page-deploy-card"
      data-tool-card-id={toolId ?? ''}
      data-status={status}
      data-expanded={isExpanded ? 'true' : 'false'}
    >
      <BaseToolCard
        status={status}
        isExpanded={isExpanded}
        onClick={hasExpandableDetails ? handleCardClick : undefined}
        className="page-deploy-tool-display"
        header={renderHeader()}
        expandedContent={isExpanded ? renderDetailsWhenExpanded() : null}
        headerExpandAffordance={hasExpandableDetails}
      />
    </div>
  );
};
