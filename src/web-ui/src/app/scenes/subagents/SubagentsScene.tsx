import React, { useCallback, useEffect, useMemo, useState } from 'react';
import {
  BadgeAlert,
  Bot,
  ChevronRight,
  FilePenLine,
  Plus,
  Shield,
  Trash2,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import {
  Badge,
  Button,
  Empty,
  Input,
  Search,
  Switch,
  Textarea,
  confirmDanger,
} from '@/component-library';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { SubagentAPI, type SubagentDetail, type SubagentInfo, type SubagentLevel } from '@/infrastructure/api/service-api/SubagentAPI';
import { useGallerySceneAutoRefresh } from '@/app/hooks/useGallerySceneAutoRefresh';
import { useNotification } from '@/shared/notification-system';
import './SubagentsScene.scss';

type SceneMode = 'browse' | 'create' | 'edit';
type SidebarFilter = 'all' | 'ready' | 'readonly' | 'user' | 'project' | 'builtin';

const NAME_REGEX = /^[a-zA-Z][a-zA-Z0-9_-]*$/;

interface EditorState {
  level: SubagentLevel;
  name: string;
  description: string;
  prompt: string;
  readonly: boolean;
  enabled: boolean;
  tools: Set<string>;
}

const EMPTY_EDITOR: EditorState = {
  level: 'user',
  name: '',
  description: '',
  prompt: '',
  readonly: true,
  enabled: true,
  tools: new Set<string>(),
};

const SubagentsScene: React.FC = () => {
  const { t } = useTranslation('scenes/subagents');
  const notification = useNotification();
  const { hasWorkspace, workspacePath } = useCurrentWorkspace();

  const [mode, setMode] = useState<SceneMode>('browse');
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState<SidebarFilter>('all');
  const [subagents, setSubagents] = useState<SubagentInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [toolNames, setToolNames] = useState<string[]>([]);
  const [editor, setEditor] = useState<EditorState>(EMPTY_EDITOR);
  const [nameError, setNameError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [detailLoading, setDetailLoading] = useState(false);

  const loadSubagents = useCallback(async () => {
    setLoading(true);
    try {
      const [items, tools] = await Promise.all([
        SubagentAPI.listSubagents({ workspacePath: workspacePath || undefined }),
        SubagentAPI.listAgentToolNames().catch(() => []),
      ]);
      setSubagents(items);
      setToolNames(tools);
      setSelectedId((current) => {
        if (current && items.some((item) => item.id === current)) return current;
        return items[0]?.id ?? null;
      });
    } finally {
      setLoading(false);
    }
  }, [workspacePath]);

  useEffect(() => {
    void loadSubagents();
  }, [loadSubagents]);

  useGallerySceneAutoRefresh({
    sceneId: 'subagents',
    refetch: () => void loadSubagents(),
  });

  const selected = useMemo(
    () => subagents.find((item) => item.id === selectedId) ?? null,
    [selectedId, subagents],
  );

  const filteredSubagents = useMemo(() => {
    const q = query.trim().toLowerCase();
    return subagents.filter((item) => {
      if (q) {
        const hay = `${item.name} ${item.description} ${item.model ?? ''}`.toLowerCase();
        if (!hay.includes(q)) return false;
      }

      switch (filter) {
        case 'ready':
          return item.enabled;
        case 'readonly':
          return item.isReadonly;
        case 'user':
        case 'project':
        case 'builtin':
          return (item.subagentSource ?? 'builtin') === filter;
        default:
          return true;
      }
    });
  }, [filter, query, subagents]);

  const counts = useMemo(() => ({
    all: subagents.length,
    ready: subagents.filter((item) => item.enabled).length,
    readonly: subagents.filter((item) => item.isReadonly).length,
    user: subagents.filter((item) => item.subagentSource === 'user').length,
    project: subagents.filter((item) => item.subagentSource === 'project').length,
    builtin: subagents.filter((item) => (item.subagentSource ?? 'builtin') === 'builtin').length,
  }), [subagents]);

  const validateName = useCallback((value: string) => {
    if (!value.trim()) return t('editor.validation.nameRequired');
    if (!NAME_REGEX.test(value.trim())) return t('editor.validation.nameFormat');
    return null;
  }, [t]);

  const resetEditor = useCallback(() => {
    setEditor({
      ...EMPTY_EDITOR,
      level: hasWorkspace ? 'project' : 'user',
    });
    setNameError(null);
  }, [hasWorkspace]);

  const startCreate = useCallback(() => {
    resetEditor();
    setMode('create');
  }, [resetEditor]);

  const openDetail = useCallback((subagentId: string) => {
    setSelectedId(subagentId);
    setMode('browse');
  }, []);

  const loadDetailIntoEditor = useCallback(async (detail: SubagentDetail) => {
    setEditor({
      level: detail.level,
      name: detail.name,
      description: detail.description,
      prompt: detail.prompt,
      readonly: detail.readonly,
      enabled: detail.enabled,
      tools: new Set(detail.tools ?? []),
    });
    setNameError(null);
  }, []);

  const startEdit = useCallback(async () => {
    if (!selected) return;
    setDetailLoading(true);
    try {
      const detail = await SubagentAPI.getSubagentDetail({
        subagentId: selected.id,
        workspacePath: workspacePath || undefined,
      });
      await loadDetailIntoEditor(detail);
      setMode('edit');
    } catch (error) {
      notification.error(
        `${t('messages.loadDetailFailed')}${error instanceof Error ? error.message : String(error)}`,
      );
    } finally {
      setDetailLoading(false);
    }
  }, [loadDetailIntoEditor, notification, selected, t, workspacePath]);

  const handleToggleEnabled = useCallback(async (subagent: SubagentInfo, enabled: boolean) => {
    try {
      await SubagentAPI.updateSubagentConfig({
        subagentId: subagent.id,
        enabled,
      });
      setSubagents((current) =>
        current.map((item) => (item.id === subagent.id ? { ...item, enabled } : item)),
      );
      notification.success(
        enabled ? t('messages.enabledSuccess', { name: subagent.name }) : t('messages.disabledSuccess', { name: subagent.name }),
      );
    } catch (error) {
      notification.error(
        `${t('messages.toggleFailed')}${error instanceof Error ? error.message : String(error)}`,
      );
    }
  }, [notification, t]);

  const handleDelete = useCallback(async () => {
    if (!selected) return;
    const source = selected.subagentSource ?? 'builtin';
    if (source === 'builtin') return;
    const ok = await confirmDanger(
      t('deleteDialog.title'),
      t('deleteDialog.message', { name: selected.name }),
    );
    if (!ok) return;

    setDeleting(true);
    try {
      await SubagentAPI.deleteSubagent(selected.id);
      notification.success(t('messages.deleteSuccess', { name: selected.name }));
      setMode('browse');
      await loadSubagents();
    } catch (error) {
      notification.error(
        `${t('messages.deleteFailed')}${error instanceof Error ? error.message : String(error)}`,
      );
    } finally {
      setDeleting(false);
    }
  }, [loadSubagents, notification, selected, t]);

  const toggleTool = useCallback((toolName: string) => {
    setEditor((current) => {
      const next = new Set(current.tools);
      if (next.has(toolName)) next.delete(toolName);
      else next.add(toolName);
      return { ...current, tools: next };
    });
  }, []);

  const handleSubmit = useCallback(async () => {
    if (mode === 'create') {
      const err = validateName(editor.name);
      setNameError(err);
      if (err) return;
    }
    if (!editor.description.trim()) {
      notification.error(t('editor.validation.descriptionRequired'));
      return;
    }
    if (!editor.prompt.trim()) {
      notification.error(t('editor.validation.promptRequired'));
      return;
    }
    if (editor.level === 'project' && !workspacePath) {
      notification.error(t('editor.validation.projectWorkspaceRequired'));
      return;
    }

    setSaving(true);
    try {
      if (mode === 'create') {
        await SubagentAPI.createSubagent({
          level: editor.level,
          name: editor.name.trim(),
          description: editor.description.trim(),
          prompt: editor.prompt.trim(),
          readonly: editor.readonly,
          tools: editor.tools.size > 0 ? Array.from(editor.tools) : undefined,
          workspacePath: editor.level === 'project' ? workspacePath || undefined : undefined,
        });
        notification.success(t('messages.createSuccess', { name: editor.name.trim() }));
      } else if (mode === 'edit' && selected) {
        await SubagentAPI.updateSubagent({
          subagentId: selected.id,
          description: editor.description.trim(),
          prompt: editor.prompt.trim(),
          readonly: editor.readonly,
          tools: editor.tools.size > 0 ? Array.from(editor.tools) : undefined,
          workspacePath: editor.level === 'project' ? workspacePath || undefined : undefined,
        });
        if (editor.enabled !== selected.enabled) {
          await SubagentAPI.updateSubagentConfig({
            subagentId: selected.id,
            enabled: editor.enabled,
          });
        }
        notification.success(t('messages.updateSuccess', { name: selected.name }));
      }

      setMode('browse');
      await loadSubagents();
    } catch (error) {
      notification.error(
        `${mode === 'create' ? t('messages.createFailed') : t('messages.updateFailed')}${error instanceof Error ? error.message : String(error)}`,
      );
    } finally {
      setSaving(false);
    }
  }, [editor, loadSubagents, mode, notification, selected, t, validateName, workspacePath]);

  const sourceTone = useCallback((source?: string): 'info' | 'success' | 'purple' => {
    if (source === 'user') return 'success';
    if (source === 'project') return 'purple';
    return 'info';
  }, []);

  const sourceLabel = useCallback((source?: string) => {
    switch (source) {
      case 'user':
        return t('badges.user');
      case 'project':
        return t('badges.project');
      default:
        return t('badges.builtin');
    }
  }, [t]);

  const readiness = useMemo(() => {
    const current = mode === 'browse' ? selected : null;
    if (!current) return null;
    if (!current.enabled) return { tone: 'neutral' as const, label: t('readiness.disabled') };
    if (!current.description.trim()) return { tone: 'warning' as const, label: t('readiness.incomplete') };
    return { tone: 'success' as const, label: t('readiness.ready') };
  }, [mode, selected, t]);

  const canManageSelected = Boolean(
    selected && (selected.subagentSource === 'user' || selected.subagentSource === 'project'),
  );

  const renderList = () => {
    if (loading) {
      return <div className="subagents-list__empty">{t('status.loading')}</div>;
    }
    if (filteredSubagents.length === 0) {
      return (
        <div className="subagents-list__empty">
          <Empty description={t('list.empty')} />
        </div>
      );
    }
    return filteredSubagents.map((item) => (
      <button
        key={item.id}
        type="button"
        className={`subagent-row${selectedId === item.id && mode === 'browse' ? ' is-active' : ''}`}
        onClick={() => openDetail(item.id)}
      >
        <div className="subagent-row__head">
          <div className="subagent-row__title-wrap">
            <span className="subagent-row__icon">
              <Bot size={15} />
            </span>
            <span className="subagent-row__title">{item.name}</span>
          </div>
          <ChevronRight size={14} className="subagent-row__chev" />
        </div>
        <p className="subagent-row__desc">{item.description || t('list.noDescription')}</p>
        <div className="subagent-row__meta">
          <Badge variant={sourceTone(item.subagentSource)}>{sourceLabel(item.subagentSource)}</Badge>
          {item.isReadonly ? <Badge variant="neutral">{t('badges.readonly')}</Badge> : null}
          {!item.enabled ? <Badge variant="warning">{t('badges.disabled')}</Badge> : null}
        </div>
      </button>
    ));
  };

  return (
    <div className="bitfun-subagents-scene">
      <header className="subagents-scene__header">
        <div className="subagents-scene__identity">
          <h1 className="subagents-scene__title">{t('page.title')}</h1>
          <div className="subagents-scene__subline">
            <p className="subagents-scene__subtitle">{t('page.subtitle')}</p>
            <div className="subagents-scene__actions">
              <Search
                className="subagents-scene__search"
                value={query}
                onChange={setQuery}
                onClear={() => setQuery('')}
                placeholder={t('page.searchPlaceholder')}
                size="small"
                clearable
              />
              <Button variant="primary" size="small" onClick={startCreate}>
                <Plus size={14} />
                <span>{t('page.newSubagent')}</span>
              </Button>
            </div>
          </div>
        </div>
      </header>

      <div className="subagents-scene__body">
        <div className="subagents-workbench">
          <aside className="subagents-sidebar">
            <div className="subagents-sidebar__scroll">
              <button
                type="button"
                className={`subagents-sidebar__item${filter === 'all' ? ' is-active' : ''}`}
                onClick={() => setFilter('all')}
              >
                <span>{t('sidebar.all')}</span>
                <span>{counts.all}</span>
              </button>
              <button
                type="button"
                className={`subagents-sidebar__item${filter === 'ready' ? ' is-active' : ''}`}
                onClick={() => setFilter('ready')}
              >
                <span>{t('sidebar.ready')}</span>
                <span>{counts.ready}</span>
              </button>
              <button
                type="button"
                className={`subagents-sidebar__item${filter === 'readonly' ? ' is-active' : ''}`}
                onClick={() => setFilter('readonly')}
              >
                <span>{t('sidebar.readonly')}</span>
                <span>{counts.readonly}</span>
              </button>

              <div className="subagents-sidebar__group">{t('sidebar.sources')}</div>

              {(['builtin', 'user', 'project'] as SidebarFilter[]).map((key) => (
                <button
                  key={key}
                  type="button"
                  className={`subagents-sidebar__item${filter === key ? ' is-active' : ''}`}
                  onClick={() => setFilter(key)}
                >
                  <span>{t(`sidebar.${key}`)}</span>
                  <span>{counts[key]}</span>
                </button>
              ))}
            </div>
          </aside>

          <section className="subagents-list">
            <div className="subagents-list__toolbar">
              <span>{t('list.resultCount', { count: filteredSubagents.length })}</span>
            </div>
            <div className="subagents-list__body">
              {renderList()}
            </div>
          </section>

          <section className="subagents-detail">
            {mode === 'browse' ? (
              selected ? (
                <div className="subagents-panel">
                  <div className="subagents-panel__head">
                    <div className="subagents-panel__title-wrap">
                      <span className="subagents-panel__icon"><Bot size={18} /></span>
                      <div>
                        <h2 className="subagents-panel__title">{selected.name}</h2>
                        <p className="subagents-panel__subtitle">{selected.description || t('detail.noDescription')}</p>
                      </div>
                    </div>
                    <div className="subagents-panel__actions">
                      {canManageSelected ? (
                        <>
                          <Button variant="secondary" size="small" onClick={() => void startEdit()} disabled={detailLoading}>
                            <FilePenLine size={13} />
                            <span>{t('detail.edit')}</span>
                          </Button>
                          <Button variant="secondary" size="small" onClick={() => void handleDelete()} disabled={deleting}>
                            <Trash2 size={13} />
                            <span>{t('detail.delete')}</span>
                          </Button>
                        </>
                      ) : null}
                    </div>
                  </div>

                  <div className="subagents-panel__scroll">
                    <div className="subagents-panel__badges">
                      <Badge variant={sourceTone(selected.subagentSource)}>{sourceLabel(selected.subagentSource)}</Badge>
                      {selected.model ? <Badge variant="neutral">{selected.model}</Badge> : null}
                      {selected.isReadonly ? <Badge variant="neutral">{t('badges.readonly')}</Badge> : null}
                      {readiness ? <Badge variant={readiness.tone}>{readiness.label}</Badge> : null}
                    </div>

                    <div className="subagents-panel__section">
                      <div className="subagents-panel__section-head">
                        <h3>{t('detail.sections.availability')}</h3>
                        <Switch
                          size="small"
                          checked={selected.enabled}
                          onChange={(event) => void handleToggleEnabled(selected, event.target.checked)}
                          disabled={!canManageSelected}
                        />
                      </div>
                      <p className="subagents-panel__hint">{t('detail.availabilityHint')}</p>
                    </div>

                    <div className="subagents-panel__section">
                      <h3>{t('detail.sections.positioning')}</h3>
                      <ul className="subagents-panel__bullets">
                        <li>{selected.description || t('detail.noDescription')}</li>
                        <li>{selected.isReadonly ? t('detail.readonlyHint') : t('detail.writeHint')}</li>
                        <li>{t('detail.toolCount', { count: selected.toolCount || selected.defaultTools?.length || 0 })}</li>
                      </ul>
                    </div>

                    <div className="subagents-panel__section">
                      <h3>{t('detail.sections.tools')}</h3>
                      {(selected.defaultTools ?? []).length > 0 ? (
                        <div className="subagents-panel__chips">
                          {(selected.defaultTools ?? []).map((tool) => (
                            <span key={tool} className="subagents-chip">{tool}</span>
                          ))}
                        </div>
                      ) : (
                        <p className="subagents-panel__hint">{t('detail.defaultToolsFallback')}</p>
                      )}
                    </div>

                    <div className="subagents-panel__section">
                      <h3>{t('detail.sections.calling')}</h3>
                      <div className="subagents-panel__callout">
                        <Shield size={14} />
                        <span>{selected.enabled ? t('detail.callReady') : t('detail.callBlocked')}</span>
                      </div>
                    </div>
                  </div>
                </div>
              ) : (
                <div className="subagents-panel subagents-panel--empty">
                  <Bot size={36} />
                  <p>{t('detail.empty')}</p>
                </div>
              )
            ) : (
              <div className="subagents-panel">
                <div className="subagents-panel__head">
                  <div className="subagents-panel__title-wrap">
                    <span className="subagents-panel__icon">
                      {mode === 'create' ? <Plus size={18} /> : <FilePenLine size={18} />}
                    </span>
                    <div>
                      <h2 className="subagents-panel__title">
                        {mode === 'create' ? t('editor.createTitle') : t('editor.editTitle')}
                      </h2>
                      <p className="subagents-panel__subtitle">
                        {mode === 'create' ? t('editor.createSubtitle') : t('editor.editSubtitle')}
                      </p>
                    </div>
                  </div>
                  <div className="subagents-panel__actions">
                    <Button variant="secondary" size="small" onClick={() => setMode('browse')}>
                      {t('editor.cancel')}
                    </Button>
                    <Button variant="primary" size="small" onClick={() => void handleSubmit()} disabled={saving}>
                      {mode === 'create' ? t('editor.create') : t('editor.save')}
                    </Button>
                  </div>
                </div>

                <div className="subagents-panel__scroll">
                  <div className="subagents-form">
                    <div className="subagents-form__field">
                      <label>{t('editor.fields.name')}</label>
                      <Input
                        value={editor.name}
                        onChange={(event) => {
                          setEditor((current) => ({ ...current, name: event.target.value }));
                          if (mode === 'create') setNameError(validateName(event.target.value));
                        }}
                        onBlur={() => mode === 'create' && setNameError(validateName(editor.name))}
                        disabled={mode === 'edit'}
                        inputSize="small"
                        placeholder={t('editor.placeholders.name')}
                        error={Boolean(nameError)}
                      />
                      {nameError ? <span className="subagents-form__error">{nameError}</span> : null}
                    </div>

                    <div className="subagents-form__row">
                      <div className="subagents-form__field">
                        <label>{t('editor.fields.level')}</label>
                        <div className="subagents-form__level-tabs">
                          {(['user', 'project'] as SubagentLevel[]).map((level) => {
                            const disabled = mode === 'edit' || (level === 'project' && !hasWorkspace);
                            return (
                              <button
                                key={level}
                                type="button"
                                className={`subagents-form__level-tab${editor.level === level ? ' is-active' : ''}`}
                                onClick={() => setEditor((current) => ({ ...current, level }))}
                                disabled={disabled}
                              >
                                {t(`editor.levels.${level}`)}
                              </button>
                            );
                          })}
                        </div>
                      </div>

                      <div className="subagents-form__field">
                        <label>{t('editor.fields.readonly')}</label>
                        <div className="subagents-form__switch-row">
                          <Switch
                            size="small"
                            checked={editor.readonly}
                            onChange={(event) => setEditor((current) => ({ ...current, readonly: event.target.checked }))}
                          />
                          <span>{editor.readonly ? t('editor.readonlyOn') : t('editor.readonlyOff')}</span>
                        </div>
                      </div>

                      {mode === 'edit' ? (
                        <div className="subagents-form__field">
                          <label>{t('editor.fields.enabled')}</label>
                          <div className="subagents-form__switch-row">
                            <Switch
                              size="small"
                              checked={editor.enabled}
                              onChange={(event) => setEditor((current) => ({ ...current, enabled: event.target.checked }))}
                            />
                            <span>{editor.enabled ? t('editor.enabledOn') : t('editor.enabledOff')}</span>
                          </div>
                        </div>
                      ) : null}
                    </div>

                    <div className="subagents-form__field">
                      <label>{t('editor.fields.description')}</label>
                      <Input
                        value={editor.description}
                        onChange={(event) => setEditor((current) => ({ ...current, description: event.target.value }))}
                        inputSize="small"
                        placeholder={t('editor.placeholders.description')}
                      />
                    </div>

                    <div className="subagents-form__field">
                      <label>{t('editor.fields.prompt')}</label>
                      <Textarea
                        value={editor.prompt}
                        onChange={(event) => setEditor((current) => ({ ...current, prompt: event.target.value }))}
                        rows={10}
                        placeholder={t('editor.placeholders.prompt')}
                      />
                    </div>

                    <div className="subagents-form__field">
                      <label>{t('editor.fields.tools')}</label>
                      <div className="subagents-form__tools">
                        {toolNames.length > 0 ? toolNames.map((tool) => (
                          <button
                            key={tool}
                            type="button"
                            className={`subagents-tool${editor.tools.has(tool) ? ' is-active' : ''}`}
                            onClick={() => toggleTool(tool)}
                          >
                            {tool}
                          </button>
                        )) : (
                          <div className="subagents-form__hint-inline">
                            <BadgeAlert size={14} />
                            <span>{t('editor.noTools')}</span>
                          </div>
                        )}
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            )}
          </section>
        </div>
      </div>
    </div>
  );
};

export default SubagentsScene;
