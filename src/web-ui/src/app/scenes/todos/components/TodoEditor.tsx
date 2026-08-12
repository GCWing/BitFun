/**
 * Create / edit form for a Todo.
 *
 * Unlike the per-workspace scheduled-jobs editor, the Todos scene is global, so
 * this form owns a workspace picker: a job cannot be scheduled without knowing
 * which workspace it runs in.
 */

import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { Button, Input, Select, Switch, Textarea } from '@/component-library';
import { agentAPI, type ModeInfo } from '@/infrastructure/api/service-api/AgentAPI';
import { useI18n } from '@/infrastructure/i18n';
import { WorkspaceKind } from '@/shared/types';
import { createLogger } from '@/shared/utils/logger';
import {
  ASSISTANT_WORKSPACE_AGENT_TYPE,
  DEFAULT_AGENT_TYPE,
  getCurrentLocalDateTimeInput,
  isFutureLocalDateTimeInput,
  type JobDraft,
  type JobDraftValidationErrors,
  type ScheduleKind,
} from '@/app/components/scheduled-jobs/scheduledJobDraft';
import type { TodoWorkspaceOption } from '../todoPresentation';

const log = createLogger('TodoEditor');

export interface TodoEditorProps {
  draft: JobDraft;
  onDraftChange: (updater: (draft: JobDraft) => JobDraft) => void;
  validationErrors: JobDraftValidationErrors;
  onValidationErrorsChange: (
    updater: (errors: JobDraftValidationErrors) => JobDraftValidationErrors,
  ) => void;
  workspaceOptions: TodoWorkspaceOption[];
  selectedWorkspaceId: string;
  onSelectedWorkspaceIdChange: (workspaceId: string) => void;
  /** True when editing an existing Todo rather than creating one. */
  isEditing: boolean;
  /**
   * Set when the Todo runs inside a specific existing session instead of
   * launching a new one. That binding is preserved on save, so the agent-type
   * picker is replaced by a note rather than implying a fresh session.
   */
  boundSessionId?: string | null;
  saving: boolean;
  onSave: () => void;
  onCancel: () => void;
}

