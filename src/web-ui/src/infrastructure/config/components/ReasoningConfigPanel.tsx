import { Button, ScrollArea } from '@openbitfun/ui';
import React, { useEffect, useMemo, useRef, useState } from 'react';
import { AlertTriangle } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { ReasoningCatalogBinding, ReasoningCatalogProjection, ReasoningConfig } from '../types';
import type { ModelsDevReasoningCatalog } from '@/infrastructure/api/service-api/AIApi';
import { aiApi } from '@/infrastructure/api';
import type { ReasoningCatalogProjectionRequest } from '@/infrastructure/api/service-api/AIApi';
import {
  cloneReasoningConfig,
  validateReasoningConfig,
} from '../utils/reasoningPresets';
import ReasoningPresetEditor from './ReasoningPresetEditor';
import './ReasoningConfigPanel.scss';

interface ReasoningConfigPanelProps {
  value: ReasoningConfig;
  generatedProjection?: ReasoningCatalogProjection | null;
  modelsDevReasoningCatalog?: ModelsDevReasoningCatalog | null;
  projectionRequest?: Omit<ReasoningCatalogProjectionRequest, 'reasoning'>;
  requestFormatLabel?: string;
  onCancel: () => void;
  onApply: (value: ReasoningConfigApplyResult) => void;
  onDraftChange?: (value: ReasoningConfigApplyResult) => void;
}

export interface ReasoningConfigApplyResult {
  reasoning: ReasoningConfig;
  projectionCatalog: ReasoningCatalogBinding;
  projection?: ReasoningCatalogProjection | null;
}

export const ReasoningConfigPanel: React.FC<ReasoningConfigPanelProps> = ({
  value,
  generatedProjection,
  modelsDevReasoningCatalog,
  projectionRequest,
  requestFormatLabel,
  onCancel,
  onApply,
  onDraftChange,
}) => {
  const { t } = useTranslation('settings/models');
  const [draft, setDraft] = useState(() => cloneReasoningConfig(value));
  const [editorInvalid, setEditorInvalid] = useState(false);
  const projectionRequestId = useRef(0);
  const onDraftChangeRef = useRef(onDraftChange);
  onDraftChangeRef.current = onDraftChange;
  const projectionRequestRef = useRef(projectionRequest);
  projectionRequestRef.current = projectionRequest;
  const projectionRequestKey = JSON.stringify(projectionRequest);
  const fallbackGeneratedProjection = projectionRequest ? undefined : generatedProjection;
  const projectionBindingKey = JSON.stringify({
    projectionRequest,
    catalog: draft.catalog,
  });
  const [resolvedProjection, setResolvedProjection] = useState<{
    bindingKey: string;
    projection?: ReasoningCatalogProjection | null;
  }>(() => ({
    bindingKey: projectionBindingKey,
    projection: generatedProjection,
  }));
  const activeGeneratedProjection = resolvedProjection.bindingKey === projectionBindingKey
    ? resolvedProjection.projection
    : undefined;

  useEffect(() => {
    onDraftChangeRef.current?.({
      reasoning: draft,
      projectionCatalog: draft.catalog ?? { source: 'auto' },
      projection: activeGeneratedProjection,
    });
  }, [activeGeneratedProjection, draft]);

  useEffect(() => {
    const currentProjectionRequest = projectionRequestRef.current;
    if (!currentProjectionRequest) {
      setResolvedProjection({
        bindingKey: projectionBindingKey,
        projection: fallbackGeneratedProjection,
      });
      return;
    }

    const requestId = ++projectionRequestId.current;
    void aiApi.projectReasoningCatalog({
      ...currentProjectionRequest,
      reasoning: draft,
    }).then((projection) => {
      if (projectionRequestId.current !== requestId) return;
      setResolvedProjection({ bindingKey: projectionBindingKey, projection });
    }).catch(() => {
      if (projectionRequestId.current !== requestId) return;
      setResolvedProjection({ bindingKey: projectionBindingKey, projection: undefined });
    });

    return () => {
      if (projectionRequestId.current === requestId) {
        projectionRequestId.current += 1;
      }
    };
  }, [draft, fallbackGeneratedProjection, projectionBindingKey, projectionRequestKey]);

  const generatedPresetIds = useMemo(() => (
    activeGeneratedProjection?.presets
      ?.filter(preset => preset.source !== 'model_config')
      .map(preset => preset.id) ?? []
  ), [activeGeneratedProjection?.presets]);
  const validationError = validateReasoningConfig(
    draft,
    generatedPresetIds,
  );
  const invalid = editorInvalid || validationError !== null;

  return (
    <div
      className="openbitfun-reasoning-config-panel"
      data-openbitfun-component="reasoning-config-panel"
      data-openbitfun-part="root"
    >
      <ScrollArea
        className="openbitfun-reasoning-config-panel__body"
        data-openbitfun-component="reasoning-config-panel"
        data-openbitfun-part="body"
      >
        <ReasoningPresetEditor
          value={draft}
          generatedProjection={activeGeneratedProjection}
          modelsDevReasoningCatalog={modelsDevReasoningCatalog}
          requestFormatLabel={requestFormatLabel}
          onChange={setDraft}
          onValidationChange={setEditorInvalid}
        />
      </ScrollArea>
      <div
        className="openbitfun-reasoning-config-panel__footer"
        data-openbitfun-component="reasoning-config-panel"
        data-openbitfun-part="footer"
      >
        {invalid && (
          <div
            className="openbitfun-reasoning-config-panel__error"
            data-openbitfun-component="reasoning-config-panel"
            data-openbitfun-part="error"
            role="alert"
          >
            <AlertTriangle size={14} aria-hidden="true" />
            <span>{t('reasoningPresets.validationError')}</span>
          </div>
        )}
        <div
          className="openbitfun-reasoning-config-panel__actions"
          data-openbitfun-component="reasoning-config-panel"
          data-openbitfun-part="actions"
        >
          <Button variant="outline" onClick={onCancel}>
            {t('actions.cancel')}
          </Button>
          <Button
            variant="fill"
            disabled={invalid}
            onClick={() => onApply({
              reasoning: draft,
              projectionCatalog: draft.catalog ?? { source: 'auto' },
              projection: activeGeneratedProjection,
            })}
          >
            {t('reasoningPresets.apply')}
          </Button>
        </div>
      </div>
    </div>
  );
};

export default ReasoningConfigPanel;
