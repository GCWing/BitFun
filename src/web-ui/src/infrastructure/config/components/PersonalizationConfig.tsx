import React from 'react';
import { FolderOpen, RefreshCw, ChevronDown } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import {
  Button,
  Card,
  CardBody,
  ConfigPageLoading,
  IconButton,
  Input,
  Modal,
  Switch,
  Textarea,
  NumberInput,
} from '@/component-library';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent, ConfigPageSection, ConfigPageRow } from './common';
import { ModelSelectionRadio } from './ModelSelectionRadio';
import { LANGUAGE_TEMPLATE_LABELS } from '../types';
import { AGENT_SESSION_TITLE, useSessionSettingsConfig } from './useSessionSettingsConfig';
import './AIFeaturesConfig.scss';
import './DebugConfig.scss';

const PersonalizationConfig: React.FC = () => {
  const { t } = useTranslation('settings/personalization');
  const {
    isLoading,
    settings,
    enabledModels,
    sessionTitleModelId,
    debugConfig,
    debugHasChanges,
    debugSaving,
    expandedTemplates,
    isTemplatesModalOpen,
    templateEntries,
    updateSetting,
    handleAgentModelChange,
    updateDebugConfig,
    saveDebugConfig,
    cancelDebugChanges,
    handleModalSave,
    handleModalCancel,
    resetDebugTemplates,
    updateTemplate,
    toggleTemplateEnabled,
    toggleTemplateExpand,
    handleSelectLogPath,
    setIsTemplatesModalOpen,
    tDebug,
  } = useSessionSettingsConfig();

  if (isLoading || !settings) {
    return (
      <ConfigPageLayout className="bitfun-func-agent-config">
        <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
        <ConfigPageContent className="bitfun-func-agent-config__content">
          <ConfigPageLoading text={t('loading.text')} />
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  return (
    <ConfigPageLayout className="bitfun-func-agent-config">
      <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
      <ConfigPageContent className="bitfun-func-agent-config__content">
        <ConfigPageSection
          title={t('features.sessionTitle.title')}
          description={t('features.sessionTitle.subtitle')}
        >
          <ConfigPageRow label={t('common.enable')} align="center">
            <div className="bitfun-func-agent-config__row-control">
              <Switch
                checked={settings.enable_session_title_generation}
                onChange={(e) => updateSetting('enable_session_title_generation', e.target.checked)}
                size="small"
              />
            </div>
          </ConfigPageRow>
          <ConfigPageRow
            className="bitfun-func-agent-config__model-row"
            label={t('model.label')}
            description={enabledModels.length === 0 ? t('models.empty') : undefined}
            align="center"
          >
            <div className="bitfun-func-agent-config__row-control bitfun-func-agent-config__row-control--model">
              <ModelSelectionRadio
                value={sessionTitleModelId}
                models={enabledModels}
                onChange={(modelId) =>
                  handleAgentModelChange(AGENT_SESSION_TITLE, 'features.sessionTitle.title', modelId)
                }
                layout="horizontal"
                size="small"
              />
            </div>
          </ConfigPageRow>
        </ConfigPageSection>

        <ConfigPageSection
          title={t('features.agentCompanion.title')}
          description={t('features.agentCompanion.subtitle')}
        >
          <ConfigPageRow label={t('features.agentCompanion.enable')} align="center">
            <div className="bitfun-func-agent-config__row-control">
              <Switch
                checked={settings.enable_agent_companion}
                onChange={(e) => updateSetting('enable_agent_companion', e.target.checked)}
                size="small"
              />
            </div>
          </ConfigPageRow>
        </ConfigPageSection>

        <ConfigPageSection
          title={tDebug('sections.combined')}
          description={tDebug('sections.combinedDescription')}
        >
          <ConfigPageRow
            label={tDebug('settings.logPath.label')}
            description={tDebug('settings.logPath.description')}
          >
            <div className="bitfun-debug-config__input-group">
              <Input
                value={debugConfig.log_path}
                onChange={(e) => updateDebugConfig({ log_path: e.target.value })}
                placeholder={tDebug('settings.logPath.placeholder')}
                variant="outlined"
                inputSize="small"
              />
              <IconButton
                variant="default"
                size="small"
                onClick={handleSelectLogPath}
                tooltip={tDebug('settings.logPath.browse')}
              >
                <FolderOpen size={16} />
              </IconButton>
            </div>
          </ConfigPageRow>

          <ConfigPageRow
            label={tDebug('settings.ingestPort.label')}
            description={tDebug('settings.ingestPort.description')}
            align="center"
          >
            <NumberInput
              value={debugConfig.ingest_port}
              onChange={(v) => updateDebugConfig({ ingest_port: v })}
              min={1024}
              max={65535}
              step={1}
              size="small"
            />
          </ConfigPageRow>

          {debugHasChanges && !isTemplatesModalOpen && (
            <ConfigPageRow label={tDebug('actions.save')} align="center">
              <div className="bitfun-debug-config__settings-actions">
                <Button
                  variant="primary"
                  size="small"
                  onClick={saveDebugConfig}
                  disabled={debugSaving}
                >
                  {debugSaving ? tDebug('actions.saving') : tDebug('actions.save')}
                </Button>
                <Button
                  variant="secondary"
                  size="small"
                  onClick={cancelDebugChanges}
                  disabled={debugSaving}
                >
                  {tDebug('actions.cancel')}
                </Button>
              </div>
            </ConfigPageRow>
          )}

          <ConfigPageRow
            label={tDebug('sections.templates')}
            description={tDebug('templates.description')}
            align="center"
          >
            <Button
              variant="secondary"
              size="small"
              onClick={() => setIsTemplatesModalOpen(true)}
            >
              {tDebug('templates.configure')}
            </Button>
          </ConfigPageRow>
        </ConfigPageSection>

        <Modal
          isOpen={isTemplatesModalOpen}
          onClose={() => setIsTemplatesModalOpen(false)}
          title={tDebug('sections.templates')}
          titleExtra={(
            <IconButton
              type="button"
              variant="ghost"
              size="xs"
              className="bitfun-debug-config__modal-reset-icon"
              onClick={resetDebugTemplates}
              tooltip={tDebug('templates.reset')}
              aria-label={tDebug('templates.reset')}
            >
              <RefreshCw size={12} strokeWidth={2} />
            </IconButton>
          )}
          size="large"
        >
          <div className="bitfun-debug-config__modal-body">
            {templateEntries.map(([language, template]) => {
              const isExpanded = expandedTemplates.has(language);
              return (
                <Card
                  key={language}
                  variant="default"
                  padding="none"
                  interactive
                  className={`bitfun-debug-config__template-card${isExpanded ? ' is-expanded' : ''}`}
                >
                  <div
                    className="bitfun-debug-config__template-header"
                    onClick={() => toggleTemplateExpand(language)}
                  >
                    <div className="bitfun-debug-config__template-info">
                      <div onClick={(e) => e.stopPropagation()}>
                        <Switch
                          checked={template.enabled}
                          onChange={() => toggleTemplateEnabled(language, template.enabled)}
                          size="small"
                        />
                      </div>
                      <span className="bitfun-debug-config__template-name">
                        {template.display_name || LANGUAGE_TEMPLATE_LABELS[language] || language}
                      </span>
                    </div>
                    <ChevronDown
                      size={16}
                      className={`bitfun-debug-config__template-arrow${isExpanded ? ' is-expanded' : ''}`}
                    />
                  </div>

                  {isExpanded && (
                    <CardBody className="bitfun-debug-config__template-content">
                      <div className="bitfun-debug-config__template-field">
                        <Textarea
                          label={tDebug('templates.instrumentation.label')}
                          value={template.instrumentation_template}
                          onChange={(e) => updateTemplate(language, { instrumentation_template: e.target.value })}
                          placeholder={tDebug('templates.instrumentation.placeholder')}
                          hint={`${tDebug('templates.instrumentation.placeholders')}: {LOCATION}, {MESSAGE}, {DATA}, {PORT}, {SESSION_ID}, {HYPOTHESIS_ID}, {RUN_ID}, {LOG_PATH}`}
                          variant="outlined"
                          autoResize
                        />
                      </div>
                      <div className="bitfun-debug-config__template-field">
                        <label className="bitfun-debug-config__template-label">
                          {tDebug('templates.region.label')}
                        </label>
                        <div className="bitfun-debug-config__region-inputs">
                          <Input
                            value={template.region_start}
                            onChange={(e) => updateTemplate(language, { region_start: e.target.value })}
                            placeholder={tDebug('templates.region.startPlaceholder')}
                            variant="outlined"
                            inputSize="small"
                          />
                          <Input
                            value={template.region_end}
                            onChange={(e) => updateTemplate(language, { region_end: e.target.value })}
                            placeholder={tDebug('templates.region.endPlaceholder')}
                            variant="outlined"
                            inputSize="small"
                          />
                        </div>
                      </div>
                      {template.notes && template.notes.length > 0 && (
                        <div className="bitfun-debug-config__template-field">
                          <label className="bitfun-debug-config__template-label">
                            {tDebug('templates.notes')}
                          </label>
                          <div className="bitfun-debug-config__template-notes">
                            {template.notes.map((note, idx) => (
                              <span key={idx} className="bitfun-debug-config__template-note">
                                {note}
                              </span>
                            ))}
                          </div>
                        </div>
                      )}
                    </CardBody>
                  )}
                </Card>
              );
            })}
          </div>

          {debugHasChanges && (
            <div className="bitfun-debug-config__modal-footer">
              <Button
                variant="primary"
                size="small"
                onClick={handleModalSave}
                disabled={debugSaving}
              >
                {debugSaving ? tDebug('actions.saving') : tDebug('actions.save')}
              </Button>
              <Button
                variant="secondary"
                size="small"
                onClick={handleModalCancel}
                disabled={debugSaving}
              >
                {tDebug('actions.cancel')}
              </Button>
            </div>
          )}
        </Modal>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default PersonalizationConfig;
