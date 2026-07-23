/**
 * Shared service managing plan build state across components.
 *
 * Centralizes build-state tracking, TodoWrite → file sync, and subscriber
 * notifications so that CreatePlanDisplay (chat card) and PlanViewer (editor)
 * stay in sync regardless of mount/unmount timing.
 */

import yaml from 'yaml';
import { workspaceAPI } from '@/infrastructure/api/service-api/WorkspaceAPI';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('PlanBuildStateService');

export interface PlanTodo {
  id: string;
  content: string;
  status?: string;
  dependencies?: string[];
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
  startedAt: number;
}

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
  startBuild(planFilePath: string, todoIds: string[]): void {
    const key = this.normalizePath(planFilePath);
    this.buildingPlans.set(key, {
      todoIds: new Set(todoIds),
      planFilePath,
      startedAt: Date.now(),
    });
    this.notify(key, { type: 'build-started', isBuilding: true });
  }

  /** Check whether a plan is currently building. */
  isBuildActive(planFilePath: string): boolean {
    return this.buildingPlans.has(this.normalizePath(planFilePath));
  }

  /** Cancel a build (e.g. on error) and notify subscribers. */
  cancelBuild(planFilePath: string): void {
    const key = this.normalizePath(planFilePath);
    if (this.buildingPlans.has(key)) {
      this.buildingPlans.delete(key);
      this.notify(key, { type: 'build-cancelled', isBuilding: false });
    }
  }

  /**
   * Subscribe to build-state changes for a plan file.
   * Returns an unsubscribe function.
   */
  subscribe(planFilePath: string, callback: BuildStateCallback): () => void {
    const key = this.normalizePath(planFilePath);
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
  markFileWriting(filePath: string): void {
    const key = this.normalizePath(filePath);
    this.writingFiles.add(key);
    setTimeout(() => this.writingFiles.delete(key), 1000);
  }

  /** Check whether a file is currently being written. */
  isFileWriting(filePath: string): boolean {
    return this.writingFiles.has(this.normalizePath(filePath));
  }

  // ==================== Internal ====================

  private normalizePath(path: string): string {
    return path.replace(/\\/g, '/');
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
        const content = await workspaceAPI.readFileContent(entry.planFilePath);

        const frontmatterMatch = content.match(/^---\n([\s\S]*?)\n---/);
        if (!frontmatterMatch) {
          log.warn('Failed to parse plan file frontmatter', { filePath: entry.planFilePath });
          continue;
        }

        const parsed = yaml.parse(frontmatterMatch[1]);
        const planContent = content.replace(/^---\n[\s\S]*?\n---\n*/, '').trim();

        const updatedTodos: PlanTodo[] = (parsed.todos || []).map((todo: PlanTodo) => {
          const incoming = incomingTodos.find(t => t.id === todo.id);
          return incoming ? { ...todo, status: incoming.status } : todo;
        });

        const updatedFrontmatter = yaml.stringify({ ...parsed, todos: updatedTodos });
        const updatedContent = `---\n${updatedFrontmatter}---\n\n${planContent}`;

        this.markFileWriting(entry.planFilePath);
        await workspaceAPI.writeFileContent('', entry.planFilePath, updatedContent);

        const allCompleted = updatedTodos.every(t => t.status === 'completed');

        if (allCompleted) {
          this.buildingPlans.delete(key);
        }

        this.notify(key, {
          type: allCompleted ? 'build-completed' : 'todos-updated',
          isBuilding: !allCompleted,
          updatedTodos,
          updatedFrontmatter: updatedFrontmatter.trim(),
          planContent,
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
