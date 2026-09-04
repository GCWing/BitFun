 

import React, { useMemo } from 'react';
import { Icon } from '@openbitfun/ui';
import { AlertCircle, Loader2 } from 'lucide-react';
import { ContextItem } from '../../../types/context';
import { contextRegistry } from '../../../services/ContextRegistry';
import { useContextStore, selectValidationState, selectIsValidating } from '../../../stores/contextStore';
import { useI18n } from '@/infrastructure/i18n';
import './ContextCard.scss';

export interface ContextCardProps {
  context: ContextItem;
  onRemove?: (id: string) => void;
  compact?: boolean;
  interactive?: boolean;
  showPreview?: boolean;
  className?: string;
}

export const ContextCard: React.FC<ContextCardProps> = ({
  context,
  onRemove,
  compact = false,
  interactive = true,
  showPreview = true,
  className = ''
}) => {
  const { t } = useI18n('components');
  
  const validationState = useContextStore(selectValidationState(context.id));
  const isValidating = useContextStore(selectIsValidating(context.id));
  
  
  const renderer = useMemo(() => {
    return contextRegistry.getRenderer(context.type);
  }, [context.type]);
  
  
  const definition = useMemo(() => {
    return contextRegistry.getDefinition(context.type);
  }, [context.type]);
  
  
  const handleRemove = (e: React.MouseEvent) => {
    e.stopPropagation();
    onRemove?.(context.id);
  };
  
  
  const content = renderer 
    ? renderer.render(context, { compact, interactive, showPreview })
    : (
      <div className="openbitfun-context-card__fallback">
        <div className="openbitfun-context-card__icon">
          <AlertCircle size={20} />
        </div>
        <div className="openbitfun-context-card__content">
          <div className="openbitfun-context-card__title">
            {t('contextSystem.contextCard.unknownType', { type: context.type })}
          </div>
        </div>
      </div>
    );
  
  
  const validationClass = validationState 
    ? validationState.valid 
      ? 'openbitfun-context-card--valid'
      : 'openbitfun-context-card--invalid'
    : '';
  
  return (
    <div 
      className={`
        openbitfun-context-card
        openbitfun-context-card--${context.type}
        ${validationClass}
        ${compact ? 'openbitfun-context-card--compact' : ''}
        ${interactive ? 'openbitfun-context-card--interactive' : ''}
        ${className}
      `.trim()}
      data-context-id={context.id}
      data-context-type={context.type}
      data-openbitfun-component="context-list"
      data-openbitfun-part="card"
      data-openbitfun-state={validationState ? (validationState.valid ? 'valid' : 'invalid') : undefined}
    >
      
      {definition && (
        <div 
          className="openbitfun-context-card__indicator"
          style={{ backgroundColor: definition.color }}
          data-openbitfun-component="context-list"
          data-openbitfun-part="cardIndicator"
        />
      )}
      
      
      <div className="openbitfun-context-card__body" data-openbitfun-component="context-list" data-openbitfun-part="cardBody">
        {content}
      </div>
      
      
      {interactive && (
        <div className="openbitfun-context-card__toolbar" data-openbitfun-component="context-list" data-openbitfun-part="cardToolbar">
          
          <div className="openbitfun-context-card__validation">
            {isValidating ? (
              <Loader2 size={14} className="openbitfun-context-card__spinner" />
            ) : validationState ? (
              validationState.valid ? (
                <Icon name="check-circle" size="sm" className="openbitfun-context-card__icon--success" />
              ) : (
                <span title={validationState.error}>
                  <AlertCircle 
                    size={14} 
                    className="openbitfun-context-card__icon--error"
                  />
                </span>
              )
            ) : null}
          </div>
          
          
          {onRemove && (
            <button
              className="openbitfun-context-card__remove-btn"
              onClick={handleRemove}
              title={t('contextSystem.contextCard.removeContext')}
            >
              <Icon name="xmark" size="sm" />
            </button>
          )}
        </div>
      )}
      
      
      {validationState && !validationState.valid && validationState.error && (
        <div className="openbitfun-context-card__error" data-openbitfun-component="context-list" data-openbitfun-part="cardError" data-openbitfun-state="invalid">
          <AlertCircle size={12} />
          <span>{validationState.error}</span>
        </div>
      )}
      
      
      {validationState && validationState.valid && validationState.warnings && validationState.warnings.length > 0 && (
        <div className="openbitfun-context-card__warnings">
          {validationState.warnings.map((warning, idx) => (
            <div key={idx} className="openbitfun-context-card__warning">
              <AlertCircle size={12} />
              <span>{warning}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

export default ContextCard;
