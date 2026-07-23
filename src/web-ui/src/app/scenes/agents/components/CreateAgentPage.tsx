import React, { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { ArrowLeft } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button, Input, Switch, Textarea, Tooltip } from '@/component-library';
import {
  CustomAgentAPI,
  type CustomAgentKind,
  type CustomAgentLevel,
  type UserContextSection,
} from '@/infrastructure/api/service-api/CustomAgentAPI';
import { toolAPI } from '@/infrastructure/api/service-api/ToolAPI';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { useNotification } from '@/shared/notification-system';
import { isUserSelectableToolName } from '@/shared/utils/toolVisibility';
import { useAgentsStore } from '../agentsStore';
import {
  filterToolsForReviewMode,
  normalizeReviewModeState,
  type SubagentEditorToolInfo,
} from './subagentEditorUtils';
import { ToolGroupPicker, ToolGroupSummary } from './ToolGroupPicker';
import { useUserToolGroups } from './useUserToolGroups';
import '../AgentsView.scss';
import './CreateAgentPage.scss';

const ID_REGEX = /^[a-zA-Z][a-zA-Z0-9_-]*$/;
const DEFAULT_MODE_POLICY: UserContextSection[] = [
  'workspace_context',
  'workspace_instructions',
  'project_layout',
];
const DEFAULT_SUBAGENT_POLICY: UserContextSection[] = [
  'workspace_context',
  'workspace_instructions',
  'project_layout',
];
const DEFAULT_CUSTOM_MODE_TOOLS = [
  'Read',
  'Glob',
  'Grep',
  'Write',
  'Edit',
  'Delete',
  'ExecCommand',
  'WriteStdin',
  'ExecControl',
  'Task',
  'Skill',
  'WebSearch',
  'WebFetch',
] as const;
const DEFAULT_CUSTOM_SUBAGENT_TOOLS = ['LS', 'Read', 'Glob', 'Grep'] as const;
const VISIBLE_CONTEXT_SECTIONS: UserContextSection[] = [
  'workspace_context',
  'workspace_instructions',
  'project_layout',
];
const TOOL_SUMMARY_MIN_HEIGHT = 96;
const TOOL_SUMMARY_MAX_HEIGHT = 360;

function defaultReadonlyForKind(kind: CustomAgentKind): boolean {
  return kind === 'subagent';
}

function defaultPolicyForKind(kind: CustomAgentKind): UserContextSection[] {
  return kind === 'mode' ? DEFAULT_MODE_POLICY : DEFAULT_SUBAGENT_POLICY;
}

function defaultSelectedTools(
  tools: SubagentEditorToolInfo[],
  kind: CustomAgentKind,
  review: boolean,
): Set<string> {
  const defaultTools =
    kind === 'mode' ? DEFAULT_CUSTOM_MODE_TOOLS : DEFAULT_CUSTOM_SUBAGENT_TOOLS;
  const selectableToolNames = new Set(
    filterToolsForReviewMode(tools, kind === 'subagent' && review).map((tool) => tool.name),
  );

  return new Set(
    defaultTools.filter((toolName) => selectableToolNames.has(toolName)),
  );
}

