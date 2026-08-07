import React, { useMemo, useState } from 'react';
import { AlertTriangle } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/component-library';
import type { ReasoningCatalogProjection, ReasoningConfig } from '../types';
import type { ProviderCatalog } from '@/infrastructure/api/service-api/AIApi';
import {
  cloneReasoningConfig,
  validateReasoningConfig,
} from '../utils/reasoningPresets';
import ReasoningPresetEditor from './ReasoningPresetEditor';
import './ReasoningConfigPanel.scss';

interface ReasoningConfigPanelProps {
  value: ReasoningConfig;
  generatedProjection?: ReasoningCatalogProjection | null;
  providerCatalog?: ProviderCatalog | null;
  onCancel: () => void;
  onApply: (value: ReasoningConfig) => void;
}

export const ReasoningConfigPanel: React.FC<ReasoningConfigPanelProps> = ({
  value,
  generatedProjection,
  providerCatalog,
  onCancel,
  onApply,
}) => {
  const { t } = useTranslation('settings/ai-model');
  const [draft, setDraft] = useState(() => cloneReasoningConfig(value));
  const [editorInvalid, setEditorInvalid] = useState(false);
  const generatedPresetIds = useMemo(() => (
    generatedProjection?.presets
      ?.filter(preset => preset.source !== 'model_config')
      .map(preset => preset.id) ?? []
  ), [generatedProjection?.presets]);
  const catalogBindingUnchanged = JSON.stringify(draft.catalog) === JSON.stringify(value.catalog);
  const activeGeneratedProjection = catalogBindingUnchanged ? generatedProjection : undefined;
  const validationError = validateReasoningConfig(
    draft,
    catalogBindingUnchanged ? generatedPresetIds : [],
  );
  const invalid = editorInvalid || validationError !== null;

  return (
    <div
      className="bitfun-reasoning-config-panel"
      data-bf-component="reasoning-config-panel"
      data-bf-part="root"
    >
      <div
        className="bitfun-reasoning-config-panel__body"
        data-bf-component="reasoning-config-panel"
        data-bf-part="body"
      >
        <ReasoningPresetEditor
          value={draft}
          generatedProjection={activeGeneratedProjection}
          providerCatalog={providerCatalog}
          onChange={setDraft}
          onValidationChange={setEditorInvalid}
        />
      </div>
      <div
        className="bitfun-reasoning-config-panel__footer"
        data-bf-component="reasoning-config-panel"
        data-bf-part="footer"
      >
        {invalid && (
          <div
            className="bitfun-reasoning-config-panel__error"
            data-bf-component="reasoning-config-panel"
            data-bf-part="error"
            role="alert"
          >
            <AlertTriangle size={14} aria-hidden="true" />
            <span>{t('reasoningPresets.validationError')}</span>
          </div>
        )}
        <div
          className="bitfun-reasoning-config-panel__actions"
          data-bf-component="reasoning-config-panel"
          data-bf-part="actions"
        >
          <Button variant="secondary" onClick={onCancel}>
            {t('actions.cancel')}
          </Button>
          <Button variant="primary" disabled={invalid} onClick={() => onApply(draft)}>
            {t('reasoningPresets.apply')}
          </Button>
        </div>
      </div>
    </div>
  );
};

export default ReasoningConfigPanel;
