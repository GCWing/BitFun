import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { BrainCircuit, Check, ChevronDown } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { getProviderDisplayName } from '@/infrastructure/config/services/modelConfigs';
import type { AIModelConfig, DefaultModelsConfig } from '@/infrastructure/config/types';
import { Tooltip } from '@/component-library';
import { createLogger } from '@/shared/utils/logger';
import type { ModelBrainstormContextMode } from '../store/modelBrainstormStore';
import {
  MODEL_BRAINSTORM_MAX_CANDIDATES,
  MODEL_BRAINSTORM_MIN_CANDIDATES,
} from '../services/ModelBrainstormService';
import './ModelBrainstormControl.scss';

const log = createLogger('ModelBrainstormControl');

interface BrainstormModelOption {
  id: string;
  label: string;
  providerName: string;
}

interface ModelBrainstormControlProps {
  enabled: boolean;
  selectedModelIds: string[];
  contextMode: ModelBrainstormContextMode;
  disabled?: boolean;
  onEnabledChange: (enabled: boolean) => void;
  onSelectedModelIdsChange: (modelIds: string[]) => void;
  onContextModeChange: (contextMode: ModelBrainstormContextMode) => void;
}

function isTextChatModel(model: AIModelConfig): model is AIModelConfig & { id: string } {
  if (!model.enabled || !model.id) {
    return false;
  }

  const capabilities = Array.isArray(model.capabilities) ? model.capabilities : [];
  return capabilities.includes('text_chat');
}

function sameModelIds(left: string[], right: string[]): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function resolveDefaultSelection(
  models: BrainstormModelOption[],
  defaultModels: DefaultModelsConfig,
): string[] {
  const availableIds = new Set(models.map(model => model.id));
  const preferred = [
    defaultModels.primary,
    defaultModels.fast,
  ].filter((modelId): modelId is string => Boolean(modelId && availableIds.has(modelId)));

  const result: string[] = [];
  for (const modelId of preferred) {
    if (!result.includes(modelId)) {
      result.push(modelId);
    }
  }

  for (const model of models) {
    if (result.length >= Math.min(MODEL_BRAINSTORM_MAX_CANDIDATES, Math.max(MODEL_BRAINSTORM_MIN_CANDIDATES, 3))) {
      break;
    }
    if (!result.includes(model.id)) {
      result.push(model.id);
    }
  }

  return result.slice(0, MODEL_BRAINSTORM_MAX_CANDIDATES);
}

