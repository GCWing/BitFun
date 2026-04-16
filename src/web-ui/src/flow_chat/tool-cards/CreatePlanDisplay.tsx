/**
 * Plan display components.
 *
 * PlanDisplay renders plan file data and supports view/build/refresh.
 * CreatePlanDisplay maps toolItem data into PlanDisplay.
 */

import React, { useState, useMemo, useCallback, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { ClipboardList, Circle, Loader2, CheckCircle, CheckCircle2, PlayCircle, XCircle, ChevronsUpDown, ChevronsDownUp } from 'lucide-react';
import type { ToolCardProps } from '../types/flow-chat';
import { ideControl } from '@/shared/services/ide-control/api';
import { flowChatManager } from '@/flow_chat/services/FlowChatManager';
import { workspaceAPI } from '@/infrastructure/api/service-api/WorkspaceAPI';
import { fileSystemService } from '@/tools/file-system/services/FileSystemService';
import { planBuildStateService } from '@/shared/services/PlanBuildStateService';
import yaml from 'yaml';
import { Tooltip } from '@/component-library';
import { createLogger } from '@/shared/utils/logger';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import { basenamePath, dirnameAbsolutePath } from '@/shared/utils/pathUtils';
import './CreatePlanDisplay.scss';

const log = createLogger('PlanDisplay');

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
  planFilePath: string;
  planContent?: string;
}

// Module-level cache to keep refreshed data after unmount.
// key: cacheKey (toolId or planFilePath), value: PlanData
const planDataCache = new Map<string, PlanData>();

// ==================== PlanDisplay core component ====================

export interface PlanDisplayProps {
  /** Full plan file path. */
  planFilePath: string;
  /** Initial name (optional, first render optimization). */
  initialName?: string;
  /** Initial overview (optional, first render optimization). */
  initialOverview?: string;
  /** Initial todos (optional, first render optimization). */
  initialTodos?: PlanTodo[];
  /** Tool status (used for loading state). */
  status?: 'pending' | 'preparing' | 'streaming' | 'running' | 'completed' | 'cancelled' | 'error' | 'analyzing';
  /** Cache key (defaults to planFilePath). */
  cacheKey?: string;
}

