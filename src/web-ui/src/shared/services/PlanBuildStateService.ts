/**
 * Shared service managing plan build state across components.
 *
 * Centralizes build-state tracking, TodoWrite → file sync, and subscriber
 * notifications so that PlanDisplay (chat card) and PlanViewer (editor)
 * stay in sync regardless of mount/unmount timing.
 */

import { workspaceAPI } from '@/infrastructure/api/service-api/WorkspaceAPI';
import { createLogger } from '@/shared/utils/logger';
import {
  parsePlanMarkdown,
  serializePlanMarkdown,
  type PlanTodo,
} from '@/shared/plan/planDocument';

const log = createLogger('PlanBuildStateService');

function yamlFrontmatter(content: string): string {
  return content.match(/^---\r?\n([\s\S]*?)\r?\n---/)?.[1].trim() ?? '';
}

export interface PlanBuildStateEvent {
  type: 'build-started' | 'build-completed' | 'build-cancelled' | 'todos-updated';
  /** Whether the plan is still building after this event. */
  isBuilding: boolean;
  /** Updated todo list (present for todos-updated and build-completed). */
  updatedTodos?: PlanTodo[];
  /** Updated YAML frontmatter string (present for todos-updated and build-completed). */
  updatedFrontmatter?: string;
  /** Plan markdown content after frontmatter (present for todos-updated and build-completed). */
  planContent?: string;
}

export type BuildStateCallback = (event: PlanBuildStateEvent) => void;

interface BuildEntry {
  todoIds: Set<string>;
  /** Original file path (preserves platform separators for API calls). */
  planFilePath: string;
  workspacePath: string;
  remoteConnectionId?: string;
  startedAt: number;
}

export interface PlanFileRef {
  planFilePath: string;
  workspacePath?: string;
  remoteConnectionId?: string;
}

export interface StartPlanBuildRequest extends PlanFileRef {
  todoIds: string[];
}

type PlanFileTarget = string | PlanFileRef;

class PlanBuildStateService {
  private static instance: PlanBuildStateService;

  /** Active builds keyed by normalized planFilePath. */
  private buildingPlans = new Map<string, BuildEntry>();

  /** Subscribers keyed by normalized planFilePath. */
  private subscribers = new Map<string, Set<BuildStateCallback>>();

  /** Files currently being written (suppresses watcher reloads). */
  private writingFiles = new Set<string>();

  private constructor() {
    this.setupGlobalListeners();
  }

  static getInstance(): PlanBuildStateService {
    if (!PlanBuildStateService.instance) {
      PlanBuildStateService.instance = new PlanBuildStateService();
    }
    return PlanBuildStateService.instance;
  }

  // ==================== Public API ====================

  /** Mark a plan as building and notify all subscribers. */
  startBuild(request: StartPlanBuildRequest): void {
    const key = this.targetKey(request);
    this.buildingPlans.set(key, {
      todoIds: new Set(request.todoIds),
      planFilePath: request.planFilePath,
      workspacePath: request.workspacePath ?? '',
      remoteConnectionId: request.remoteConnectionId,
      startedAt: Date.now(),
    });
    this.notify(key, { type: 'build-started', isBuilding: true });
  }

  /** Check whether a plan is currently building. */
  isBuildActive(target: PlanFileTarget): boolean {
    return this.buildingPlans.has(this.targetKey(target));
  }

  /** Cancel a build (e.g. on error) and notify subscribers. */
  cancelBuild(target: PlanFileTarget): void {
    const key = this.targetKey(target);
    if (this.buildingPlans.has(key)) {
      this.buildingPlans.delete(key);
      this.notify(key, { type: 'build-cancelled', isBuilding: false });
    }
  }

  /**
   * Subscribe to build-state changes for a plan file.
   * Returns an unsubscribe function.
   */
  subscribe(target: PlanFileTarget, callback: BuildStateCallback): () => void {
    const key = this.targetKey(target);
    if (!this.subscribers.has(key)) {
      this.subscribers.set(key, new Set());
    }
    this.subscribers.get(key)!.add(callback);

    return () => {
      const subs = this.subscribers.get(key);
      if (subs) {
        subs.delete(callback);
        if (subs.size === 0) {
          this.subscribers.delete(key);
        }
      }
    };
  }

