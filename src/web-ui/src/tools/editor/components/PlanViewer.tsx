/** Optimized viewer/editor for `.plan.md` files (frontmatter + markdown body). */

import React, { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { Circle, ArrowRight, Check, XCircle, Loader2, CheckCircle, AlertCircle, FileText, Pencil, X, ChevronDown, Trash2, Plus } from 'lucide-react';
import yaml from 'yaml';
import { MEditor } from '../meditor';
import type { EditorInstance } from '../meditor';
import { createLogger } from '@/shared/utils/logger';
import { CubeLoading, Button } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { workspaceAPI } from '@/infrastructure/api/service-api/WorkspaceAPI';
import { flowChatManager } from '@/flow_chat/services/FlowChatManager';
import { fileSystemService } from '@/tools/file-system/services/FileSystemService';
import { planBuildStateService } from '@/shared/services/PlanBuildStateService';
import './PlanViewer.scss';

const log = createLogger('PlanViewer');

// Styles used by markdown rendering (math + code highlight).
import 'katex/dist/katex.min.css';
import 'highlight.js/styles/github-dark.css';

interface PlanTodo {
  id: string;
  content: string;
  status?: string;
  dependencies?: string[];
}

interface PlanData {
  name: string;
  overview: string;
  todos: PlanTodo[];
}

export interface PlanViewerProps {
  /** File path */
  filePath: string;
  /** Workspace path */
  workspacePath?: string;
  /** File name */
  fileName?: string;
  /** Jump to specified line number */
  jumpToLine?: number;
  /** Jump to specified column number */
  jumpToColumn?: number;
}

const PlanViewer: React.FC<PlanViewerProps> = ({
  filePath,
  workspacePath,
  fileName,
  jumpToLine: _jumpToLine,
  jumpToColumn: _jumpToColumn,
}) => {
  const { t } = useI18n('tools');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [planData, setPlanData] = useState<PlanData | null>(null);
  const [planContent, setPlanContent] = useState<string>('');
  // Initialize build state from the shared service to survive unmounts.
  const [isBuildStarted, setIsBuildStarted] = useState(() => {
    return filePath ? planBuildStateService.isBuildActive(filePath) : false;
  });
  const [originalContent, setOriginalContent] = useState('');
  // Edit mode: display raw yaml frontmatter
  const [isEditingYaml, setIsEditingYaml] = useState(false);
  const [yamlContent, setYamlContent] = useState<string>('');
  const [originalYamlContent, setOriginalYamlContent] = useState<string>('');
  // Todos list expand/collapse state (collapsed by default)
  const [isTodosExpanded, setIsTodosExpanded] = useState(false);
  const [isTrailingTodoEditing, setIsTrailingTodoEditing] = useState(false);
  const [trailingTodoDrafts, setTrailingTodoDrafts] = useState<Record<string, string>>({});
  const [trailingDeletedTodoKeys, setTrailingDeletedTodoKeys] = useState<string[]>([]);
  const [trailingAddedTodos, setTrailingAddedTodos] = useState<PlanTodo[]>([]);
  
  const editorRef = useRef<EditorInstance>(null);
  const yamlEditorRef = useRef<EditorInstance>(null);
  const isUnmountedRef = useRef(false);

  const basePath = useMemo(() => {
    if (!filePath) return undefined;
    const normalizedPath = filePath.replace(/\\/g, '/');
    const lastSlashIndex = normalizedPath.lastIndexOf('/');
    if (lastSlashIndex >= 0) {
      return normalizedPath.substring(0, lastSlashIndex);
    }
    return undefined;
  }, [filePath, t]);

  const displayFileName = useMemo(() => {
    if (fileName) return fileName;
    if (!filePath) return '';
    const parts = filePath.replace(/\\/g, '/').split('/');
    return parts[parts.length - 1] || '';
  }, [filePath, fileName]);

  const hasTodos = !!(planData?.todos && planData.todos.length > 0);

  useEffect(() => {
    isUnmountedRef.current = false;
    return () => {
      isUnmountedRef.current = true;
      editorRef.current?.destroy();
      yamlEditorRef.current?.destroy();
    };
  }, []);

  const loadFileContent = useCallback(async () => {
    if (!filePath || isUnmountedRef.current) return;

    if (planBuildStateService.isFileWriting(filePath)) {
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const content = await workspaceAPI.readFileContent(filePath);

      const frontmatterMatch = content.match(/^---\n([\s\S]*?)\n---/);
      if (frontmatterMatch) {
        const rawYaml = frontmatterMatch[1];
        const parsed = yaml.parse(rawYaml);
        const markdownContent = content.replace(/^---\n[\s\S]*?\n---\n*/, '').trim();

        if (!isUnmountedRef.current) {
          setPlanData({
            name: parsed.name || '',
            overview: parsed.overview || '',
            todos: parsed.todos || [],
          });
          setPlanContent(markdownContent);
          setOriginalContent(markdownContent);
          setYamlContent(rawYaml);
          setOriginalYamlContent(rawYaml);
        }
      } else {
        if (!isUnmountedRef.current) {
          setPlanData(null);
          setPlanContent(content);
          setOriginalContent(content);
        }
      }
    } catch (err) {
      if (!isUnmountedRef.current) {
        const errStr = String(err);
        log.error('Failed to load file', err);
        // Simplify error message
        let displayError = t('editor.common.loadFailed');
        if (errStr.includes('does not exist') || errStr.includes('No such file')) {
          displayError = t('editor.common.fileNotFound');
        } else if (errStr.includes('Permission denied') || errStr.includes('permission')) {
          displayError = t('editor.common.permissionDenied');
        }
        setError(displayError);
      }
    } finally {
      if (!isUnmountedRef.current) {
        setLoading(false);
      }
    }
  }, [filePath]);

  useEffect(() => {
    loadFileContent();
  }, [loadFileContent]);

  useEffect(() => {
    if (!filePath) return;

    const normalizedPlanPath = filePath.replace(/\\/g, '/');
    const dirPath = filePath.substring(0, filePath.lastIndexOf('\\') >= 0 
      ? filePath.lastIndexOf('\\') 
      : filePath.lastIndexOf('/'));

    let debounceTimer: ReturnType<typeof setTimeout> | null = null;

    const unwatch = fileSystemService.watchFileChanges(dirPath, (event) => {
      const eventPath = event.path.replace(/\\/g, '/');
      if (eventPath !== normalizedPlanPath) return;
      if (event.type !== 'modified') return;

      if (debounceTimer) {
        clearTimeout(debounceTimer);
      }
      debounceTimer = setTimeout(() => {
        loadFileContent();
      }, 300);
    });

    return () => {
      unwatch();
      if (debounceTimer) {
        clearTimeout(debounceTimer);
      }
    };
  }, [filePath, loadFileContent]);

  // Subscribe to shared build state service for cross-component sync.
  useEffect(() => {
    if (!filePath) return;

    // Sync initial state (in case filePath just became available).
    setIsBuildStarted(planBuildStateService.isBuildActive(filePath));

    const unsubscribe = planBuildStateService.subscribe(filePath, (event) => {
      setIsBuildStarted(event.isBuilding);

      if (event.updatedTodos) {
        // Update plan data with latest todos.
        setPlanData(prev => prev ? { ...prev, todos: event.updatedTodos! } : null);

        // Update yaml content for edit mode consistency.
        if (event.updatedFrontmatter) {
          setYamlContent(event.updatedFrontmatter);
          setOriginalYamlContent(event.updatedFrontmatter);
        }
      }
    });

    return unsubscribe;
  }, [filePath]);

  const remainingTodos = useMemo(() => {
    if (!planData?.todos) return 0;
    return planData.todos.filter(t => t.status !== 'completed').length;
  }, [planData]);

  const buildStatus = useMemo((): 'build' | 'building' | 'built' => {
    if (planData?.todos?.length) {
      const statuses = planData.todos.map(t => t.status);
      if (statuses.every(s => s === 'completed')) {
        return 'built';
      }
    }
    if (isBuildStarted) {
      return 'building';
    }
    return 'build';
  }, [planData, isBuildStarted]);

  useEffect(() => {
    if (buildStatus === 'built' && isBuildStarted) {
      setIsBuildStarted(false);
    }
  }, [buildStatus, isBuildStarted]);

  const hasUnsavedChanges = useMemo(() => {
    const yamlChanged = yamlContent !== originalYamlContent;
    const contentChanged = planContent !== originalContent;
    return yamlChanged || contentChanged;
  }, [yamlContent, originalYamlContent, planContent, originalContent]);

  const saveFileContent = useCallback(async () => {
    if (!hasUnsavedChanges || !filePath) return;

    try {
      // Rebuild full content
      let fullContent = '';
      if (yamlContent) {
        fullContent = `---\n${yamlContent}\n---\n\n${planContent}`;
      } else {
        fullContent = planContent;
      }

      await workspaceAPI.writeFileContent(workspacePath || '', filePath, fullContent);
      setOriginalContent(planContent);
      setOriginalYamlContent(yamlContent);
      
      // Re-parse yaml to update planData
      if (yamlContent) {
        try {
          const parsed = yaml.parse(yamlContent);
          setPlanData({
            name: parsed.name || '',
            overview: parsed.overview || '',
            todos: parsed.todos || [],
          });
        } catch (e) {
          log.warn('YAML parse failed', e);
        }
      }
    } catch (err) {
      log.error('Failed to save file', err);
    }
  }, [planContent, yamlContent, filePath, workspacePath, hasUnsavedChanges]);

  const handleContentChange = useCallback((newContent: string) => {
    setPlanContent(newContent);
  }, []);

  const handleYamlChange = useCallback((newContent: string) => {
    setYamlContent(newContent);
  }, []);

  const handleSave = useCallback((_value: string) => {
    saveFileContent();
  }, [saveFileContent]);

  const startTrailingTodoEdit = useCallback(() => {
    if (!planData?.todos?.length) return;
    const drafts: Record<string, string> = {};
    planData.todos.forEach((todo, index) => {
      const key = todo.id || String(index);
      drafts[key] = todo.content;
    });
    setTrailingTodoDrafts(drafts);
    setTrailingDeletedTodoKeys([]);
    setTrailingAddedTodos([]);
    setIsTrailingTodoEditing(true);
  }, [planData]);

  const cancelTrailingTodoEdit = useCallback(() => {
    setIsTrailingTodoEditing(false);
    setTrailingTodoDrafts({});
    setTrailingDeletedTodoKeys([]);
    setTrailingAddedTodos([]);
  }, []);

  const handleAddTrailingTodo = useCallback(() => {
    const id = `new-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    const newTodo: PlanTodo = {
      id,
      content: '',
      status: 'pending',
    };
    setTrailingAddedTodos(prev => [...prev, newTodo]);
    setTrailingTodoDrafts(prev => ({ ...prev, [id]: '' }));
  }, []);

  const saveTrailingTodoEdit = useCallback(async () => {
    if (!planData?.todos?.length || !filePath) return;

    const nextTodos = planData.todos
      .map((todo, index) => ({ todo, key: todo.id || String(index) }))
      .filter(({ key }) => !trailingDeletedTodoKeys.includes(key))
      .map(({ todo, key }) => {
        const nextContent = (trailingTodoDrafts[key] ?? todo.content).trim();
        return {
          ...todo,
          content: nextContent || todo.content,
        };
      })
      .concat(
        trailingAddedTodos
          .map(todo => ({
            ...todo,
            content: (trailingTodoDrafts[todo.id] ?? todo.content).trim(),
          }))
          .filter(todo => !!todo.content)
      );

    let nextYamlContent = yamlContent;
    if (yamlContent) {
      try {
        const parsed = yaml.parse(yamlContent) || {};
        parsed.todos = nextTodos;
        nextYamlContent = yaml.stringify(parsed).trimEnd();
      } catch (e) {
        log.warn('Failed to parse yaml while saving trailing todo edit', e);
      }
    }

    try {
      const fullContent = nextYamlContent
        ? `---\n${nextYamlContent}\n---\n\n${planContent}`
        : planContent;
      await workspaceAPI.writeFileContent(workspacePath || '', filePath, fullContent);
      setPlanData(prev => (prev ? { ...prev, todos: nextTodos } : prev));
      setYamlContent(nextYamlContent);
      setOriginalYamlContent(nextYamlContent);
      setOriginalContent(planContent);
      setIsTrailingTodoEditing(false);
      setTrailingTodoDrafts({});
      setTrailingDeletedTodoKeys([]);
      setTrailingAddedTodos([]);
    } catch (err) {
      log.error('Failed to save trailing todo edit', err);
    }
  }, [planData, trailingAddedTodos, trailingDeletedTodoKeys, trailingTodoDrafts, yamlContent, planContent, workspacePath, filePath]);

  const handleDeleteTrailingTodo = useCallback((todoKey: string) => {
    if (todoKey.startsWith('new-')) {
      setTrailingAddedTodos(prev => prev.filter(todo => todo.id !== todoKey));
      setTrailingTodoDrafts(prev => {
        const { [todoKey]: _removed, ...rest } = prev;
        return rest;
      });
      return;
    }
    setTrailingDeletedTodoKeys(prev => (prev.includes(todoKey) ? prev : [...prev, todoKey]));
  }, []);

  const displayedTodos = useMemo(() => {
    if (!planData?.todos) return [];
    if (!isTrailingTodoEditing) return planData.todos;
    return [
      ...planData.todos.filter((todo, index) => !trailingDeletedTodoKeys.includes(todo.id || String(index))),
      ...trailingAddedTodos,
    ];
  }, [isTrailingTodoEditing, planData, trailingAddedTodos, trailingDeletedTodoKeys]);

  const renderSharedTodoPanel = useCallback((placement: 'inline' | 'trailing') => {
    const panelClassName = placement === 'trailing'
      ? `plan-viewer-todos plan-viewer-todos--trailing ${isEditingYaml ? 'plan-viewer-todos--yaml-editing' : ''}`
      : `plan-viewer-todos ${isTodosExpanded ? 'plan-viewer-todos--expanded' : ''} ${isEditingYaml ? 'plan-viewer-todos--yaml-editing' : ''}`;
    const toolbarClassName = `${placement === 'trailing' ? 'trailing-todo-toolbar' : 'todo-inline-toolbar'} ${isEditingYaml ? 'todo-toolbar--yaml' : ''}`;

    return (
      <div className={panelClassName}>
        <div className={toolbarClassName}>
          {isEditingYaml ? (
            <button
              type="button"
              className="edit-btn"
              onClick={() => setIsEditingYaml(false)}
              title={t('editor.planViewer.toggleYamlEditOff')}
            >
              <X size={14} />
            </button>
          ) : isTrailingTodoEditing ? (
            <>
              <button
                type="button"
                className="edit-btn"
                onClick={handleAddTrailingTodo}
                title={t('editor.common.add')}
              >
                <Plus size={14} />
              </button>
              <button
                type="button"
                className="edit-btn edit-btn--confirm"
                onClick={saveTrailingTodoEdit}
                title={t('editor.common.save')}
              >
                <Check size={14} />
              </button>
              <button
                type="button"
                className="edit-btn"
                onClick={cancelTrailingTodoEdit}
                title={t('editor.common.cancel')}
              >
                <X size={14} />
              </button>
            </>
          ) : (
            <>
              <button
                type="button"
                className="edit-btn"
                onClick={() => setIsEditingYaml(true)}
                title={t('editor.planViewer.toggleYamlEditOn')}
              >
                <FileText size={14} />
              </button>
              <button
                type="button"
                className="edit-btn"
                onClick={startTrailingTodoEdit}
                title={t('editor.common.edit')}
              >
                <Pencil size={14} />
              </button>
            </>
          )}
        </div>

        {isEditingYaml ? (
          <div className="yaml-editor-section">
            <div className="yaml-editor-content">
              <MEditor
                ref={yamlEditorRef}
                value={yamlContent}
                onChange={handleYamlChange}
                onSave={handleSave}
                mode="edit"
                theme="dark"
                height="200px"
                width="100%"
                placeholder={t('editor.planViewer.yamlPlaceholder')}
                readonly={false}
                toolbar={false}
                autofocus={true}
              />
            </div>
          </div>
        ) : (
          <div className="todos-list">
            {displayedTodos.map((todo, index) => (
              <div
                key={todo.id || index}
                className={`todo-item status-${todo.status || 'pending'}`}
              >
                {getTodoIcon(todo.status)}
                {isTrailingTodoEditing ? (
                  <>
                    <input
                      className="todo-content-input"
                      value={trailingTodoDrafts[todo.id || String(index)] ?? todo.content}
                      onChange={(e) => {
                        const key = todo.id || String(index);
                        setTrailingTodoDrafts(prev => ({
                          ...prev,
                          [key]: e.target.value,
                        }));
                      }}
                    />
                    <button
                      type="button"
                      className="todo-delete-btn"
                      onClick={() => handleDeleteTrailingTodo(todo.id || String(index))}
                      title={t('editor.common.delete')}
                    >
                      <Trash2 size={13} />
                    </button>
                  </>
                ) : (
                  <span className="todo-content">{todo.content}</span>
                )}
              </div>
            ))}
          </div>
        )}
      </div>
    );
  }, [
    cancelTrailingTodoEdit,
    displayedTodos,
    handleAddTrailingTodo,
    handleDeleteTrailingTodo,
    handleSave,
    handleYamlChange,
    isEditingYaml,
    isTodosExpanded,
    isTrailingTodoEditing,
    saveTrailingTodoEdit,
    startTrailingTodoEdit,
    t,
    trailingTodoDrafts,
    yamlContent,
  ]);

  // Build button click handler
  const handleBuild = useCallback(async () => {
    if (!filePath || buildStatus !== 'build' || !planData) return;

    try {
      // Register build in shared service (notifies all subscribers including CreatePlanDisplay).
      const todoIds = planData.todos.map(t => t.id);
      planBuildStateService.startBuild(filePath, todoIds);

      // Process todos, keep only id, content, and status
      const simpleTodos = planData.todos.map(t => ({
        id: t.id,
        content: t.content,
        status: t.status,
      }));

      const message = `Implement the plan as specified, it is attached for your reference. Do NOT edit the plan file itself. To-do's from the plan have already been created. Do not create them again. Mark them as in_progress as you work, starting with the first one. Don't stop until you have completed all the to-dos.

<attached_file path="${filePath}">
<plan>
${planContent}
</plan>
<todos>
${JSON.stringify(simpleTodos, null, 2)}
</todos>
</attached_file>`;

      const displayMessage = t('editor.planViewer.buildPlanTitle', { name: planData.name });
      await flowChatManager.sendMessage(message, undefined, displayMessage, 'agentic', 'agentic');
    } catch (err) {
      log.error('Build failed', err);
      planBuildStateService.cancelBuild(filePath);
    }
  }, [filePath, buildStatus, planData, planContent, t]);

  // Get todo status icon
  const getTodoIcon = (status?: string) => {
    switch (status) {
      case 'completed':
        return <Check size={14} className="todo-icon todo-icon--completed" />;
      case 'in_progress':
        return <ArrowRight size={14} className="todo-icon todo-icon--in-progress" />;
      case 'cancelled':
        return <XCircle size={14} className="todo-icon todo-icon--cancelled" />;
      case 'pending':
      default:
        return <Circle size={14} className="todo-icon todo-icon--pending" />;
    }
  };

  // Render loading state
  if (loading) {
    return (
      <div className="bitfun-plan-viewer bitfun-plan-viewer--loading">
        <CubeLoading size="medium" text={t('editor.planViewer.loadingPlan')} />
      </div>
    );
  }

  // Render error state
  if (error) {
    return (
      <div className="bitfun-plan-viewer bitfun-plan-viewer--error">
        <div className="error-content">
          <AlertCircle className="error-icon" />
          <p>{error}</p>
          <Button variant="secondary" size="small" onClick={loadFileContent}>
            {t('editor.common.retry')}
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className="bitfun-plan-viewer">
      <div
        className={`plan-viewer-header ${hasTodos ? 'plan-viewer-header--collapsible' : ''}`}
        onClick={() => {
          if (hasTodos && !isEditingYaml) {
            setIsTodosExpanded(!isTodosExpanded);
          }
        }}
      >
        <div className="header-left">
          {hasTodos && (
            <span
              className={`header-expand-indicator ${isTodosExpanded ? 'header-expand-indicator--expanded' : ''} ${isEditingYaml ? 'header-expand-indicator--disabled' : ''}`}
            >
              <ChevronDown size={14} />
            </span>
          )}
          <FileText size={16} className="file-icon" />
          <span className="file-name">{displayFileName}</span>
          {hasUnsavedChanges && <span className="unsaved-indicator">{t('editor.planViewer.unsaved')}</span>}
        </div>
        <div className="header-right" onClick={(e) => e.stopPropagation()}>
          {hasTodos && (
            <>
              <span className="todos-count">{t('editor.planViewer.remainingTodos', { count: remainingTodos })}</span>

              <button
                className={`build-btn build-btn--${buildStatus}`}
                onClick={handleBuild}
                disabled={buildStatus !== 'build'}
              >
                {buildStatus === 'building' ? (
                  <>
                    <Loader2 size={14} className="animate-spin" />
                    <span>{t('editor.planViewer.building')}</span>
                  </>
                ) : buildStatus === 'built' ? (
                  <>
                    <CheckCircle size={14} />
                    <span>{t('editor.planViewer.built')}</span>
                  </>
                ) : (
                  <span>{t('editor.planViewer.build')}</span>
                )}
              </button>
            </>
          )}
        </div>
      </div>

      {hasTodos && (isEditingYaml || isTodosExpanded) && renderSharedTodoPanel('inline')}

      <div className="plan-viewer-content">
        <div className="plan-markdown">
          <MEditor
            ref={editorRef}
            value={planContent}
            onChange={handleContentChange}
            onSave={handleSave}
            mode="ir"
            theme="dark"
            height="auto"
            width="100%"
            placeholder={t('editor.planViewer.contentPlaceholder')}
            readonly={false}
            toolbar={false}
            basePath={basePath}
          />
        </div>
        {hasTodos && isTodosExpanded && !isEditingYaml && renderSharedTodoPanel('trailing')}
      </div>
    </div>
  );
};

export default PlanViewer;