const TodoEditor: React.FC<TodoEditorProps> = ({
  draft,
  onDraftChange,
  validationErrors,
  onValidationErrorsChange,
  workspaceOptions,
  selectedWorkspaceId,
  onSelectedWorkspaceIdChange,
  isEditing,
  boundSessionId = null,
  saving,
  onSave,
  onCancel,
}) => {
  const { t } = useI18n(['scenes/todos', 'common', 'shared']);
  const [availableModes, setAvailableModes] = useState<ModeInfo[]>([]);

  const selectedWorkspace = useMemo(
    () => workspaceOptions.find((option) => option.value === selectedWorkspaceId)?.workspace ?? null,
    [selectedWorkspaceId, workspaceOptions],
  );
  const isAssistantWorkspace = selectedWorkspace?.workspaceKind === WorkspaceKind.Assistant;

  useEffect(() => {
    let cancelled = false;

    const loadAvailableModes = async () => {
      try {
        const modes = await agentAPI.getAvailableModes();
        if (!cancelled) setAvailableModes(modes);
      } catch (error) {
        log.error('Failed to load available modes for the Todo editor', { error });
      }
    };

    void loadAvailableModes();
    return () => { cancelled = true; };
  }, []);

  // An assistant workspace only ever runs its own agent type; keep the draft in
  // step with the picked workspace so a saved Todo cannot carry a stale one.
  useEffect(() => {
    if (!selectedWorkspace) return;
    onDraftChange((current) => {
      const nextAgentType = isAssistantWorkspace
        ? ASSISTANT_WORKSPACE_AGENT_TYPE
        : current.agentType === ASSISTANT_WORKSPACE_AGENT_TYPE || !current.agentType
          ? DEFAULT_AGENT_TYPE
          : current.agentType;
      return current.agentType === nextAgentType
        ? current
        : { ...current, agentType: nextAgentType };
    });
  }, [isAssistantWorkspace, onDraftChange, selectedWorkspace]);

  const agentTypeOptions = useMemo(() => {
    if (isAssistantWorkspace) {
      return [{ value: ASSISTANT_WORKSPACE_AGENT_TYPE, label: ASSISTANT_WORKSPACE_AGENT_TYPE }];
    }

    const options = availableModes
      .filter((mode) => mode.id !== ASSISTANT_WORKSPACE_AGENT_TYPE)
      .map((mode) => ({ value: mode.id, label: mode.name?.trim() || mode.id }));

    if (draft.agentType && !options.some((option) => option.value === draft.agentType)) {
      options.push({ value: draft.agentType, label: draft.agentType });
    }
    return options;
  }, [availableModes, draft.agentType, isAssistantWorkspace]);

  const workspaceSelectOptions = useMemo(
    () => workspaceOptions.map((option) => ({
      value: option.value,
      label: option.label,
      description: option.description,
    })),
    [workspaceOptions],
  );

  const updateDraft = useCallback(
    (patch: Partial<JobDraft>) => onDraftChange((current) => ({ ...current, ...patch })),
    [onDraftChange],
  );

  const clearError = useCallback(
    (field: keyof JobDraftValidationErrors) =>
      onValidationErrorsChange((current) => ({ ...current, [field]: false })),
    [onValidationErrorsChange],
  );

  return (
    <section
      className="bf-todos__editor"
      aria-label={isEditing ? t('editor.editTitle') : t('editor.createTitle')}
      data-bf-scene="todos"
      data-bf-part="editor"
      data-testid="todos-editor"
    >
      <header className="bf-todos__editor-head" data-bf-scene="todos" data-bf-part="editorHead">
        <h3 className="bf-todos__editor-title">
          {isEditing ? t('editor.editTitle') : t('editor.createTitle')}
        </h3>
      </header>

      <div className="bf-todos__editor-grid" data-bf-scene="todos" data-bf-part="editorGrid">
        <label className="bf-todos__field" data-bf-scene="todos" data-bf-part="field">
          <span className="bf-todos__field-label">{t('editor.fields.name')}</span>
          <Input
            size="small"
            value={draft.name}
            error={validationErrors.name}
            placeholder={t('editor.placeholders.name')}
            onChange={(event) => {
              const name = event.currentTarget.value;
              clearError('name');
              updateDraft({ name });
            }}
            data-testid="todos-editor-name"
          />
        </label>

        <label className="bf-todos__field" data-bf-scene="todos" data-bf-part="field">
          <span className="bf-todos__field-label">{t('shared:features.workspace')}</span>
          <Select
            size="small"
            options={workspaceSelectOptions}
            value={selectedWorkspaceId}
            searchable
            placeholder={t('editor.placeholders.workspace')}
            onChange={(value) => onSelectedWorkspaceIdChange(String(value))}
            renderOption={(option) => (
              <div className="bf-todos__workspace-option">
                <span className="bf-todos__workspace-option-label">{option.label}</span>
                {option.description ? (
                  <span className="bf-todos__workspace-option-path">{option.description}</span>
                ) : null}
              </div>
            )}
          />
        </label>

        {boundSessionId ? (
          <div className="bf-todos__field" data-bf-scene="todos" data-bf-part="field">
            <span className="bf-todos__field-label">{t('editor.fields.runsIn')}</span>
            <span className="bf-todos__field-static" title={boundSessionId}>
              {t('target.existingSession')}
            </span>
            <span className="bf-todos__field-note">{t('editor.hints.boundSession')}</span>
          </div>
        ) : (
          <label className="bf-todos__field" data-bf-scene="todos" data-bf-part="field">
            <span className="bf-todos__field-label">{t('editor.fields.agentType')}</span>
            <Select
              size="small"
              options={agentTypeOptions}
              value={draft.agentType}
              error={validationErrors.agentType}
              disabled={isAssistantWorkspace}
              placeholder={t('editor.placeholders.agentType')}
              onChange={(value) => {
                clearError('agentType');
                updateDraft({ agentType: String(value) });
              }}
            />
          </label>
        )}

        <label className="bf-todos__field" data-bf-scene="todos" data-bf-part="field">
          <span className="bf-todos__field-label">{t('editor.fields.scheduleKind')}</span>
          <Select
            size="small"
            value={draft.scheduleKind}
            options={[
              { value: 'at', label: t('schedule.kinds.at') },
              { value: 'every', label: t('schedule.kinds.every') },
              { value: 'cron', label: t('schedule.kinds.cron') },
            ]}
            onChange={(value) => {
              const scheduleKind = value as ScheduleKind;
              onValidationErrorsChange((current) => ({
                ...current,
                at: false,
                everyMinutes: false,
                cronExpr: false,
              }));
              onDraftChange((current) => ({
                ...current,
                scheduleKind,
                at: scheduleKind === 'at' && !current.at.trim()
                  ? getCurrentLocalDateTimeInput()
                  : current.at,
              }));
            }}
            data-testid="todos-editor-schedule-kind"
          />
        </label>

        {draft.scheduleKind === 'at' ? (
          <label className="bf-todos__field" data-bf-scene="todos" data-bf-part="field">
            <span className="bf-todos__field-label">{t('editor.fields.at')}</span>
            <Input
              size="small"
              type="datetime-local"
              value={draft.at}
              error={validationErrors.at}
              onChange={(event) => {
                const at = event.currentTarget.value;
                clearError('at');
                // Re-enable a Todo that was left off after its time passed.
                onDraftChange((current) => ({
                  ...current,
                  at,
                  enabled: !current.enabled && isFutureLocalDateTimeInput(at) ? true : current.enabled,
                }));
              }}
            />
          </label>
        ) : null}

        {draft.scheduleKind === 'every' ? (
          <>
            <label className="bf-todos__field" data-bf-scene="todos" data-bf-part="field">
              <span className="bf-todos__field-label">{t('editor.fields.everyMinutes')}</span>
              <Input
                size="small"
                type="number"
                value={draft.everyMinutes}
                error={validationErrors.everyMinutes}
                placeholder="60"
                onChange={(event) => {
                  const everyMinutes = event.currentTarget.value;
                  clearError('everyMinutes');
                  updateDraft({ everyMinutes });
                }}
              />
            </label>
            <label className="bf-todos__field" data-bf-scene="todos" data-bf-part="field">
              <span className="bf-todos__field-label">{t('editor.fields.anchor')}</span>
              <Input
                size="small"
                type="datetime-local"
                value={draft.anchorMs}
                placeholder={t('editor.placeholders.anchor')}
                onChange={(event) => updateDraft({ anchorMs: event.currentTarget.value })}
              />
            </label>
          </>
        ) : null}

        {draft.scheduleKind === 'cron' ? (
          <>
            <label className="bf-todos__field" data-bf-scene="todos" data-bf-part="field">
              <span className="bf-todos__field-label">{t('editor.fields.cronExpr')}</span>
              <Input
                size="small"
                value={draft.expr}
                error={validationErrors.cronExpr}
                placeholder="0 8 * * *"
                onChange={(event) => {
                  const expr = event.currentTarget.value;
                  clearError('cronExpr');
                  updateDraft({ expr });
                }}
              />
              <span className="bf-todos__field-note">{t('editor.hints.cronExpr')}</span>
            </label>
            <label className="bf-todos__field" data-bf-scene="todos" data-bf-part="field">
              <span className="bf-todos__field-label">{t('editor.fields.timezone')}</span>
              <Input
                size="small"
                value={draft.tz}
                placeholder={t('editor.placeholders.timezone')}
                onChange={(event) => updateDraft({ tz: event.currentTarget.value })}
              />
            </label>
          </>
        ) : null}

        <div
          className="bf-todos__field bf-todos__field--toggle"
          data-bf-scene="todos"
          data-bf-part="field"
        >
          <span className="bf-todos__field-label">{t('editor.fields.enabled')}</span>
          <Switch
            size="small"
            checked={draft.enabled}
            aria-label={t('editor.fields.enabled')}
            onChange={(event) => updateDraft({ enabled: event.currentTarget.checked })}
          />
        </div>
      </div>

      <label
        className="bf-todos__field bf-todos__field--prompt"
        data-bf-scene="todos"
        data-bf-part="field"
      >
        <span className="bf-todos__field-label">{t('editor.fields.prompt')}</span>
        <Textarea
          value={draft.text}
          error={validationErrors.text}
          autoResize
          showCount
          maxLength={4000}
          placeholder={t('editor.placeholders.prompt')}
          onChange={(event) => {
            const text = event.currentTarget.value;
            clearError('text');
            updateDraft({ text });
          }}
          data-testid="todos-editor-prompt"
        />
      </label>

      {workspaceOptions.length === 0 ? (
        <p className="bf-todos__warning" data-bf-scene="todos" data-bf-part="warning">
          {t('editor.noWorkspace')}
        </p>
      ) : null}

      <div className="bf-todos__editor-actions" data-bf-scene="todos" data-bf-part="editorActions">
        <Button size="small" variant="ghost" onClick={onCancel}>
          {t('common:nav.scheduledJobs.actions.cancel')}
        </Button>
        <Button
          size="small"
          variant="primary"
          onClick={onSave}
          isLoading={saving}
          disabled={workspaceOptions.length === 0 || !selectedWorkspaceId}
          data-testid="todos-editor-save"
        >
          {isEditing ? t('editor.actions.save') : t('editor.actions.create')}
        </Button>
      </div>
    </section>
  );
};

export default TodoEditor;