  /** Mark a file as being written to suppress watcher reloads. */
  markFileWriting(target: PlanFileTarget): void {
    const key = this.targetKey(target);
    this.writingFiles.add(key);
    setTimeout(() => this.writingFiles.delete(key), 1000);
  }

  /** Check whether a file is currently being written. */
  isFileWriting(target: PlanFileTarget): boolean {
    return this.writingFiles.has(this.targetKey(target));
  }

  // ==================== Internal ====================

  private normalizePath(path: string): string {
    return path.replace(/\\/g, '/');
  }

  private targetKey(target: PlanFileTarget): string {
    if (typeof target === 'string') {
      return `local::${this.normalizePath(target)}`;
    }
    return [
      target.remoteConnectionId ?? 'local',
      this.normalizePath(target.workspacePath ?? ''),
      this.normalizePath(target.planFilePath),
    ].join('::');
  }

  private notify(key: string, event: PlanBuildStateEvent): void {
    const subs = this.subscribers.get(key);
    if (subs) {
      subs.forEach(cb => cb(event));
    }
  }

  private setupGlobalListeners(): void {
    window.addEventListener('bitfun:todowrite-update', this.handleTodoWriteUpdate);
    window.addEventListener('bitfun:dialog-cancelled', this.handleDialogCancelled);
  }

  /**
   * Global handler: when TodoWrite events arrive, update the plan file
   * and notify subscribers with the latest data.
   */
  private handleTodoWriteUpdate = async (event: Event): Promise<void> => {
    const customEvent = event as CustomEvent<{
      sessionId: string;
      turnId: string;
      todos: Array<{ id: string; content: string; status: string }>;
      merge: boolean;
    }>;
    const { todos: incomingTodos } = customEvent.detail;

    if (!incomingTodos.length) return;

    for (const [key, entry] of this.buildingPlans.entries()) {
      const matchedTodos = incomingTodos.filter(t => entry.todoIds.has(t.id));
      if (matchedTodos.length === 0) continue;

      try {
        const content = await workspaceAPI.readFileContent(
          entry.planFilePath,
          undefined,
          entry.remoteConnectionId,
        );
        const parsed = parsePlanMarkdown(content);

        const updatedTodos: PlanTodo[] = parsed.todos.map((todo) => {
          const incoming = incomingTodos.find(t => t.id === todo.id);
          return incoming ? { ...todo, status: incoming.status } : todo;
        });

        const updatedContent = serializePlanMarkdown(parsed, { todos: updatedTodos });
        const updatedDocument = parsePlanMarkdown(updatedContent);

        this.markFileWriting({
          planFilePath: entry.planFilePath,
          workspacePath: entry.workspacePath,
          remoteConnectionId: entry.remoteConnectionId,
        });
        await workspaceAPI.writeFileContent(
          entry.workspacePath,
          entry.planFilePath,
          updatedContent,
          entry.remoteConnectionId,
        );

        const allCompleted = updatedTodos.every(t => t.status === 'completed');

        if (allCompleted) {
          this.buildingPlans.delete(key);
        }

        this.notify(key, {
          type: allCompleted ? 'build-completed' : 'todos-updated',
          isBuilding: !allCompleted,
          updatedTodos,
          updatedFrontmatter: yamlFrontmatter(updatedContent),
          planContent: updatedDocument.planContent,
        });
      } catch (error) {
        log.error('Failed to sync todo status', { filePath: entry.planFilePath, error });
      }
    }
  };

  /** Cancel all active builds when a dialog is cancelled. */
  private handleDialogCancelled = (): void => {
    if (this.buildingPlans.size === 0) return;

    for (const key of Array.from(this.buildingPlans.keys())) {
      this.notify(key, { type: 'build-cancelled', isBuilding: false });
    }
    this.buildingPlans.clear();
  };
}

export const planBuildStateService = PlanBuildStateService.getInstance();
