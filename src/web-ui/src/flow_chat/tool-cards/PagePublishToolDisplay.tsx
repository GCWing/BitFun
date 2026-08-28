/**
 * PagePublish tool card — shows publish slug / version / URLs.
 */
import React, { useCallback, useMemo, useState } from 'react';
import { Button } from '@bitfun/ui';
import { useTranslation } from 'react-i18next';
import { ExternalLink } from 'lucide-react';
import type { ToolCardProps } from '../types/flow-chat';
import { PagePublishToolCard } from '@bitfun/ui/flow-chat';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import { pageAPI } from '@/infrastructure/api/service-api/PageAPI';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import { notificationService } from '@/shared/notification-system';

async function openPage(slug: string, knownGeneration?: string, versionId?: string) {
  if (!slug) return;
  const generation = knownGeneration || (await pageAPI.listPages())
    .find((page) => page.slug === slug)?.generation;
  if (generation == null) throw new Error('Page no longer exists');
  const link = await pageAPI.createOpenLink(slug, generation, versionId);
  await systemAPI.openExternal(link.open_url);
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
  const generation = toolResult?.result?.generation as string | undefined;

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

  const fields = success ? [
    slug ? { label: `${t('toolCards.pagePublish.labelSlug')}:`, value: slug, monospace: true } : null,
    versionId ? { label: `${t('toolCards.pagePublish.labelVersion')}:`, value: versionId, monospace: true } : null,
    deployed && urlPath ? { label: `${t('toolCards.pagePublish.labelPath')}:`, value: urlPath, monospace: true } : null,
    !deployed && previewPath ? { label: `${t('toolCards.pagePublish.labelPreview')}:`, value: previewPath, monospace: true } : null,
  ].filter((field): field is NonNullable<typeof field> => Boolean(field)) : [];

  return (
    <div
      ref={cardRootRef}
      data-bf-adapter="page-publish"
      data-testid="chat-page-publish-card"
      data-tool-card-id={toolId ?? ''}
      data-status={status}
      data-expanded={isExpanded ? 'true' : 'false'}
    >
      <PagePublishToolCard
        status={isFailed ? 'error' : status}
        isExpanded={isExpanded}
        onToggle={hasExpandableDetails ? toggleExpanded : undefined}
        action={`${t('toolCards.pagePublish.title')}:`}
        subject={commandText}
        version={versionId || undefined}
        loading={isLoading}
        fields={fields}
        error={isFailed ? getErrorMessage() : undefined}
        actions={success ? (
          <>
            {deployed && urlPath && (
              <Button
                type="button"
                variant="fill"
                size="sm"
                leadingIcon={<ExternalLink size={12} />}
                data-testid="chat-page-publish-open-prod-btn"
                onClick={() => void openPage(slug, generation).catch(() => {
                  notificationService.error(t('toolCards.pagePublish.openFailed'));
                })}
              >
                {t('toolCards.pagePublish.openProduction')}
              </Button>
            )}
            {previewPath && (
              <Button
                type="button"
                variant="outline"
                size="sm"
                leadingIcon={<ExternalLink size={12} />}
                data-testid="chat-page-publish-open-preview-btn"
                onClick={() => void openPage(slug, generation, versionId).catch(() => {
                  notificationService.error(t('toolCards.pagePublish.openFailed'));
                })}
              >
                {t('toolCards.pagePublish.openPreview')}
              </Button>
            )}
          </>
        ) : undefined}
      />
    </div>
  );
};
