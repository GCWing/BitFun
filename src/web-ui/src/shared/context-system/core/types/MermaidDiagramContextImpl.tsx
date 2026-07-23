 

import React from 'react';
import { Network, CheckCircle } from 'lucide-react';
import type { MermaidDiagramContext, ValidationResult, RenderOptions } from '../../../types/context';
import type { 
  ContextTransformer, 
  ContextValidator, 
  ContextCardRenderer 
} from '../../../services/ContextRegistry';
import { i18nService } from '@/infrastructure/i18n';



export class MermaidDiagramContextTransformer implements ContextTransformer<'mermaid-diagram'> {
  readonly type = 'mermaid-diagram' as const;
  
  transform(context: MermaidDiagramContext): unknown {
    
    return {
      type: 'mermaid-diagram',
      code: context.diagramCode,
      title: context.diagramTitle,
      diagramType: context.diagramType,
    };
  }
  
  estimateSize(context: MermaidDiagramContext): number {
    
    return context.diagramCode.length;
  }
}



export class MermaidDiagramContextValidator implements ContextValidator<'mermaid-diagram'> {
  readonly type = 'mermaid-diagram' as const;
  
  async validate(context: MermaidDiagramContext): Promise<ValidationResult> {
    try {
      
      if (!context.diagramCode || context.diagramCode.trim() === '') {
        return {
          valid: false,
          error: 'Mermaid code is empty.'
        };
      }
      
      
      const warnings: string[] = [];
      if (context.diagramCode.length > 10000) {
        warnings.push(i18nService.t('components:contextSystem.validation.warnings.mermaidCodeLong', { maxChars: 10000 }));
      }
      if (context.diagramCode.length > 50000) {
        return {
          valid: false,
          error: 'Code is too long (>50000 characters). Cannot process.'
        };
      }
      
      
      const firstLine = context.diagramCode.trim().split('\n')[0];
      const validStarters = ['graph', 'flowchart', 'sequencediagram', 'classdiagram', 'statediagram', 'erdiagram', 'gantt', 'pie', 'journey'];
      const isValid = validStarters.some(starter => firstLine.toLowerCase().includes(starter));
      
      if (!isValid) {
        warnings.push(i18nService.t('components:contextSystem.validation.warnings.mermaidCodeInvalid'));
      }
      
      return {
        valid: true,
        warnings: warnings.length > 0 ? warnings : undefined
      };
    } catch (error) {
      return {
        valid: false,
        error: `Validation failed: ${String(error)}`
      };
    }
  }
  
  quickValidate(context: MermaidDiagramContext): ValidationResult {
    
    if (!context.diagramCode || context.diagramCode.trim() === '') {
      return { valid: false, error: 'Mermaid code is empty.' };
    }
    
    if (context.diagramCode.length > 50000) {
      return { valid: false, error: 'Code is too long (>50000 characters).' };
    }
    
    return { valid: true };
  }
}



export class MermaidDiagramCardRenderer implements ContextCardRenderer<'mermaid-diagram'> {
  readonly type = 'mermaid-diagram' as const;
  
  render(context: MermaidDiagramContext, options?: RenderOptions): React.ReactNode {
    const { compact = false, interactive = true } = options || {};
    
    return (
      <div className={`bitfun-context-card bitfun-context-card--mermaid ${compact ? 'bitfun-context-card--compact' : ''}`}>
        <div className="bitfun-context-card__icon">
          <Network size={compact ? 16 : 20} />
        </div>
        
        <div className="bitfun-context-card__content">
          <div className="bitfun-context-card__title">
            {context.diagramTitle || i18nService.t('components:contextSystem.diagram.defaultTitle')}
          </div>
          
          {!compact && (
            <div className="bitfun-context-card__subtitle">
              {this.getDiagramTypeLabel(context.diagramType)}
              <span className="bitfun-context-card__meta">
                {' • '}{this.formatCodeSize(context.diagramCode.length)}
              </span>
            </div>
          )}
        </div>
        
        {interactive && (
          <div className="bitfun-context-card__actions">
            {this.renderValidationIndicator(context)}
          </div>
        )}
      </div>
    );
  }
  
  private getDiagramTypeLabel(diagramType?: string): string {
    const typeMap: Record<string, string> = {
      'flowchart': i18nService.t('components:contextSystem.diagram.types.flowchart'),
      'sequence': i18nService.t('components:contextSystem.diagram.types.sequence'),
      'class': i18nService.t('components:contextSystem.diagram.types.class'),
      'state': i18nService.t('components:contextSystem.diagram.types.state'),
      'er': i18nService.t('components:contextSystem.diagram.types.er'),
      'gantt': i18nService.t('components:contextSystem.diagram.types.gantt'),
      'other': i18nService.t('components:contextSystem.diagram.types.other')
    };
    
    return typeMap[diagramType || 'other'] || i18nService.t('components:contextSystem.diagram.types.other');
  }
  
  private formatCodeSize(length: number): string {
    if (length < 1000) {
      return i18nService.t('components:contextSystem.diagram.codeSize.chars', { count: length });
    }
    return i18nService.t('components:contextSystem.diagram.codeSize.kChars', {
      count: (length / 1000).toFixed(1)
    });
  }
  
  private renderValidationIndicator(_context: MermaidDiagramContext): React.ReactNode {
    
    
    return (
      <div className="bitfun-context-card__status">
        <CheckCircle size={16} className="bitfun-context-card__status-icon--success" />
      </div>
    );
  }
}

