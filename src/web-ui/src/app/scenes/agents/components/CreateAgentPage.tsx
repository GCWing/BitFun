import React, { useState, useEffect, useCallback } from 'react';
import { ArrowLeft } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Input, Textarea, Switch, Button } from '@/component-library';
import { SubagentAPI } from '@/infrastructure/api/service-api/SubagentAPI';
import type { SubagentLevel } from '@/infrastructure/api/service-api/SubagentAPI';
import { toolAPI } from '@/infrastructure/api/service-api/ToolAPI';
import { useNotification } from '@/shared/notification-system';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { useAgentsStore } from '../agentsStore';
import {
  filterToolsForReviewMode,
  normalizeReviewModeState,
  type SubagentEditorToolInfo,
} from './subagentEditorUtils';
import '../AgentsView.scss';
import './CreateAgentPage.scss';

const NAME_REGEX = /^[a-zA-Z][a-zA-Z0-9_-]*$/;

const CreateAgentPage: React.FC = () => {
  const { t } = useTranslation('scenes/agents');
  const { openHome, agentEditorMode, editingAgentId } = useAgentsStore();
  const notification = useNotification();
  const { hasWorkspace, workspacePath } = useCurrentWorkspace();

  const isEdit = agentEditorMode === 'edit' && Boolean(editingAgentId);

  const [level, setLevel] = useState<SubagentLevel>('user');
  const [name, setName] = useState('');
  const [nameError, setNameError] = useState<string | null>(null);
  const [description, setDescription] = useState('');
  const [prompt, setPrompt] = useState('');
  const [readonly, setReadonly] = useState(true);
  const [review, setReview] = useState(false);
  const [toolInfos, setToolInfos] = useState<SubagentEditorToolInfo[]>([]);
  const [selectedTools, setSelectedTools] = useState<Set<string>>(new Set());
  const [submitting, setSubmitting] = useState(false);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState<string | null>(null);

  useEffect(() => {
    toolAPI.getAllToolsInfo()
      .then((tools) => {
        const normalizedTools = tools
          .map((tool): SubagentEditorToolInfo | null => {
            const name = typeof tool?.name === 'string' ? tool.name : '';
            if (!name) return null;
            return {
              name,
              isReadonly: Boolean(tool?.isReadonly ?? tool?.is_readonly),
            };
          })
          .filter((tool): tool is SubagentEditorToolInfo => Boolean(tool));
        setToolInfos(normalizedTools);
      })
      .catch(() => setToolInfos([]));
  }, []);

  useEffect(() => {
    if (!hasWorkspace && level === 'project') {
      setLevel('user');
    }
  }, [hasWorkspace, level]);

  useEffect(() => {
    if (!isEdit || !editingAgentId) {
      setDetailLoading(false);
      setDetailError(null);
      return;
    }

    let cancelled = false;
    setDetailLoading(true);
    setDetailError(null);

    (async () => {
      try {
        const d = await SubagentAPI.getSubagentDetail({
          subagentId: editingAgentId,
          workspacePath: workspacePath || undefined,
        });
        if (cancelled) return;
        setName(d.name);
        setDescription(d.description);
        setPrompt(d.prompt);
        setReadonly(d.readonly);
        setReview(d.review);
        setLevel(d.level);
        setSelectedTools(new Set(d.tools ?? []));
        setNameError(null);
      } catch (e) {
        if (cancelled) return;
        setDetailError(e instanceof Error ? e.message : String(e));
      } finally {
        if (!cancelled) setDetailLoading(false);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [isEdit, editingAgentId, workspacePath]);

  const validateName = useCallback((v: string) => {
    if (!v.trim()) return t('agentsOverview.form.nameRequired');
    if (!NAME_REGEX.test(v.trim())) return t('agentsOverview.form.nameFormat');
    return null;
  }, [t]);

  const toggleTool = (tool: string) => {
    setSelectedTools((prev) => {
      const next = new Set(prev);
      if (next.has(tool)) {
        next.delete(tool);
      } else {
        next.add(tool);
      }
      return next;
    });
  };

  const handleReviewChange = (nextReview: boolean) => {
    setReview(nextReview);
    const next = normalizeReviewModeState({
      review: nextReview,
      readonly,
      selectedTools,
      availableTools: toolInfos,
    });
    setReadonly(next.readonly);
    setSelectedTools(next.selectedTools);
  };

  const handleReadonlyChange = (nextReadonly: boolean) => {
    if (review) {
      setReadonly(true);
      return;
    }
    setReadonly(nextReadonly);
  };

  const handleSubmit = async () => {
    if (!isEdit) {
      const err = validateName(name);
      if (err) { setNameError(err); return; }
    }
    if (!description.trim()) { notification.error(t('agentsOverview.form.descRequired')); return; }
    if (!prompt.trim()) { notification.error(t('agentsOverview.form.promptRequired')); return; }
    if (level === 'project' && !workspacePath) {
      notification.error(t('agentsOverview.form.noWorkspace'));
      return;
    }
    if (isEdit && !editingAgentId) {
      return;
    }

    setSubmitting(true);
    try {
      if (isEdit && editingAgentId) {
        await SubagentAPI.updateSubagent({
          subagentId: editingAgentId,
          description: description.trim(),
          prompt: prompt.trim(),
          readonly,
          review,
          tools: selectedTools.size > 0 ? Array.from(selectedTools) : undefined,
          workspacePath: level === 'project' ? workspacePath : undefined,
        });
        notification.success(t('agentsOverview.form.updateSuccess', { name: name.trim() }));
      } else {
        await SubagentAPI.createSubagent({
          level,
          name: name.trim(),
          description: description.trim(),
          prompt: prompt.trim(),
          readonly,
          review,
          tools: selectedTools.size > 0 ? Array.from(selectedTools) : undefined,
          workspacePath: level === 'project' ? workspacePath : undefined,
        });
        notification.success(t('agentsOverview.form.createSuccess', { name: name.trim() }));
      }
      openHome();
    } catch (err) {
      notification.error(
        (isEdit ? t('agentsOverview.form.updateFailed') : t('agentsOverview.form.createFailed')) +
        (err instanceof Error ? err.message : String(err))
      );
    } finally {
      setSubmitting(false);
    }
  };

  const formTitle = isEdit
    ? t('agentsOverview.form.titleEdit')
    : t('agentsOverview.form.title');
  const formSubtitle = isEdit
    ? t('agentsOverview.form.subtitleEdit')
    : t('agentsOverview.form.subtitle');
  const submitLabel = isEdit
    ? t('agentsOverview.form.save')
    : t('agentsOverview.form.submit');
  const selectableTools = filterToolsForReviewMode(toolInfos, review);

  if (isEdit && detailLoading) {
    return (
      <div className="tv">
        <div className="tv__editor-bar">
          <button className="tv__back-btn" onClick={openHome} type="button">
            <ArrowLeft size={14} />
            <span>{t('agentsOverview.backToOverview')}</span>
          </button>
        </div>
        <div className="th__list-body">
          <div className="th__list-inner">
            <p className="th__title-sub">{t('agentsOverview.form.loadingDetail')}</p>
          </div>
        </div>
      </div>
    );
  }

  if (isEdit && detailError) {
    return (
      <div className="tv">
        <div className="tv__editor-bar">
          <button className="tv__back-btn" onClick={openHome} type="button">
            <ArrowLeft size={14} />
            <span>{t('agentsOverview.backToOverview')}</span>
          </button>
        </div>
        <div className="th__list-body">
          <div className="th__list-inner">
            <p className="th-create-panel__error">{detailError}</p>
            <Button variant="secondary" size="small" onClick={openHome}>{t('agentsOverview.form.cancel')}</Button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="tv">
      <div className="tv__editor-bar">
        <button className="tv__back-btn" onClick={openHome} type="button">
          <ArrowLeft size={14} />
          <span>{t('agentsOverview.backToOverview')}</span>
        </button>
      </div>

      <div className="th__list-body">
        <div className="th__list-inner">
          <div className="th-create-page__head">
            <h2 className="th__title">{formTitle}</h2>
            <p className="th__title-sub">{formSubtitle}</p>
          </div>

          <div className="th-create-page__form">
            <div className="th-create-panel__field">
              <label className="th-create-panel__label">{t('agentsOverview.form.name')}</label>
              <Input
                value={name}
                onChange={(e) => { setName(e.target.value); setNameError(validateName(e.target.value)); }}
                onBlur={() => setNameError(validateName(name))}
                placeholder={t('agentsOverview.form.namePlaceholder')}
                inputSize="small"
                error={!!nameError}
                disabled={isEdit}
              />
              {nameError && <span className="th-create-panel__error">{nameError}</span>}
            </div>

            <div className="th-create-panel__field">
              <label className="th-create-panel__label">{t('agentsOverview.form.description')}</label>
              <Input
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder={t('agentsOverview.form.descPlaceholder')}
                inputSize="small"
              />
            </div>

            <div className="th-create-panel__field th-create-panel__field--row">
              <div className="th-create-panel__level-group">
                {(['user', 'project'] as SubagentLevel[]).map((lv) => {
                  const disabled = (lv === 'project' && !hasWorkspace) || isEdit;
                  return (
                    <button
                      key={lv}
                      type="button"
                      disabled={disabled}
                      className={`th-create-panel__level-btn${level === lv ? ' is-active' : ''}`}
                      onClick={() => setLevel(lv)}
                      title={disabled && !isEdit ? t('agentsOverview.form.noWorkspace') : undefined}
                    >
                      {lv === 'user' ? t('agentsOverview.filterUser') : t('agentsOverview.filterProject')}
                    </button>
                  );
                })}
              </div>
              <div className="th-create-panel__readonly-row">
                <label className="th-create-panel__label">{t('agentsOverview.form.readonly')}</label>
                <Switch
                  checked={readonly}
                  disabled={review}
                  onChange={(e) => handleReadonlyChange(e.target.checked)}
                  size="small"
                />
              </div>
              <div className="th-create-panel__readonly-row">
                <label className="th-create-panel__label">
                  {t('agentsOverview.form.review', {
                    defaultValue: 'Review',
                  })}
                </label>
                <Switch checked={review} onChange={(e) => handleReviewChange(e.target.checked)} size="small" />
              </div>
            </div>

            {selectableTools.length > 0 && (
              <div className="th-create-panel__field">
                <label className="th-create-panel__label">
                  {t('agentsOverview.form.tools')}
                  <span className="th-create-panel__label-hint">
                    {review
                      ? t('agentsOverview.form.reviewToolsHint', {
                        defaultValue: 'Review Sub-Agents can only use read-only tools.',
                      })
                      : t('agentsOverview.form.toolsHint', {
                        optionalLabel: t('agentsOverview.form.toolsOptional'),
                      })}
                  </span>
                </label>
                <div className="th-create-panel__tools">
                  {selectableTools.map((tool) => (
                    <button
                      key={tool.name}
                      type="button"
                      className={`th-list__tool-item${selectedTools.has(tool.name) ? ' is-on' : ''}`}
                      onClick={() => toggleTool(tool.name)}
                    >
                      <span className="th-list__tool-item-name">{tool.name}</span>
                    </button>
                  ))}
                </div>
              </div>
            )}

            <div className="th-create-panel__field">
              <label className="th-create-panel__label">{t('agentsOverview.form.prompt')}</label>
              <Textarea
                value={prompt}
                onChange={(e) => setPrompt(e.target.value)}
                placeholder={t('agentsOverview.form.promptPlaceholder')}
                rows={8}
              />
            </div>

            <div className="th-create-page__actions">
              <Button variant="secondary" size="small" onClick={openHome} disabled={submitting}>
                {t('agentsOverview.form.cancel')}
              </Button>
              <Button variant="primary" size="small" onClick={handleSubmit} disabled={submitting}>
                {submitting ? '…' : submitLabel}
              </Button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default CreateAgentPage;