const CreateAgentPage: React.FC = () => {
  const { t } = useTranslation('scenes/agents');
  const notification = useNotification();
  const { hasWorkspace, workspacePath } = useCurrentWorkspace();
  const { openHome, agentEditorMode, editingAgentId } = useAgentsStore();
  const {
    groups: userToolGroups,
    saveGroups: saveUserToolGroups,
  } = useUserToolGroups();

  const isEdit = agentEditorMode === 'edit' && Boolean(editingAgentId);

  const [kind, setKind] = useState<CustomAgentKind>('mode');
  const [level, setLevel] = useState<CustomAgentLevel>('user');
  const [agentId, setAgentId] = useState('');
  const [agentIdError, setAgentIdError] = useState<string | null>(null);
  const [name, setName] = useState('');
  const [nameError, setNameError] = useState<string | null>(null);
  const [description, setDescription] = useState('');
  const [prompt, setPrompt] = useState('');
  const [readonly, setReadonly] = useState(defaultReadonlyForKind('mode'));
  const [review, setReview] = useState(false);
  const [toolInfos, setToolInfos] = useState<SubagentEditorToolInfo[]>([]);
  const [selectedTools, setSelectedTools] = useState<Set<string>>(new Set());
  const [toolsEditing, setToolsEditing] = useState(false);
  const [pendingTools, setPendingTools] = useState<Set<string> | null>(null);
  const [toolSummaryHeight, setToolSummaryHeight] = useState<number | null>(null);
  const [userContextPolicy, setUserContextPolicy] = useState<Set<UserContextSection>>(
    () => new Set(defaultPolicyForKind('mode')),
  );
  const [submitting, setSubmitting] = useState(false);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState<string | null>(null);
  const definitionColumnRef = useRef<HTMLDivElement>(null);
  const capabilitiesColumnRef = useRef<HTMLDivElement>(null);
  const toolSummaryRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    toolAPI
      .getAllToolsInfo()
      .then((tools) => {
        const normalizedTools = tools
          .map((tool): SubagentEditorToolInfo | null => {
            const toolName = typeof tool?.name === 'string' ? tool.name : '';
            if (!toolName || !isUserSelectableToolName(toolName)) {
              return null;
            }
            return {
              name: toolName,
              description: typeof tool?.description === 'string' ? tool.description : '',
              isReadonly: Boolean(tool?.is_readonly),
              needsPermissions: !tool?.is_readonly,
              dynamicInfo: tool?.dynamic_info,
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
    if (isEdit) {
      return;
    }
    setLevel('user');
    setReadonly(defaultReadonlyForKind(kind));
    setReview(false);
    setUserContextPolicy(new Set(defaultPolicyForKind(kind)));
    setSelectedTools(defaultSelectedTools(toolInfos, kind, false));
    setToolsEditing(false);
    setPendingTools(null);
  }, [isEdit, kind, toolInfos]);

  useEffect(() => {
    if (isEdit || toolInfos.length === 0) {
      return;
    }

    setSelectedTools((prev) => {
      if (prev.size > 0) {
        return prev;
      }
      return defaultSelectedTools(toolInfos, kind, review);
    });
  }, [isEdit, kind, review, toolInfos]);

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
        const detail = await CustomAgentAPI.getCustomAgentDetail({
          agentId: editingAgentId,
          workspacePath: workspacePath || undefined,
        });
        if (cancelled) {
          return;
        }
        setKind(detail.kind);
        setLevel(detail.level);
        setAgentId(detail.agentId);
        setName(detail.name);
        setDescription(detail.description);
        setPrompt(detail.prompt);
        setReadonly(detail.readonly);
        setReview(detail.review);
        setSelectedTools(new Set(detail.tools ?? []));
        setToolsEditing(false);
        setPendingTools(null);
        setUserContextPolicy(new Set(detail.userContextPolicy));
        setAgentIdError(null);
        setNameError(null);
      } catch (error) {
        if (cancelled) {
          return;
        }
        setDetailError(error instanceof Error ? error.message : String(error));
      } finally {
        if (!cancelled) {
          setDetailLoading(false);
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [editingAgentId, isEdit, workspacePath]);

  const validateAgentId = useCallback(
    (value: string) => {
      if (!value.trim()) {
        return t('agentsOverview.form.idRequired');
      }
      if (!ID_REGEX.test(value.trim())) {
        return t('agentsOverview.form.idFormat');
      }
      return null;
    },
    [t],
  );

  const validateName = useCallback(
    (value: string) => {
      if (!value.trim()) {
        return t('agentsOverview.form.nameRequired');
      }
      return null;
    },
    [t],
  );

  const toggleContextSection = useCallback((section: UserContextSection) => {
    setUserContextPolicy((prev) => {
      const next = new Set(prev);
      if (next.has(section)) {
        next.delete(section);
      } else {
        next.add(section);
      }
      return next;
    });
  }, []);

  const handleReviewChange = useCallback(
    (nextReview: boolean) => {
      setReview(nextReview);
      const next = normalizeReviewModeState({
        review: nextReview,
        readonly,
        selectedTools,
        availableTools: toolInfos,
      });
      setReadonly(next.readonly);
      setSelectedTools(next.selectedTools);
      setPendingTools((current) => current ? new Set(next.selectedTools) : null);
    },
    [readonly, selectedTools, toolInfos],
  );

  const handleReadonlyChange = useCallback(
    (nextReadonly: boolean) => {
      if (review) {
        setReadonly(true);
        return;
      }
      setReadonly(nextReadonly);
    },
    [review],
  );

  const selectableTools = useMemo(
    () => filterToolsForReviewMode(toolInfos, kind === 'subagent' && review),
    [kind, review, toolInfos],
  );
  const selectableGroupTools = useMemo(() => selectableTools.map((tool) => ({
    name: tool.name,
    description: tool.description,
    is_readonly: tool.isReadonly,
    needs_permissions: tool.needsPermissions,
    dynamic_info: tool.dynamicInfo,
  })), [selectableTools]);
  const managementGroupTools = useMemo(() => toolInfos.map((tool) => ({
    name: tool.name,
    description: tool.description,
    is_readonly: tool.isReadonly,
    needs_permissions: tool.needsPermissions,
    dynamic_info: tool.dynamicInfo,
  })), [toolInfos]);

  useLayoutEffect(() => {
    const desktopMediaQuery = window.matchMedia('(min-width: 961px)');

    const updateToolSummaryHeight = () => {
      if (toolsEditing || !desktopMediaQuery.matches) {
        setToolSummaryHeight(null);
        return;
      }

      const definitionColumn = definitionColumnRef.current;
      const capabilitiesColumn = capabilitiesColumnRef.current;
      const toolSummary = toolSummaryRef.current;
      if (!definitionColumn || !capabilitiesColumn || !toolSummary) {
        setToolSummaryHeight(null);
        return;
      }

      const availableHeight =
        capabilitiesColumn.getBoundingClientRect().bottom
        - toolSummary.getBoundingClientRect().top;
      const nextHeight = Math.min(
        TOOL_SUMMARY_MAX_HEIGHT,
        Math.max(TOOL_SUMMARY_MIN_HEIGHT, Math.floor(availableHeight)),
      );
      setToolSummaryHeight((currentHeight) => (
        currentHeight === nextHeight ? currentHeight : nextHeight
      ));
    };

    updateToolSummaryHeight();
    const resizeObserver = typeof ResizeObserver === 'undefined'
      ? null
      : new ResizeObserver(updateToolSummaryHeight);
    if (definitionColumnRef.current) {
      resizeObserver?.observe(definitionColumnRef.current);
    }
    if (capabilitiesColumnRef.current) {
      resizeObserver?.observe(capabilitiesColumnRef.current);
    }
    window.addEventListener('resize', updateToolSummaryHeight);
    desktopMediaQuery.addEventListener('change', updateToolSummaryHeight);

    return () => {
      resizeObserver?.disconnect();
      window.removeEventListener('resize', updateToolSummaryHeight);
      desktopMediaQuery.removeEventListener('change', updateToolSummaryHeight);
    };
  }, [selectableTools.length, toolsEditing]);

  const contextSectionLabels: Record<UserContextSection, string> = {
    workspace_context: t('agentsOverview.form.contextWorkspaceContext'),
    workspace_instructions: t('agentsOverview.form.contextWorkspaceInstructions'),
    project_layout: t('agentsOverview.form.contextProjectLayout'),
  };
  const contextSectionTooltips: Record<UserContextSection, string> = {
    workspace_context: t('agentsOverview.form.contextWorkspaceContextTooltip'),
    workspace_instructions: t('agentsOverview.form.contextWorkspaceInstructionsTooltip'),
    project_layout: t('agentsOverview.form.contextProjectLayoutTooltip'),
  };

  const handleSubmit = useCallback(async () => {
    const nextAgentIdError = validateAgentId(agentId);
    const nextNameError = validateName(name);
    setAgentIdError(nextAgentIdError);
    setNameError(nextNameError);
    if (nextAgentIdError || nextNameError) {
      return;
    }
    if (!description.trim()) {
      notification.error(t('agentsOverview.form.descRequired'));
      return;
    }
    if (!prompt.trim()) {
      notification.error(t('agentsOverview.form.promptRequired'));
      return;
    }
    if (kind === 'subagent' && level === 'project' && !workspacePath) {
      notification.error(t('agentsOverview.form.noWorkspace'));
      return;
    }
    if (kind === 'mode' && level === 'project') {
      notification.error(t('agentsOverview.form.modeUserOnly'));
      return;
    }
    if (isEdit && !editingAgentId) {
      return;
    }

    setSubmitting(true);
    try {
      const payload = {
        kind,
        level: kind === 'subagent' ? level : 'user',
        id: agentId.trim(),
        name: name.trim(),
        description: description.trim(),
        prompt: prompt.trim(),
        readonly,
        review: kind === 'subagent' ? review : false,
        tools: selectedTools.size > 0 ? Array.from(selectedTools) : undefined,
        userContextPolicy: Array.from(userContextPolicy),
        workspacePath: kind === 'subagent' && level === 'project' ? workspacePath : undefined,
      } as const;

      if (isEdit && editingAgentId) {
        await CustomAgentAPI.updateCustomAgent({
          agentId: editingAgentId,
          name: payload.name,
          description: payload.description,
          prompt: payload.prompt,
          readonly: payload.readonly,
          review: payload.review,
          tools: payload.tools,
          userContextPolicy: payload.userContextPolicy,
          workspacePath: payload.workspacePath,
        });
        notification.success(t('agentsOverview.form.updateSuccess', { name: payload.name }));
      } else {
        await CustomAgentAPI.createCustomAgent(payload);
        notification.success(t('agentsOverview.form.createSuccess', { name: payload.name }));
      }
      openHome();
    } catch (error) {
      notification.error(
        `${
          isEdit
            ? t('agentsOverview.form.updateFailed')
            : t('agentsOverview.form.createFailed')
        }${error instanceof Error ? error.message : String(error)}`,
      );
    } finally {
      setSubmitting(false);
    }
  }, [
    agentId,
    description,
    editingAgentId,
    isEdit,
    kind,
    level,
    name,
    notification,
    openHome,
    prompt,
    readonly,
    review,
    selectedTools,
    t,
    userContextPolicy,
    validateAgentId,
    validateName,
    workspacePath,
  ]);

  const formTitle = isEdit
    ? t('agentsOverview.form.titleEdit')
    : t('agentsOverview.form.title');
  const formSubtitle = isEdit
    ? t(
        kind === 'subagent'
          ? 'agentsOverview.form.subtitleEditSubagent'
          : 'agentsOverview.form.subtitleEditMode',
      )
    : t('agentsOverview.form.subtitle');
  const submitLabel = isEdit
    ? t('agentsOverview.form.save')
    : t('agentsOverview.form.submit');
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
            <Button variant="secondary" size="small" onClick={openHome}>
              {t('agentsOverview.form.cancel')}
            </Button>
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
            <div className="th-create-page__heading">
              <h2 className="th__title">{formTitle}</h2>
              <p className="th__title-sub">{formSubtitle}</p>
            </div>
            <div className="th-create-page__actions">
              <Button variant="secondary" size="small" onClick={openHome} disabled={submitting}>
                {t('agentsOverview.form.cancel')}
              </Button>
              <Button
                variant="primary"
                size="small"
                onClick={() => void handleSubmit()}
                disabled={submitting || toolsEditing}
              >
                {submitting ? '…' : submitLabel}
              </Button>
            </div>
          </div>

          <div className="th-create-page__form">
            <div className="th-create-page__columns">
              <div
                ref={definitionColumnRef}
                className="th-create-page__column th-create-page__column--definition"
              >
                <div className="th-create-panel__field">
                  <label className="th-create-panel__label">{t('agentsOverview.form.kind')}</label>
                  <div className="th-create-panel__level-group">
                    {(['mode', 'subagent'] as CustomAgentKind[]).map((candidateKind) => (
                      <Tooltip
                        key={candidateKind}
                        content={t(
                          candidateKind === 'mode'
                            ? 'agentsOverview.form.kindAgentTooltip'
                            : 'agentsOverview.form.kindSubagentTooltip',
                        )}
                        placement="top"
                      >
                        <button
                          type="button"
                          disabled={isEdit}
                          className={`th-create-panel__level-btn${kind === candidateKind ? ' is-active' : ''}`}
                          onClick={() => setKind(candidateKind)}
                        >
                          {candidateKind === 'mode'
                            ? t('filters.mode')
                            : t('filters.subagent')}
                        </button>
                      </Tooltip>
                    ))}
                  </div>
                </div>

                <div className="th-create-panel__identity-fields">
                  <div className="th-create-panel__field">
                    <label className="th-create-panel__label">{t('agentsOverview.form.id')}</label>
                    <Input
                      value={agentId}
                      onChange={(event) => {
                        setAgentId(event.target.value);
                        setAgentIdError(validateAgentId(event.target.value));
                      }}
                      onBlur={() => setAgentIdError(validateAgentId(agentId))}
                      placeholder={t('agentsOverview.form.idPlaceholder')}
                      inputSize="small"
                      error={!!agentIdError}
                      disabled={isEdit}
                    />
                    {agentIdError ? (
                      <span className="th-create-panel__error">{agentIdError}</span>
                    ) : null}
                  </div>

                  <div className="th-create-panel__field">
                    <label className="th-create-panel__label">{t('agentsOverview.form.name')}</label>
                    <Input
                      value={name}
                      onChange={(event) => {
                        setName(event.target.value);
                        setNameError(validateName(event.target.value));
                      }}
                      onBlur={() => setNameError(validateName(name))}
                      placeholder={t('agentsOverview.form.namePlaceholder')}
                      inputSize="small"
                      error={!!nameError}
                    />
                    {nameError ? (
                      <span className="th-create-panel__error">{nameError}</span>
                    ) : null}
                  </div>
                </div>

                <div className="th-create-panel__field">
                  <label className="th-create-panel__label">
                    {t('agentsOverview.form.description')}
                  </label>
                  <Input
                    value={description}
                    onChange={(event) => setDescription(event.target.value)}
                    placeholder={t('agentsOverview.form.descPlaceholder')}
                    inputSize="small"
                  />
                </div>

                {kind === 'subagent' ? (
                  <div className="th-create-panel__field">
                    <label className="th-create-panel__label">{t('agentsOverview.form.level')}</label>
                    <div className="th-create-panel__level-group">
                      {(['user', 'project'] as CustomAgentLevel[]).map((candidateLevel) => {
                        const disabled =
                          (candidateLevel === 'project' && !hasWorkspace) || isEdit;
                        return (
                          <button
                            key={candidateLevel}
                            type="button"
                            disabled={disabled}
                            className={`th-create-panel__level-btn${level === candidateLevel ? ' is-active' : ''}`}
                            onClick={() => setLevel(candidateLevel)}
                            title={
                              disabled && !isEdit && candidateLevel === 'project'
                                ? t('agentsOverview.form.noWorkspace')
                                : undefined
                            }
                          >
                            {candidateLevel === 'user'
                              ? t('agentsOverview.filterUser')
                              : t('agentsOverview.filterProject')}
                          </button>
                        );
                      })}
                    </div>
                  </div>
                ) : null}

                <div className="th-create-panel__field th-create-panel__field--row">
                  <div className="th-create-panel__readonly-row">
                    <label className="th-create-panel__label">
                      {t('agentsOverview.form.readonly')}
                    </label>
                    <Switch
                      checked={readonly}
                      disabled={kind === 'subagent' && review}
                      onChange={(event) => handleReadonlyChange(event.target.checked)}
                      size="small"
                    />
                  </div>
                  {kind === 'subagent' ? (
                    <div className="th-create-panel__readonly-row">
                      <label className="th-create-panel__label">
                        {t('agentsOverview.form.review')}
                      </label>
                      <Switch
                        checked={review}
                        onChange={(event) => handleReviewChange(event.target.checked)}
                        size="small"
                      />
                    </div>
                  ) : null}
                </div>

                {selectableTools.length > 0 ? (
                  <div className="th-create-panel__field">
                    <div className="th-create-panel__field-head">
                      <label className="th-create-panel__label">
                        {t('agentsOverview.form.tools')}
                        <span className="th-create-panel__label-hint">
                          {kind === 'subagent' && review
                            ? t('agentsOverview.form.reviewToolsHint')
                            : t('agentsOverview.form.toolsHint', {
                                optionalLabel: t('agentsOverview.form.toolsOptional'),
                              })}
                        </span>
                      </label>
                      {toolsEditing ? (
                        <div className="th-create-panel__tool-edit-actions">
                          <Button
                            variant="ghost"
                            size="small"
                            onClick={() => {
                              setToolsEditing(false);
                              setPendingTools(null);
                            }}
                            disabled={submitting}
                          >
                            {t('agentsOverview.cancel')}
                          </Button>
                          <Button
                            variant="secondary"
                            size="small"
                            onClick={() => {
                              setSelectedTools(new Set(pendingTools ?? selectedTools));
                              setToolsEditing(false);
                              setPendingTools(null);
                            }}
                            disabled={submitting}
                          >
                            {t('agentsOverview.save')}
                          </Button>
                        </div>
                      ) : (
                        <Button
                          variant="ghost"
                          size="small"
                          onClick={() => {
                            setPendingTools(new Set(selectedTools));
                            setToolsEditing(true);
                          }}
                          disabled={submitting}
                        >
                          {t('agentsOverview.toolsEdit')}
                        </Button>
                      )}
                    </div>
                    {toolsEditing ? (
                      <ToolGroupPicker
                        tools={selectableGroupTools}
                        managementTools={managementGroupTools}
                        selectedToolNames={Array.from(pendingTools ?? selectedTools)}
                        userGroups={userToolGroups}
                        onSelectionChange={(toolNames) => setPendingTools(new Set(toolNames))}
                        onSaveUserGroups={saveUserToolGroups}
                        disabled={submitting}
                        testId="custom-agent-tool-groups"
                      />
                    ) : (
                      <div
                        ref={toolSummaryRef}
                        className="th-create-panel__tool-summary"
                        style={toolSummaryHeight === null ? undefined : { height: toolSummaryHeight }}
                      >
                        <ToolGroupSummary
                          tools={selectableGroupTools}
                          selectedToolNames={Array.from(selectedTools)}
                          userGroups={userToolGroups}
                        />
                      </div>
                    )}
                  </div>
                ) : null}

              </div>

              <div
                ref={capabilitiesColumnRef}
                className="th-create-page__column th-create-page__column--capabilities"
              >
                <div className="th-create-panel__field">
                  <label className="th-create-panel__label">
                    {t('agentsOverview.form.contextPolicy')}
                    <span className="th-create-panel__label-hint">
                      {t('agentsOverview.form.contextPolicyHint')}
                    </span>
                  </label>
                  <div className="th-create-panel__tools">
                    {VISIBLE_CONTEXT_SECTIONS.map((section) => {
                      const label = contextSectionLabels[section];
                      const tooltipContent = contextSectionTooltips[section];
                      return (
                        <Tooltip
                          key={section}
                          content={tooltipContent}
                          placement="top"
                          className="th-create-panel__context-tooltip"
                          interactive
                        >
                          <button
                            type="button"
                            className={`th-list__tool-item${userContextPolicy.has(section) ? ' is-on' : ''}`}
                            onClick={() => toggleContextSection(section)}
                            aria-label={`${label}: ${tooltipContent}`}
                          >
                            <span className="th-list__tool-item-name">
                              {label}
                            </span>
                          </button>
                        </Tooltip>
                      );
                    })}
                  </div>
                </div>

                <div className="th-create-panel__field th-create-panel__field--prompt">
                  <label className="th-create-panel__label">{t('agentsOverview.form.prompt')}</label>
                  <Textarea
                    className="th-create-panel__prompt-textarea"
                    value={prompt}
                    onChange={(event) => setPrompt(event.target.value)}
                    placeholder={t('agentsOverview.form.promptPlaceholder')}
                    rows={23}
                  />
                </div>
              </div>
            </div>

          </div>
        </div>
      </div>
    </div>
  );
};

export default CreateAgentPage;