export const PlanDisplay: React.FC<PlanDisplayProps> = ({
  planFilePath,
  initialName = '',
  initialOverview = '',
  initialTodos = [],
  status = 'completed',
  cacheKey,
}) => {
  const { t } = useTranslation('flow-chat');
  const effectiveCacheKey = cacheKey || planFilePath;
  
  const [refreshedData, setRefreshedData] = useState<PlanData | null>(() => {
    return planDataCache.get(effectiveCacheKey) || null;
  });
  
  // Initialize build state from the shared service to survive unmounts.
  const [isBuildStarted, setIsBuildStarted] = useState(() => {
    return planFilePath ? planBuildStateService.isBuildActive(planFilePath) : false;
  });
  
  const [isTodosExpanded, setIsTodosExpanded] = useState(false);
  const toolCardId = cacheKey ?? planFilePath;
  const { cardRootRef, applyExpandedState } = useToolCardHeightContract({
    toolId: toolCardId,
    toolName: 'CreatePlan',
  });

  const hasAutoLoaded = useRef(false);

  // Streaming may provide partial data before planFilePath is available.
  const initialPlanData = useMemo((): PlanData | null => {
    const hasAnyData = planFilePath || initialName || initialOverview || initialTodos.length > 0;
    if (!hasAnyData) return null;
    
    return {
      name: initialName,
      overview: initialOverview,
      todos: initialTodos,
      planFilePath: planFilePath,
      planContent: undefined,
    };
  }, [planFilePath, initialName, initialOverview, initialTodos]);

  const planData = refreshedData || initialPlanData;

  // Subscribe to shared build state service for cross-component sync.
  useEffect(() => {
    if (!planFilePath) return;
    
    // Sync initial state (in case planFilePath just became available).
    setIsBuildStarted(planBuildStateService.isBuildActive(planFilePath));
    
    const unsubscribe = planBuildStateService.subscribe(planFilePath, (event) => {
      setIsBuildStarted(event.isBuilding);
      
      if (event.updatedTodos) {
        const cached = planDataCache.get(effectiveCacheKey);
        const newPlanData: PlanData = {
          name: cached?.name || initialName,
          overview: cached?.overview || initialOverview,
          todos: event.updatedTodos,
          planFilePath: planFilePath,
          planContent: event.planContent || cached?.planContent,
        };
        setRefreshedData(newPlanData);
        planDataCache.set(effectiveCacheKey, newPlanData);
      }
    });
    
    return unsubscribe;
  }, [planFilePath, effectiveCacheKey, initialName, initialOverview]);

  // Load latest content on mount and refresh on file changes.
  useEffect(() => {
    if (!planFilePath) {
      return;
    }

    const normalizedPlanPath = planFilePath.replace(/\\/g, '/');
    const dirPath = dirnameAbsolutePath(planFilePath);

    const loadFromFile = async () => {
      // Skip refresh while writing to avoid feedback loops.
      if (planBuildStateService.isFileWriting(planFilePath)) {
        return;
      }

      try {
        const content = await workspaceAPI.readFileContent(planFilePath);
        
        const frontmatterMatch = content.match(/^---\n([\s\S]*?)\n---/);
        if (frontmatterMatch) {
          const parsed = yaml.parse(frontmatterMatch[1]);
          const planContent = content.replace(/^---\n[\s\S]*?\n---\n*/, '').trim();
          
          const newPlanData: PlanData = {
            name: parsed.name || initialName,
            overview: parsed.overview || initialOverview,
            todos: parsed.todos || initialTodos,
            planFilePath: planFilePath,
            planContent: planContent,
          };
          
          setRefreshedData(newPlanData);
          planDataCache.set(effectiveCacheKey, newPlanData);
        }
      } catch (error) {
        log.warn('Failed to load plan file', { planFilePath, error });
      }
    };

    // Always load once on mount to capture changes during unmount.
    if (!hasAutoLoaded.current) {
      hasAutoLoaded.current = true;
      loadFromFile();
    }

    if (!dirPath) {
      return;
    }

    let debounceTimer: ReturnType<typeof setTimeout> | null = null;

    const unwatch = fileSystemService.watchFileChanges(dirPath, (event) => {
      const eventPath = event.path.replace(/\\/g, '/');
      if (eventPath !== normalizedPlanPath) {
        return;
      }
      
      if (event.type !== 'modified') {
        return;
      }

      // Extra 300ms debounce on the client (server already debounces 500ms).
      if (debounceTimer) {
        clearTimeout(debounceTimer);
      }
      debounceTimer = setTimeout(() => {
        loadFromFile();
      }, 300);
    });

    return () => {
      unwatch();
      if (debounceTimer) {
        clearTimeout(debounceTimer);
      }
    };
  }, [effectiveCacheKey, planFilePath, initialName, initialOverview, initialTodos]);

  // Build button status transitions: build -> building -> built.
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
    if (buildStatus === 'built') {
      if (isBuildStarted) {
        setIsBuildStarted(false);
      }
    }
  }, [buildStatus, isBuildStarted]);

  const planFileName = useMemo(() => {
    return basenamePath(planFilePath);
  }, [planFilePath]);

  const handleViewPlan = useCallback(() => {
    if (planFilePath) {
      ideControl.navigation.goToFile(planFilePath);
    }
  }, [planFilePath]);

  const handleBuild = useCallback(async () => {
    if (!planFilePath || buildStatus !== 'build') return;
    
    try {
      const content = await workspaceAPI.readFileContent(planFilePath);
      
      const frontmatterMatch = content.match(/^---\n([\s\S]*?)\n---/);
      if (!frontmatterMatch) {
        throw new Error('Unable to parse plan file frontmatter');
      }
      
      const parsed = yaml.parse(frontmatterMatch[1]);
      const planContent = content.replace(/^---\n[\s\S]*?\n---\n*/, '').trim();
      
      const latestPlanData: PlanData = {
        name: parsed.name || initialName,
        overview: parsed.overview || initialOverview,
        todos: parsed.todos || initialTodos,
        planFilePath: planFilePath,
        planContent: planContent,
      };
      
      setRefreshedData(latestPlanData);
      planDataCache.set(effectiveCacheKey, latestPlanData);

      // Register build in shared service (notifies all subscribers including PlanViewer).
      const todoIds = latestPlanData.todos.map(t => t.id);
      planBuildStateService.startBuild(planFilePath, todoIds);

      // Send message using the latest data.
      const simpleTodos = latestPlanData.todos.map(t => ({ 
        id: t.id, 
        content: t.content,
        status: t.status
      }));

      const message = `Implement the plan as specified, it is attached for your reference. Do NOT edit the plan file itself. To-do's from the plan have already been created. Do not create them again. Mark them as in_progress as you work, starting with the first one. Don't stop until you have completed all the to-dos.

<attached_file path="${latestPlanData.planFilePath}">
<plan>
${planContent}
</plan>
<todos>
${JSON.stringify(simpleTodos, null, 2)}
</todos>
</attached_file>`;

      const displayMessage = `Build Plan: ${latestPlanData.name}`;
      await flowChatManager.sendMessage(message, undefined, displayMessage, 'agentic', 'agentic');
    } catch (error) {
      log.error('Build failed', { cacheKey: effectiveCacheKey, planFilePath, error });
      planBuildStateService.cancelBuild(planFilePath);
    }
  }, [planFilePath, buildStatus, effectiveCacheKey, initialName, initialOverview, initialTodos]);

  const handleToggleTodos = useCallback(() => {
    applyExpandedState(isTodosExpanded, !isTodosExpanded, setIsTodosExpanded);
  }, [applyExpandedState, isTodosExpanded]);

  const isLoading = status === 'preparing' || status === 'streaming' || status === 'running';

  if (!planData) {
    return (
      <div className={`create-plan-display create-plan-display--loading create-plan-display--loading-shimmer status-${status}`}>
        <div className="create-plan-header create-plan-header--loading-shimmer">
          <span>{t('toolCards.plan.loadingPlan')}</span>
        </div>
      </div>
    );
  }

  return (
    <div
      ref={cardRootRef}
      data-tool-card-id={toolCardId ?? ''}
      className={`create-plan-display status-${status}${isLoading ? ' create-plan-display--plan-generating' : ''}`}
    >
      <Tooltip content={t('toolCards.plan.clickToOpenPlan')}>
        <div 
          className={`create-plan-header create-plan-header--clickable${isLoading ? ' create-plan-header--loading-shimmer' : ''}`}
          onClick={handleViewPlan}
        >
          <div className="header-left">
            <div className="file-icon-wrapper">
              <ClipboardList size={14} />
            </div>
            <span className="file-name">{planFileName}</span>
          </div>
        </div>
      </Tooltip>

      <div className="create-plan-content">
        <div className="plan-content-left">
          <h3 className="plan-title">{planData.name}</h3>
          <p className="plan-overview">{planData.overview}</p>
        </div>
        {planData.todos && planData.todos.length > 0 && (
          <button
            className="todos-toggle-btn"
            type="button"
            onClick={handleToggleTodos}
          >
            {isTodosExpanded ? <ChevronsDownUp size={22} /> : <ChevronsUpDown size={22} />}
          </button>
        )}
      </div>

      {planData.todos && planData.todos.length > 0 && isTodosExpanded && (
        <div className="create-plan-todos create-plan-todos--expanded">
          <div className="todos-list">
            {planData.todos.map((todo, index) => (
              <div
                key={todo.id || index}
                className={`todo-item status-${todo.status || 'pending'}`}
              >
                {todo.status === 'completed' && (
                  <CheckCircle2 size={12} className="todo-icon todo-icon--completed" />
                )}
                {todo.status === 'in_progress' && (
                  <PlayCircle size={12} className="todo-icon todo-icon--in-progress" />
                )}
                {(!todo.status || todo.status === 'pending') && (
                  <Circle size={12} className="todo-icon todo-icon--pending" />
                )}
                {todo.status === 'cancelled' && (
                  <XCircle size={12} className="todo-icon todo-icon--cancelled" />
                )}
                <span className="todo-content">{todo.content}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      <div className={`create-plan-footer${isLoading ? ' create-plan-footer--generating-only' : ''}`}>
        {!isLoading && (
          <button className="view-plan-btn" type="button" onClick={handleViewPlan}>
            {t('toolCards.plan.viewPlan')}
          </button>
        )}
        <button 
          className={`build-btn build-btn--${buildStatus}`}
          onClick={handleBuild}
          disabled={buildStatus !== 'build' || isLoading}
        >
          {buildStatus === 'building' ? (
            <>
              <Loader2 size={14} className="animate-spin" />
              <span>{t('toolCards.plan.building')}</span>
            </>
          ) : buildStatus === 'built' ? (
            <>
              <CheckCircle size={14} />
              <span>{t('toolCards.plan.built')}</span>
            </>
          ) : isLoading ? (
            <>
              <Loader2 size={14} className="animate-spin" />
              <span>{t('toolCards.plan.generating')}</span>
            </>
          ) : (
            <span>{t('toolCards.plan.build')}</span>
          )}
        </button>
      </div>
    </div>
  );
};

// ==================== CreatePlanDisplay tool wrapper ====================

/**
 * Tool wrapper that maps toolItem data into PlanDisplay.
 */
export const CreatePlanDisplay: React.FC<ToolCardProps> = ({
  toolItem,
}) => {
  const { status, toolResult, partialParams, isParamsStreaming, toolCall } = toolItem;
  const toolInput = toolCall?.input as Record<string, unknown> | undefined;
  const useStreamingInputFallback =
    Boolean(isParamsStreaming) ||
    status === 'streaming' ||
    status === 'preparing' ||
    status === 'running';

  const planFilePath = useMemo(() => {
    if (isParamsStreaming && partialParams?.plan_file_path) {
      return String(partialParams.plan_file_path);
    }
    const fromResult = toolResult?.result?.plan_file_path;
    if (fromResult) return String(fromResult);
    if (useStreamingInputFallback && toolInput?.plan_file_path != null) {
      return String(toolInput.plan_file_path);
    }
    return '';
  }, [isParamsStreaming, partialParams, toolResult, useStreamingInputFallback, toolInput]);

  const initialName = useMemo(() => {
    if (isParamsStreaming && partialParams?.name != null) {
      return String(partialParams.name);
    }
    const fromResult = toolResult?.result?.name;
    if (fromResult != null) return String(fromResult);
    if (useStreamingInputFallback && toolInput?.name != null) {
      return String(toolInput.name);
    }
    return '';
  }, [isParamsStreaming, partialParams, toolResult, useStreamingInputFallback, toolInput]);

  const initialOverview = useMemo(() => {
    if (isParamsStreaming && partialParams?.overview != null) {
      return String(partialParams.overview);
    }
    const fromResult = toolResult?.result?.overview;
    if (fromResult != null) return String(fromResult);
    if (useStreamingInputFallback && toolInput?.overview != null) {
      return String(toolInput.overview);
    }
    return '';
  }, [isParamsStreaming, partialParams, toolResult, useStreamingInputFallback, toolInput]);

  const initialTodos = useMemo(() => {
    if (isParamsStreaming && partialParams?.todos && Array.isArray(partialParams.todos)) {
      return partialParams.todos;
    }
    if (toolResult?.result?.todos && Array.isArray(toolResult.result.todos)) {
      return toolResult.result.todos;
    }
    if (useStreamingInputFallback && toolInput?.todos && Array.isArray(toolInput.todos)) {
      return toolInput.todos as PlanTodo[];
    }
    return [];
  }, [isParamsStreaming, partialParams, toolResult, useStreamingInputFallback, toolInput]);

  return (
    <PlanDisplay
      planFilePath={planFilePath}
      initialName={initialName}
      initialOverview={initialOverview}
      initialTodos={initialTodos}
      status={status as PlanDisplayProps['status']}
      cacheKey={toolItem.id}
    />
  );
};
