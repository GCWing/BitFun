/**
 * MiniAppToolDisplay — InitMiniApp result; layout aligned with GitToolDisplay (BaseToolCard).
 */
import React, { useCallback, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { AppWindow, ExternalLink } from 'lucide-react';
import { CubeLoading } from '../../component-library';
import type { ToolCardProps } from '../types/flow-chat';
import { BaseToolCard, ToolCardHeader } from './BaseToolCard';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import { useOverlayManager } from '@/app/hooks/useOverlayManager';
import type { OverlaySceneId } from '@/app/overlay/types';
import './MiniAppToolDisplay.scss';

export const InitMiniAppDisplay: React.FC<ToolCardProps> = ({ toolItem }) => {
  const { t } = useTranslation('flow-chat');
  const { status, toolResult, partialParams, isParamsStreaming, toolCall } = toolItem;
  const { openOverlay } = useOverlayManager();
  const [isExpanded, setIsExpanded] = useState(false);

  const toolId = toolItem.id ?? toolCall?.id;
  const { cardRootRef, applyExpandedState } = useToolCardHeightContract({
    toolId,
    toolName: toolItem.toolName,
  });

  const name = useMemo(() => {
    if (isParamsStreaming) return (partialParams?.name as string | undefined) || '';
    return (toolCall?.input as Record<string, unknown> | undefined)?.name as string | undefined || '';
  }, [isParamsStreaming, partialParams, toolCall?.input]);

  const appId = toolResult?.result?.app_id as string | undefined;
  const path = toolResult?.result?.path as string | undefined;
  const success = toolResult?.success === true;
  const isLoading = status === 'running' || status === 'streaming' || status === 'preparing';
  const isFailed = status === 'error' || (status === 'completed' && toolResult != null && toolResult.success === false);

  const hasExpandableDetails =
    isFailed || (status === 'completed' && success && Boolean(appId));

  const toggleExpanded = useCallback(() => {
    applyExpandedState(isExpanded, !isExpanded, setIsExpanded);
  }, [applyExpandedState, isExpanded]);

  const handleCardClick = useCallback(
    (e: React.MouseEvent) => {
      if (!hasExpandableDetails) return;
      const target = e.target as HTMLElement;
      if (target.closest('.miniapp-action-buttons')) return;
      toggleExpanded();
    },
    [hasExpandableDetails, toggleExpanded]
  );

  const getErrorMessage = () => {
    if (toolResult && 'error' in toolResult && toolResult.error) {
      return String(toolResult.error);
    }
    return t('toolCards.initMiniApp.createFailed');
  };

  const commandText = useMemo(() => {
    if (isLoading) {
      return name || t('toolCards.initMiniApp.creatingShort');
    }
    if (isFailed) {
      return name || t('toolCards.initMiniApp.untitled');
    }
    return name || appId || t('toolCards.initMiniApp.untitled');
  }, [appId, isFailed, isLoading, name, t]);

  const renderStatusIcon = () => {
    if (isLoading) {
      return <CubeLoading size="small" />;
    }
    return null;
  };

  const renderHeader = () => (
    <ToolCardHeader
      icon={<AppWindow size={16} />}
      iconClassName="miniapp-icon"
      action={`${t('toolCards.initMiniApp.title')}:`}
      content={
        <span className="miniapp-tool-info">
          <span className="operation-tag">
            {isLoading
              ? t('toolCards.initMiniApp.operationInit')
              : isFailed
                ? t('toolCards.initMiniApp.operationInit')
                : t('toolCards.initMiniApp.skeletonReady')}
          </span>
          <span className="command-text">{commandText}</span>
        </span>
      }
      extra={
        <>
          {success && appId && status === 'completed' && (
            <span className="output-summary" title={appId}>
              {appId}
            </span>
          )}
          {isFailed && (
            <div className="error-indicator">
              <span className="error-text">{t('toolCards.initMiniApp.failed')}</span>
            </div>
          )}
        </>
      }
      statusIcon={renderStatusIcon()}
    />
  );

  const renderExpandedSuccess = () => {
    if (!appId) return null;
    return (
      <div className="miniapp-result-container">
        <div className="miniapp-result-rows">
          <div className="miniapp-result-row">
            <span className="miniapp-result-label">{t('toolCards.initMiniApp.labelAppId')}</span>
            <span className="miniapp-result-value" title={appId}>
              {appId}
            </span>
          </div>
          {path ? (
            <div className="miniapp-result-row">
              <span className="miniapp-result-label">{t('toolCards.initMiniApp.labelPath')}</span>
              <span className="miniapp-result-value" title={path}>
                {path}
              </span>
            </div>
          ) : null}
        </div>
        <div className="miniapp-result-footer miniapp-action-buttons">
          <button
            type="button"
            className="miniapp-open-btn"
            onClick={() => openOverlay(`miniapp:${appId}` as OverlaySceneId)}
            title={t('toolCards.initMiniApp.openInMiniAppTitle')}
          >
            <ExternalLink size={12} />
            <span>{t('toolCards.initMiniApp.openInMiniApp')}</span>
          </button>
        </div>
      </div>
    );
  };

  const renderExpandedError = () => (
    <div className="error-content">
      <div className="error-message">{getErrorMessage()}</div>
      {name ? (
        <div className="error-meta">
          <span className="error-operation">{t('toolCards.initMiniApp.nameLabel', { name })}</span>
        </div>
      ) : null}
    </div>
  );

  const renderDetailsWhenExpanded = (): React.ReactNode => {
    if (isFailed) {
      return renderExpandedError();
    }
    if (success && appId) {
      return renderExpandedSuccess();
    }
    return null;
  };

  return (
    <div ref={cardRootRef} data-tool-card-id={toolId ?? ''}>
      <BaseToolCard
        status={status}
        isExpanded={isExpanded}
        onClick={hasExpandableDetails ? handleCardClick : undefined}
        className="miniapp-tool-display"
        header={renderHeader()}
        expandedContent={isExpanded ? renderDetailsWhenExpanded() : null}
        headerExpandAffordance={hasExpandableDetails}
      />
    </div>
  );
};
