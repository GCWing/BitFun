/**
 * Image analysis card component.
 * Displays analysis progress and results.
 */

import React, { useState } from 'react';
import {
  Loader, 
  CheckCircle, 
  AlertCircle, 
  ChevronDown, 
  ChevronUp,
  Eye,
  Sparkles
} from 'lucide-react';
import type { FlowImageAnalysisItem } from '../types/flow-chat';
import { Button } from '@/component-library';
import './ImageAnalysisCard.scss';

export interface ImageAnalysisCardProps {
  analysisItem: FlowImageAnalysisItem;
  onRetry?: () => void;
  onExpand?: () => void;
}

export const ImageAnalysisCard: React.FC<ImageAnalysisCardProps> = ({
  analysisItem,
  onRetry,
}) => {
  const [expanded, setExpanded] = useState(false);
  const { imageContext, result, status, error } = analysisItem;
  
  const duration = result?.analysis_time_ms 
    ? `${result.analysis_time_ms}ms`
    : '';
  
  return (
    <div className="image-analysis-card" data-status={status}>
      <div className="image-analysis-card__header">
        <div className="image-analysis-card__thumbnail">
          {imageContext.thumbnailUrl || imageContext.dataUrl ? (
            <img 
              src={imageContext.thumbnailUrl || imageContext.dataUrl} 
              alt={imageContext.imageName}
            />
          ) : (
            <div className="image-analysis-card__thumbnail-placeholder">
              <Eye size={24} />
            </div>
          )}
        </div>
        
        <div className="image-analysis-card__info">
          <div className="image-analysis-card__filename">
            {imageContext.imageName}
          </div>
          
          {status === 'analyzing' && (
            <div className="image-analysis-card__status analyzing">
              <Loader className="spinner" size={14} />
              <span>AI is analyzing the image...</span>
            </div>
          )}
          
          {status === 'completed' && result && (
            <div className="image-analysis-card__status completed">
              <CheckCircle className="icon" size={14} />
              <span>Analysis complete</span>
              {duration && (
                <span className="time">{duration}</span>
              )}
            </div>
          )}
          
          {status === 'error' && (
            <div className="image-analysis-card__status error">
              <AlertCircle className="icon" size={14} />
              <span>Analysis failed</span>
              {onRetry && (
                <Button variant="secondary" size="small" className="retry-btn" onClick={onRetry}>
                  Retry
                </Button>
              )}
            </div>
          )}
        </div>
      </div>
      
      {status === 'completed' && result && (
        <div className="image-analysis-card__content">
          <div className="image-analysis-card__summary">
            <Sparkles size={14} className="summary-icon" />
            <span>{result.summary}</span>
          </div>
          
          <Button 
            variant="ghost"
            size="small"
            className="image-analysis-card__expand-btn"
            onClick={() => setExpanded(!expanded)}
          >
            {expanded ? (
              <>
                <ChevronUp size={14} /> Collapse details
              </>
            ) : (
              <>
                <ChevronDown size={14} /> View detailed analysis
              </>
            )}
          </Button>
          
          {expanded && (
            <div className="image-analysis-card__detailed">
              <div className="detail-section">
                <h4>Detailed description</h4>
                <p>{result.detailed_description}</p>
              </div>
              
              {result.detected_elements.length > 0 && (
                <div className="detail-section">
                  <h4>Key elements detected</h4>
                  <div className="tags">
                    {result.detected_elements.map((elem, idx) => (
                      <span key={idx} className="tag">{elem}</span>
                    ))}
                  </div>
                </div>
              )}
              
              <div className="detail-section metadata">
                <span className="meta-item">
                  Confidence: {(result.confidence * 100).toFixed(1)}%
                </span>
                <span className="meta-separator">•</span>
                <span className="meta-item">
                  Analysis time: {result.analysis_time_ms}ms
                </span>
              </div>
            </div>
          )}
        </div>
      )}
      
      {status === 'error' && error && (
        <div className="image-analysis-card__error">
          <AlertCircle size={16} />
          <span>{error}</span>
        </div>
      )}
    </div>
  );
};

export default ImageAnalysisCard;
