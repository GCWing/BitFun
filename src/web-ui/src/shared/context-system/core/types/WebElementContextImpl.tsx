 

import React from 'react';
import { Code2 } from 'lucide-react';
import type { WebElementContext, ValidationResult, RenderOptions } from '../../../types/context';
import type {
  ContextTransformer,
  ContextValidator,
  ContextCardRenderer,
} from '../../../services/ContextRegistry';
import { i18nService } from '@/infrastructure/i18n';



export class WebElementContextTransformer implements ContextTransformer<'web-element'> {
  readonly type = 'web-element' as const;

  transform(context: WebElementContext): unknown {
    return {
      type: 'web-element',
      id: context.id,
      tag_name: context.tagName,
      path: context.path,
      attributes: context.attributes,
      text_content: context.textContent,
      outer_html: context.outerHTML,
      source_url: context.sourceUrl ?? null,
    };
  }

  estimateSize(context: WebElementContext): number {
    return (
      context.path.length +
      context.outerHTML.length +
      JSON.stringify(context.attributes).length +
      (context.textContent?.length ?? 0)
    );
  }
}



export class WebElementContextValidator implements ContextValidator<'web-element'> {
  readonly type = 'web-element' as const;

  async validate(context: WebElementContext): Promise<ValidationResult> {
    if (!context.tagName) {
      return { valid: false, error: 'Web element must have a tag name.' };
    }
    if (!context.path) {
      return { valid: false, error: 'Web element must have a CSS path.' };
    }
    return { valid: true };
  }
}



export class WebElementCardRenderer implements ContextCardRenderer<'web-element'> {
  readonly type = 'web-element' as const;

  render(context: WebElementContext, options?: RenderOptions): React.ReactElement {
    const { compact = false } = options || {};

    const attrEntries = Object.entries(context.attributes).slice(0, 6);

    return (
      <div className="context-card web-element-context-card" data-compact={compact}>
        <div className="context-card__header">
          <div className="context-card__icon">
            <Code2 size={16} />
          </div>
          <div className="context-card__info">
            <div className="context-card__title">
              <span className="web-element-context-card__tag">&lt;{context.tagName}&gt;</span>
            </div>
            {!compact && context.sourceUrl && (
              <div className="context-card__meta">
                <span title={context.sourceUrl}>
                  {i18nService.t('components:contextSystem.webElement.from')}{' '}
                  {context.sourceUrl.length > 40
                    ? `${context.sourceUrl.slice(0, 40)}…`
                    : context.sourceUrl}
                </span>
              </div>
            )}
          </div>
        </div>

        {!compact && (
          <div className="web-element-context-card__details">
            <div className="web-element-context-card__path" title={context.path}>
              {context.path.length > 60 ? `…${context.path.slice(-60)}` : context.path}
            </div>
            {attrEntries.length > 0 && (
              <div className="web-element-context-card__attrs">
                {attrEntries.map(([k, v]) => (
                  <span key={k} className="web-element-context-card__attr">
                    <span className="web-element-context-card__attr-name">{k}</span>
                    {v && (
                      <>
                        <span className="web-element-context-card__attr-eq">=</span>
                        <span className="web-element-context-card__attr-val">
                          &quot;{v.length > 20 ? `${v.slice(0, 20)}…` : v}&quot;
                        </span>
                      </>
                    )}
                  </span>
                ))}
              </div>
            )}
            {context.textContent && (
              <div className="web-element-context-card__text">
                {context.textContent.length > 80
                  ? `${context.textContent.slice(0, 80)}…`
                  : context.textContent}
              </div>
            )}
          </div>
        )}
      </div>
    );
  }
}
