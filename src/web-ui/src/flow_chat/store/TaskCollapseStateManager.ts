/**
 * Task tool collapse state manager for sharing collapse state across components
 */

type StateChangeListener = (toolId: string, isCollapsed: boolean) => void;

class TaskCollapseStateManager {
  private static instance: TaskCollapseStateManager;
  private collapseStates: Map<string, boolean> = new Map();
  private listeners: Set<StateChangeListener> = new Set();

  private constructor() {}

  public static getInstance(): TaskCollapseStateManager {
    if (!TaskCollapseStateManager.instance) {
      TaskCollapseStateManager.instance = new TaskCollapseStateManager();
    }
    return TaskCollapseStateManager.instance;
  }

  public setCollapsed(toolId: string, isCollapsed: boolean): void {
    this.collapseStates.set(toolId, isCollapsed);
    this.listeners.forEach(listener => {
      listener(toolId, isCollapsed);
    });
  }

  public isCollapsed(toolId: string): boolean {
    return this.collapseStates.get(toolId) ?? false;
  }

  /**
   * Returns undefined when no record exists (distinguishes "never set" from "explicitly expanded")
   */
  public getCollapsedOrUndefined(toolId: string): boolean | undefined {
    return this.collapseStates.get(toolId);
  }

  public toggle(toolId: string): boolean {
    const newState = !this.isCollapsed(toolId);
    this.setCollapsed(toolId, newState);
    return newState;
  }

  public addListener(listener: StateChangeListener): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  public clear(toolId: string): void {
    this.collapseStates.delete(toolId);
  }

  public clearAll(): void {
    this.collapseStates.clear();
  }
}

export const taskCollapseStateManager = TaskCollapseStateManager.getInstance();

