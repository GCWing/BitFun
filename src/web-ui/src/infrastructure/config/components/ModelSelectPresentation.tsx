import { useCallback } from 'react';
import { Sparkles } from 'lucide-react';
import { type SelectOption } from '@/component-library';
import { getProviderDisplayName } from '../services/modelConfigs';
import { getEffectiveReasoningMode, isReasoningVisiblyEnabled } from '../utils/reasoning';
import type { AIModelConfig } from '../types';
import './ModelSelectPresentation.scss';

export type ModelSelectOption = SelectOption & {
  meta?: string;
  enableThinking?: boolean;
};

export function useModelSelectPresentation() {
  const formatContextWindow = useCallback((contextWindow?: number) => {
    if (!contextWindow) return null;
    return `${Math.round(contextWindow / 1000)}k`;
  }, []);

  const buildModelOption = useCallback((model: AIModelConfig): ModelSelectOption => {
    const meta = [getProviderDisplayName(model)];
    const contextWindow = formatContextWindow(model.context_window);

    if (contextWindow) {
      meta.push(contextWindow);
    }
    if (model.reasoning_effort) {
      meta.push(model.reasoning_effort);
    }

    return {
      label: model.model_name || model.name || model.id || '',
      value: model.id || '',
      meta: meta.join(' · '),
      enableThinking: isReasoningVisiblyEnabled(getEffectiveReasoningMode(model)),
    };
  }, [formatContextWindow]);

  const renderModelOption = useCallback((option: SelectOption) => {
    const modelOption = option as ModelSelectOption;

    return (
      <div className="model-select-presentation__option">
        <div className="model-select-presentation__option-title">
          <span className="model-select-presentation__option-name">{modelOption.label}</span>
          {modelOption.enableThinking && (
            <Sparkles size={12} className="model-select-presentation__thinking" />
          )}
        </div>
        {modelOption.meta && (
          <div className="model-select-presentation__option-meta">{modelOption.meta}</div>
        )}
      </div>
    );
  }, []);

  const renderModelValue = useCallback((option?: SelectOption | SelectOption[]) => {
    const selectedOption = Array.isArray(option) ? option[0] : option;
    if (!selectedOption) return null;

    const modelOption = selectedOption as ModelSelectOption;
    return (
      <span
        className={[
          'select__value',
          'model-select-presentation__value',
          !modelOption.meta && 'model-select-presentation__value--single-line',
        ].filter(Boolean).join(' ')}
      >
        <span className="model-select-presentation__value-text">
          <span className="model-select-presentation__value-title">
            <span className="model-select-presentation__value-name">{modelOption.label}</span>
            {modelOption.enableThinking && (
              <Sparkles size={12} className="model-select-presentation__thinking" />
            )}
          </span>
          {modelOption.meta && (
            <span className="model-select-presentation__value-meta">{modelOption.meta}</span>
          )}
        </span>
      </span>
    );
  }, []);

  return { buildModelOption, renderModelOption, renderModelValue };
}
