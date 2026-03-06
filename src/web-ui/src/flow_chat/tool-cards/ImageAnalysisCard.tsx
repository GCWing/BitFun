/**
 * Image analysis tool card - compact mode
 * Used for view_image tool
 */

import React, { useState, useMemo, useEffect, useCallback } from 'react';
import { Loader2, Clock, Check } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import { CompactToolCard, CompactToolCardHeader } from './CompactToolCard';
import './ImageAnalysisCard.scss';

const imageAnalysisExpandedStateCache = new Map<string, boolean>();

export const ImageAnalysisCard: React.FC<ToolCardProps> = ({
  toolItem,
  onExpand
}) => {
  const { t } = useTranslation('flow-chat');
  const { toolCall, toolResult, status } = toolItem;
  const toolId = toolItem.id || toolCall?.id;
  const [isExpanded, setIsExpanded] = useState(false);

  useEffect(() => {
    if (!toolId) return;
    const cached = imageAnalysisExpandedStateCache.get(toolId);
    setIsExpanded(cached ?? false);
  }, [toolId]);

  const getStatusIcon = () => {
    switch (status) {
      case 'running':
      case 'streaming':
        return <Loader2 className="animate-spin" size={12} />;
      case 'completed':
        return <Check size={12} className="icon-check-done" />;
      case 'pending':
      default:
        return <Clock size={12} />;
    }
  };

  const getAnalysisInfo = () => {
    const input = toolCall?.input;
    
    if (!input) {
      return {
        prompt: t('toolCards.imageAnalysis.parsingAnalysisInfo'),
        imagePath: null,
        imageName: t('toolCards.imageAnalysis.unknownImage'),
        focusAreas: [],
        detailLevel: 'normal'
      };
    }
    
    const imagePath = input.image_path || input.image_id || null;
    let imageName = t('toolCards.imageAnalysis.unknownImage');
    if (imagePath) {
      if (imagePath.startsWith('data:')) {
        imageName = t('toolCards.imageAnalysis.clipboardImage');
      } else {
        const parts = imagePath.split(/[/\\]/);
        imageName = parts[parts.length - 1];
      }
    }
    
    return {
      prompt: input.analysis_prompt || t('toolCards.imageAnalysis.analyzeImageContent'),
      imagePath,
      imageName,
      focusAreas: input.focus_areas || [],
      detailLevel: input.detail_level || 'normal'
    };
  };

  const getAnalysisResult = () => {
    if (!toolResult?.result) return null;

    const raw = toolResult.result;
    const result =
      (raw?.analysis || raw?.description || raw?.content) ? raw :
      (raw?.result?.analysis || raw?.result?.description || raw?.result?.content) ? raw.result :
      (raw?.data?.analysis || raw?.data?.description || raw?.data?.content) ? raw.data :
      null;

    if (result) {
      return {
        analysis: result.analysis || result.description || result.content,
        modelUsed: result.model_used || result.model,
        imagePath: result.image_path
      };
    }
    
    return null;
  };

  const handleToggleExpand = useCallback(() => {
    window.dispatchEvent(new CustomEvent('tool-card-toggle'));
    setIsExpanded(prev => {
      const next = !prev;
      if (toolId) {
        imageAnalysisExpandedStateCache.set(toolId, next);
      }
      return next;
    });
    onExpand?.();
  }, [onExpand, toolId]);

  const analysisInfo = useMemo(() => getAnalysisInfo(), [toolCall?.input]);
  const analysisResult = useMemo(() => getAnalysisResult(), [toolResult?.result]);
  const hasResultData = toolResult?.result !== undefined && toolResult?.result !== null;
  const canExpand = status === 'completed' && !!analysisResult;

  const handleCardClick = () => {
    if (canExpand) {
      handleToggleExpand();
    }
  };

  const renderContent = () => {
    if (status === 'completed') {
      return (
        <>
          {t('toolCards.imageAnalysis.imageAnalysis')}: {analysisInfo.imageName}
          {hasResultData && (
            <span style={{ color: 'var(--color-text-muted)', fontSize: '11px', marginLeft: '8px' }}>
              → {t('toolCards.imageAnalysis.completed')}
            </span>
          )}
        </>
      );
    }
    if (status === 'running' || status === 'streaming') {
      return <>{t('toolCards.imageAnalysis.analyzing')} {analysisInfo.imageName}...</>;
    }
    if (status === 'pending') {
      return <>{t('toolCards.imageAnalysis.preparing')} {analysisInfo.imageName}</>;
    }
    return null;
  };

  const expandedContent = useMemo(() => {
    if (!analysisResult) return null;
    
    return (
      <div className="image-analysis-expanded-content">
        {analysisInfo.prompt && (
          <div className="analysis-prompt-section">
            <div className="section-label">{t('toolCards.imageAnalysis.analysisPrompt')}</div>
            <div className="section-content">{analysisInfo.prompt}</div>
          </div>
        )}
        
        {analysisInfo.focusAreas && analysisInfo.focusAreas.length > 0 && (
          <div className="focus-areas-section">
            <div className="section-label">{t('toolCards.imageAnalysis.focusAreas')}</div>
            <div className="focus-areas-tags">
              {analysisInfo.focusAreas.map((area: string, index: number) => (
                <span key={index} className="focus-area-tag">{area}</span>
              ))}
            </div>
          </div>
        )}
        
        <div className="analysis-result-section">
          <div className="section-label">
            {t('toolCards.imageAnalysis.analysisResult')}
            {analysisResult.modelUsed && (
              <span className="model-badge">{analysisResult.modelUsed}</span>
            )}
          </div>
          <div className="analysis-content">
            {analysisResult.analysis}
          </div>
        </div>
      </div>
    );
  }, [analysisInfo, analysisResult, t]);

  // Important: do not conditionally skip hooks (e.g. useMemo) across renders.
  // Returning early here is safe because all hooks above have already run.
  if ((status as string) === 'error') {
    return null;
  }

  const normalizedStatus = status === 'analyzing' ? 'running' : status;

  return (
    <CompactToolCard
      status={normalizedStatus as 'pending' | 'preparing' | 'streaming' | 'running' | 'completed' | 'error' | 'cancelled'}
      isExpanded={isExpanded}
      onClick={handleCardClick}
      className="image-analysis-card"
      clickable={canExpand}
      header={
        <CompactToolCardHeader
          statusIcon={getStatusIcon()}
          content={renderContent()}
        />
      }
      expandedContent={expandedContent ?? undefined}
    />
  );
};
