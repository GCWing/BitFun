/**
 * Image analysis card component.
 * Displays analysis progress and results.
 */

import { Button } from '@bitfun/ui';
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
    <div data-bf-component="image-analysis-card" data-bf-part="root" data-bf-status={status} data-bf-state={expanded ? 'expanded' : ''} className="image-analysis-card" data-status={status}>
      <div data-bf-component="image-analysis-card" data-bf-part="header" className="image-analysis-card__header">
        <div data-bf-component="image-analysis-card" data-bf-part="thumbnail" className="image-analysis-card__thumbnail">
          {imageContext.thumbnailUrl || imageContext.dataUrl ? (
            <img 
              src={imageContext.thumbnailUrl || imageContext.dataUrl} 
              alt={imageContext.imageName}
            />
          ) : (
            <div data-bf-component="image-analysis-card" data-bf-part="placeholder" className="image-analysis-card__thumbnail-placeholder">
              <Eye size={24} />
            </div>
          )}
        </div>
        
        <div data-bf-component="image-analysis-card" data-bf-part="info" className="image-analysis-card__info">
          <div data-bf-component="image-analysis-card" data-bf-part="filename" className="image-analysis-card__filename">
            {imageContext.imageName}
          </div>
          
          {status === 'analyzing' && (
            <div data-bf-component="image-analysis-card" data-bf-part="status" className="image-analysis-card__status analyzing">
              <Loader className="spinner" size={14} />
              <span>AI is analyzing the image...</span>
            </div>
          )}
          
          {status === 'completed' && result && (
            <div data-bf-component="image-analysis-card" data-bf-part="status" className="image-analysis-card__status completed">
              <CheckCircle className="icon" size={14} />
              <span>Analysis complete</span>
              {duration && (
                <span className="time">{duration}</span>
              )}
            </div>
          )}
          
          {status === 'error' && (
            <div data-bf-component="image-analysis-card" data-bf-part="status" className="image-analysis-card__status error">
              <AlertCircle className="icon" size={14} />
              <span>Analysis failed</span>
              {onRetry && (
                <Button
                  className="image-analysis-card__retry"
                  variant="outline"
                  size="sm"
                  onClick={onRetry}
                >
                  Retry
                </Button>
              )}
            </div>
          )}
        </div>
      </div>
      
      {status === 'completed' && result && (
        <div data-bf-component="image-analysis-card" data-bf-part="content" className="image-analysis-card__content">
          <div data-bf-component="image-analysis-card" data-bf-part="summary" className="image-analysis-card__summary">
            <Sparkles size={14} className="summary-icon" />
            <span>{result.summary}</span>
          </div>
          
          <Button
            className="image-analysis-card__toggle"
            variant="outline"
            size="sm"
            leadingIcon={expanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
            onClick={() => setExpanded(!expanded)}
          >
            {expanded ? 'Collapse details' : 'View detailed analysis'}
          </Button>
          
          {expanded && (
            <div data-bf-component="image-analysis-card" data-bf-part="details" className="image-analysis-card__detailed">
              <div data-bf-component="image-analysis-card" data-bf-part="section" className="detail-section">
                <h4>Detailed description</h4>
                <p>{result.detailed_description}</p>
              </div>
              
              {result.detected_elements.length > 0 && (
                <div data-bf-component="image-analysis-card" data-bf-part="section" className="detail-section">
                  <h4>Key elements detected</h4>
                  <div data-bf-component="image-analysis-card" data-bf-part="tags" className="tags">
                    {result.detected_elements.map((elem, idx) => (
                      <span key={idx} className="tag">{elem}</span>
                    ))}
                  </div>
                </div>
              )}
              
              <div data-bf-component="image-analysis-card" data-bf-part="metadata" className="detail-section metadata">
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
        <div data-bf-component="image-analysis-card" data-bf-part="error" className="image-analysis-card__error">
          <AlertCircle size={16} />
          <span>{error}</span>
        </div>
      )}
    </div>
  );
};

export default ImageAnalysisCard;