export const ModelBrainstormControl: React.FC<ModelBrainstormControlProps> = ({
  enabled,
  selectedModelIds,
  contextMode,
  disabled = false,
  onEnabledChange,
  onSelectedModelIdsChange,
  onContextModeChange,
}) => {
  const { t } = useTranslation('flow-chat');
  const [models, setModels] = useState<BrainstormModelOption[]>([]);
  const [defaultModels, setDefaultModels] = useState<DefaultModelsConfig>({});
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const hostRef = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const hasUserEditedSelectionRef = useRef(false);
  const [menuStyle, setMenuStyle] = useState<React.CSSProperties>({
    position: 'fixed',
    visibility: 'hidden',
  });

  const loadModels = useCallback(async () => {
    try {
      const configData = await configManager.getConfigs(['ai.models', 'ai.default_models']);
      const allModels = (configData['ai.models'] as AIModelConfig[] | undefined) || [];
      const defaultModelConfig = (configData['ai.default_models'] as DefaultModelsConfig | undefined) || {};
      setDefaultModels(defaultModelConfig);
      setModels(allModels
        .filter(isTextChatModel)
        .map(model => ({
          id: model.id,
          label: model.model_name || model.name || model.id,
          providerName: getProviderDisplayName(model),
        })));
    } catch (error) {
      log.error('Failed to load brainstorm model options', { error });
      setModels([]);
    }
  }, []);

  useEffect(() => {
    loadModels();
    const unsubscribe = configManager.onConfigChange((path) => {
      if (path.startsWith('ai.')) {
        loadModels();
      }
    });

    return unsubscribe;
  }, [loadModels]);

  const availableModelIds = useMemo(() => new Set(models.map(model => model.id)), [models]);
  const selectedAvailableModelIds = useMemo(
    () => selectedModelIds.filter(modelId => availableModelIds.has(modelId)),
    [availableModelIds, selectedModelIds],
  );

  useEffect(() => {
    if (models.length === 0) {
      return;
    }

    if (selectedModelIds.length === 0 && !hasUserEditedSelectionRef.current) {
      const defaults = resolveDefaultSelection(models, defaultModels);
      if (defaults.length > 0) {
        onSelectedModelIdsChange(defaults);
      }
      return;
    }

    if (!sameModelIds(selectedAvailableModelIds, selectedModelIds)) {
      onSelectedModelIdsChange(selectedAvailableModelIds);
    }
  }, [
    defaultModels,
    models,
    onSelectedModelIdsChange,
    selectedAvailableModelIds,
    selectedModelIds,
  ]);

  useEffect(() => {
    if (!dropdownOpen || !hostRef.current) {
      return;
    }

    const updatePosition = () => {
      if (!hostRef.current) {
        return;
      }
      const rect = hostRef.current.getBoundingClientRect();
      setMenuStyle({
        position: 'fixed',
        visibility: 'visible',
        right: `${Math.max(12, window.innerWidth - rect.right)}px`,
        bottom: `${window.innerHeight - rect.top + 6}px`,
      });
    };

    updatePosition();
    window.addEventListener('resize', updatePosition);
    window.addEventListener('scroll', updatePosition, true);

    return () => {
      window.removeEventListener('resize', updatePosition);
      window.removeEventListener('scroll', updatePosition, true);
    };
  }, [dropdownOpen]);

  useEffect(() => {
    if (!dropdownOpen) {
      return;
    }

    const handleMouseDown = (event: MouseEvent) => {
      const target = event.target as Node;
      if (hostRef.current?.contains(target) || menuRef.current?.contains(target)) {
        return;
      }
      setDropdownOpen(false);
    };

    document.addEventListener('mousedown', handleMouseDown);
    return () => document.removeEventListener('mousedown', handleMouseDown);
  }, [dropdownOpen]);

  const hasEnoughModels = models.length >= MODEL_BRAINSTORM_MIN_CANDIDATES;
  const selectedCount = selectedAvailableModelIds.length;
  const controlDisabled = disabled || !hasEnoughModels;
  const tooltip = !hasEnoughModels
    ? t('modelBrainstorm.notEnoughModels')
    : enabled
      ? t('modelBrainstorm.enabledTooltip', { count: selectedCount })
      : t('modelBrainstorm.disabledTooltip');
  const contextModeLabels: Record<ModelBrainstormContextMode, string> = {
    independent: t('modelBrainstorm.contextModes.independent'),
    shared: t('modelBrainstorm.contextModes.shared'),
  };

  const toggleModel = useCallback((modelId: string) => {
    const isSelected = selectedAvailableModelIds.includes(modelId);
    hasUserEditedSelectionRef.current = true;
    if (isSelected) {
      onSelectedModelIdsChange(selectedAvailableModelIds.filter(id => id !== modelId));
      return;
    }

    if (selectedAvailableModelIds.length >= MODEL_BRAINSTORM_MAX_CANDIDATES) {
      return;
    }

    onSelectedModelIdsChange([...selectedAvailableModelIds, modelId]);
  }, [onSelectedModelIdsChange, selectedAvailableModelIds]);

  return (
    <div ref={hostRef} className="model-brainstorm-control">
      <Tooltip content={tooltip}>
        <button
          type="button"
          className={[
            'model-brainstorm-control__trigger',
            enabled ? 'model-brainstorm-control__trigger--enabled' : '',
            dropdownOpen ? 'model-brainstorm-control__trigger--open' : '',
          ].filter(Boolean).join(' ')}
          disabled={controlDisabled}
          data-testid="model-brainstorm-trigger"
          aria-pressed={enabled}
          onClick={(event) => {
            event.stopPropagation();
            if (controlDisabled) {
              return;
            }
            onEnabledChange(!enabled);
            setDropdownOpen(true);
          }}
        >
          <BrainCircuit size={13} strokeWidth={2.1} />
          <span className="model-brainstorm-control__label">{t('modelBrainstorm.triggerLabel')}</span>
          {enabled && (
            <span className="model-brainstorm-control__count">{selectedCount}</span>
          )}
          <ChevronDown size={10} className="model-brainstorm-control__chevron" />
        </button>
      </Tooltip>

      {dropdownOpen && createPortal(
        <div
          ref={menuRef}
          className="model-brainstorm-control__menu"
          style={menuStyle}
          data-testid="model-brainstorm-menu"
        >
          <div className="model-brainstorm-control__menu-header">
            <span>{t('modelBrainstorm.menuTitle')}</span>
            <span>{selectedCount}/{MODEL_BRAINSTORM_MAX_CANDIDATES}</span>
          </div>
          <div className="model-brainstorm-control__mode-row">
            <span className="model-brainstorm-control__mode-label">
              {t('modelBrainstorm.contextModeLabel')}
            </span>
            <div className="model-brainstorm-control__mode-switch" role="group">
              {(['independent', 'shared'] as const).map(mode => (
                <button
                  key={mode}
                  type="button"
                  className={[
                    'model-brainstorm-control__mode-option',
                    contextMode === mode ? 'model-brainstorm-control__mode-option--active' : '',
                  ].filter(Boolean).join(' ')}
                  aria-pressed={contextMode === mode}
                  onClick={() => onContextModeChange(mode)}
                >
                  {contextModeLabels[mode]}
                </button>
              ))}
            </div>
          </div>
          <div className="model-brainstorm-control__list">
            {models.map(model => {
              const selected = selectedAvailableModelIds.includes(model.id);
              const canSelect = selected || selectedAvailableModelIds.length < MODEL_BRAINSTORM_MAX_CANDIDATES;
              return (
                <button
                  key={model.id}
                  type="button"
                  className={[
                    'model-brainstorm-control__option',
                    selected ? 'model-brainstorm-control__option--selected' : '',
                  ].filter(Boolean).join(' ')}
                  disabled={!canSelect}
                  data-testid="model-brainstorm-option"
                  data-model-id={model.id}
                  data-selected={selected ? 'true' : 'false'}
                  onClick={() => toggleModel(model.id)}
                >
                  <span className="model-brainstorm-control__option-main">
                    <span className="model-brainstorm-control__option-name">{model.label}</span>
                    <span className="model-brainstorm-control__option-provider">{model.providerName}</span>
                  </span>
                  {selected && <Check size={14} className="model-brainstorm-control__option-check" />}
                </button>
              );
            })}
          </div>
        </div>,
        document.body,
      )}
    </div>
  );
};
