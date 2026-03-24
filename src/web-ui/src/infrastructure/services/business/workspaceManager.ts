 

import { WorkspaceInfo, WorkspaceKind, globalStateAPI } from '../../../shared/types';
import { normalizeRemoteWorkspacePath } from '@/shared/utils/pathUtils';
import { createLogger } from '@/shared/utils/logger';
import { listen } from '@tauri-apps/api/event';

const log = createLogger('WorkspaceManager');

interface WorkspaceIdentityChangedEvent {
  workspaceId: string;
  workspacePath: string;
  name: string;
  identity: WorkspaceInfo['identity'];
  changedFields: string[];
}

export type WorkspaceEvent =
  | { type: 'workspace:opened'; workspace: WorkspaceInfo }
  | { type: 'workspace:closed'; workspaceId: string }
  | { type: 'workspace:removed'; workspaceId: string }
  | { type: 'workspace:switched'; workspace: WorkspaceInfo }
  | { type: 'workspace:active-changed'; workspace: WorkspaceInfo | null }
  | { type: 'workspace:updated'; workspace: WorkspaceInfo }
  | { type: 'workspace:loading'; loading: boolean }
  | { type: 'workspace:error'; error: string | null };

export type WorkspaceEventListener = (event: WorkspaceEvent) => void;

export interface WorkspaceState {
  currentWorkspace: WorkspaceInfo | null;
  openedWorkspaces: Map<string, WorkspaceInfo>;
  activeWorkspaceId: string | null;
  lastUsedWorkspaceId: string | null;
  recentWorkspaces: WorkspaceInfo[];
  loading: boolean;
  error: string | null;
}

export type WorkspaceSection = 'assistants' | 'projects';
export type WorkspaceReorderPosition = 'before' | 'after';

class WorkspaceManager {
  private static instance: WorkspaceManager | null = null;
  private state: WorkspaceState;
  private listeners: Set<WorkspaceEventListener> = new Set();
  private isInitialized = false;
  private isInitializing = false;
  private identityEventListening = false;

  private constructor() {
    this.state = {
      currentWorkspace: null,
      openedWorkspaces: new Map(),
      activeWorkspaceId: null,
      lastUsedWorkspaceId: null,
      recentWorkspaces: [],
      loading: true,
      error: null,
    };
  }

  public static getInstance(): WorkspaceManager {
    if (!WorkspaceManager.instance) {
      WorkspaceManager.instance = new WorkspaceManager();
    }
    return WorkspaceManager.instance;
  }

  public getState(): WorkspaceState {
    return {
      ...this.state,
      openedWorkspaces: new Map(this.state.openedWorkspaces),
    };
  }

  public addEventListener(listener: WorkspaceEventListener): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  private emit(event: WorkspaceEvent): void {
    log.debug('Emitting event', { type: event.type });
    this.listeners.forEach(listener => {
      try {
        listener(event);
      } catch (error) {
        log.error('Event listener execution error', { eventType: event.type, error });
      }
    });
  }

  private updateState(updates: Partial<WorkspaceState>, event?: WorkspaceEvent): void {
    this.state = {
      ...this.state,
      ...updates,
      openedWorkspaces: updates.openedWorkspaces
        ? new Map(updates.openedWorkspaces)
        : this.state.openedWorkspaces,
    };

    log.debug('State updated', {
      activeWorkspaceId: this.state.activeWorkspaceId,
      openedWorkspaceCount: this.state.openedWorkspaces.size,
    });

    if (event) {
      this.emit(event);
    }
  }

  private setLoading(loading: boolean): void {
    this.updateState({ loading }, { type: 'workspace:loading', loading });
  }

  private setError(error: string | null): void {
    this.updateState({ error }, { type: 'workspace:error', error });
  }

  private buildOpenedWorkspaceMap(workspaces: WorkspaceInfo[]): Map<string, WorkspaceInfo> {
    return new Map(workspaces.map(workspace => [workspace.id, workspace]));
  }

  private getOpenedWorkspacesList(): WorkspaceInfo[] {
    return Array.from(this.state.openedWorkspaces.values());
  }

  private isWorkspaceInSection(workspace: WorkspaceInfo, section: WorkspaceSection): boolean {
    return section === 'assistants'
      ? workspace.workspaceKind === 'assistant'
      : workspace.workspaceKind !== 'assistant';
  }

  private preserveOpenedWorkspaceOrder(workspaces: WorkspaceInfo[]): WorkspaceInfo[] {
    const currentOrder = Array.from(this.state.openedWorkspaces.keys());
    const nextWorkspaceMap = this.buildOpenedWorkspaceMap(workspaces);
    const orderedWorkspaces = currentOrder
      .map(workspaceId => nextWorkspaceMap.get(workspaceId))
      .filter((workspace): workspace is WorkspaceInfo => Boolean(workspace));

    const existingIds = new Set(orderedWorkspaces.map(workspace => workspace.id));

    for (const workspace of workspaces) {
      if (!existingIds.has(workspace.id)) {
        orderedWorkspaces.push(workspace);
      }
    }

    return orderedWorkspaces;
  }

  private buildReorderedOpenedWorkspaceIds(
    section: WorkspaceSection,
    sourceWorkspaceId: string,
    targetWorkspaceId: string,
    position: WorkspaceReorderPosition
  ): string[] | null {
    if (sourceWorkspaceId === targetWorkspaceId) {
      return null;
    }

    const openedWorkspaces = this.getOpenedWorkspacesList();
    const workspaceMap = this.buildOpenedWorkspaceMap(openedWorkspaces);
    const sourceWorkspace = workspaceMap.get(sourceWorkspaceId);
    const targetWorkspace = workspaceMap.get(targetWorkspaceId);

    if (!sourceWorkspace || !targetWorkspace) {
      return null;
    }

    if (
      !this.isWorkspaceInSection(sourceWorkspace, section) ||
      !this.isWorkspaceInSection(targetWorkspace, section)
    ) {
      return null;
    }

    const sectionWorkspaceIds = openedWorkspaces
      .filter(workspace => this.isWorkspaceInSection(workspace, section))
      .map(workspace => workspace.id);
    const sourceIndex = sectionWorkspaceIds.indexOf(sourceWorkspaceId);
    const targetIndex = sectionWorkspaceIds.indexOf(targetWorkspaceId);

    if (sourceIndex === -1 || targetIndex === -1) {
      return null;
    }

    const reorderedSectionWorkspaceIds = [...sectionWorkspaceIds];
    reorderedSectionWorkspaceIds.splice(sourceIndex, 1);

    const insertionBaseIndex = sourceIndex < targetIndex ? targetIndex - 1 : targetIndex;
    const insertionIndex = position === 'after'
      ? insertionBaseIndex + 1
      : insertionBaseIndex;

    reorderedSectionWorkspaceIds.splice(insertionIndex, 0, sourceWorkspaceId);

    const reorderedOpenedWorkspaceIds: string[] = [];
    let sectionCursor = 0;

    for (const workspace of openedWorkspaces) {
      if (this.isWorkspaceInSection(workspace, section)) {
        reorderedOpenedWorkspaceIds.push(reorderedSectionWorkspaceIds[sectionCursor]);
        sectionCursor += 1;
      } else {
        reorderedOpenedWorkspaceIds.push(workspace.id);
      }
    }

    return reorderedOpenedWorkspaceIds;
  }

  private resolveLastUsedWorkspaceId(
    currentWorkspace: WorkspaceInfo | null,
    recentWorkspaces: WorkspaceInfo[],
    openedWorkspaces: Map<string, WorkspaceInfo>
  ): string | null {
    return (
      currentWorkspace?.id ||
      recentWorkspaces[0]?.id ||
      openedWorkspaces.keys().next().value ||
      null
    );
  }

  private updateWorkspaceState(
    currentWorkspace: WorkspaceInfo | null,
    recentWorkspaces: WorkspaceInfo[],
    openedWorkspaces: WorkspaceInfo[],
    loading: boolean,
    error: string | null,
    event?: WorkspaceEvent
  ): void {
    const openedWorkspaceMap = this.buildOpenedWorkspaceMap(openedWorkspaces);

    const resolvedCurrentWorkspace = currentWorkspace
      ? openedWorkspaceMap.get(currentWorkspace.id) ?? currentWorkspace
      : null;

    this.updateState(
      {
        currentWorkspace: resolvedCurrentWorkspace,
        openedWorkspaces: openedWorkspaceMap,
        activeWorkspaceId: resolvedCurrentWorkspace?.id ?? null,
        lastUsedWorkspaceId: this.resolveLastUsedWorkspaceId(
          resolvedCurrentWorkspace,
          recentWorkspaces,
          openedWorkspaceMap
        ),
        recentWorkspaces,
        loading,
        error,
      },
      event
    );
  }

  private async ensureIdentityChangeListener(): Promise<void> {
    if (this.identityEventListening) {
      return;
    }

    this.identityEventListening = true;

    try {
      await listen<WorkspaceIdentityChangedEvent>('workspace-identity-changed', event => {
        this.applyIdentityUpdate(event.payload);
      });
    } catch (error) {
      this.identityEventListening = false;
      log.error('Failed to subscribe workspace identity updates', { error });
    }
  }

  private applyIdentityUpdate(update: WorkspaceIdentityChangedEvent): void {
    const updateWorkspace = (workspace: WorkspaceInfo | null): WorkspaceInfo | null => {
      if (!workspace) {
        return null;
      }

      const matches =
        workspace.id === update.workspaceId ||
        workspace.rootPath === update.workspacePath;

      if (!matches) {
        return workspace;
      }

      return {
        ...workspace,
        name: update.name,
        identity: update.identity ?? null,
      };
    };

    const currentWorkspace = updateWorkspace(this.state.currentWorkspace);
    const openedWorkspaces = new Map(
      Array.from(this.state.openedWorkspaces.entries()).map(([id, workspace]) => [
        id,
        updateWorkspace(workspace) ?? workspace,
      ])
    );
    const recentWorkspaces = this.state.recentWorkspaces.map(
      workspace => updateWorkspace(workspace) ?? workspace
    );

    const updatedWorkspace =
      currentWorkspace?.id === update.workspaceId
        ? currentWorkspace
        : openedWorkspaces.get(update.workspaceId) ||
          recentWorkspaces.find(workspace => workspace.id === update.workspaceId) ||
          null;

    if (!updatedWorkspace) {
      return;
    }

    this.updateState(
      {
        currentWorkspace,
        openedWorkspaces,
        recentWorkspaces,
      },
      { type: 'workspace:updated', workspace: updatedWorkspace }
    );
  }

  public async initialize(): Promise<void> {
    if (this.isInitialized || this.isInitializing) {
      return;
    }

    try {
      this.isInitializing = true;
      log.info('Initializing workspace state');
      await this.ensureIdentityChangeListener();

      const initResult = await globalStateAPI.initializeGlobalState();
      log.debug('Backend initialization completed', { result: initResult });
      await globalStateAPI.cleanupInvalidWorkspaces();

      const [recentWorkspaces, openedWorkspaces, currentWorkspace] = await Promise.all([
        globalStateAPI.getRecentWorkspaces(),
        globalStateAPI.getOpenedWorkspaces(),
        globalStateAPI.getCurrentWorkspace(),
      ]);

      this.updateWorkspaceState(
        currentWorkspace,
        recentWorkspaces,
        openedWorkspaces,
        false,
        null,
        currentWorkspace
          ? { type: 'workspace:opened', workspace: currentWorkspace }
          : undefined
      );

      this.emit({ type: 'workspace:loading', loading: false });
      this.isInitialized = true;
      log.info('Workspace state initialization completed', {
        activeWorkspaceId: currentWorkspace?.id ?? null,
        openedWorkspaceCount: openedWorkspaces.length,
      });
    } catch (error) {
      log.error('Failed to initialize workspace state', { error });
      const errorMessage = error instanceof Error ? error.message : String(error);
      this.updateWorkspaceState(null, [], [], false, errorMessage);
      this.emit({ type: 'workspace:error', error: errorMessage });
    } finally {
      this.isInitializing = false;
    }
  }

  public async openWorkspace(path: string): Promise<WorkspaceInfo> {
    try {
      this.setLoading(true);
      this.setError(null);

      log.info('Opening workspace', { path });

      const workspace = await globalStateAPI.openWorkspace(path);
      const [recentWorkspaces, openedWorkspaces] = await Promise.all([
        globalStateAPI.getRecentWorkspaces(),
        globalStateAPI.getOpenedWorkspaces(),
      ]);

      this.updateWorkspaceState(
        workspace,
        recentWorkspaces,
        openedWorkspaces,
        false,
        null,
        { type: 'workspace:opened', workspace }
      );

      return workspace;
    } catch (error) {
      log.error('Failed to open workspace', { path, error });
      const errorMessage = error instanceof Error ? error.message : String(error);
      this.updateState({ loading: false, error: errorMessage }, { type: 'workspace:error', error: errorMessage });
      throw error;
    }
  }

  public async openRemoteWorkspace(remoteWorkspace: {
    connectionId: string;
    connectionName: string;
    remotePath: string;
  }): Promise<WorkspaceInfo> {
    try {
      this.setLoading(true);
      this.setError(null);

      log.info('Opening remote workspace', remoteWorkspace);

      const remotePath = normalizeRemoteWorkspacePath(remoteWorkspace.remotePath);

      const workspace = await globalStateAPI.openRemoteWorkspace(
        remotePath,
        remoteWorkspace.connectionId,
        remoteWorkspace.connectionName,
      );

      const [recentWorkspaces, openedWorkspaces] = await Promise.all([
        globalStateAPI.getRecentWorkspaces(),
        globalStateAPI.getOpenedWorkspaces(),
      ]);

      this.updateWorkspaceState(
        workspace,
        recentWorkspaces,
        openedWorkspaces,
        false,
        null,
        { type: 'workspace:opened', workspace }
      );

      return workspace;
    } catch (error) {
      log.error('Failed to open remote workspace', { remoteWorkspace, error });
      const errorMessage = error instanceof Error ? error.message : String(error);
      this.updateState({ loading: false, error: errorMessage }, { type: 'workspace:error', error: errorMessage });
      throw error;
    }
  }

  public async removeRemoteWorkspace(connectionId: string): Promise<void> {
    try {
      const workspace = this.findRemoteWorkspaceByConnectionId(connectionId);
      if (!workspace) {
        return;
      }

      await globalStateAPI.closeWorkspace(workspace.id);

      const [currentWorkspace, recentWorkspaces, openedWorkspaces] = await Promise.all([
        globalStateAPI.getCurrentWorkspace(),
        globalStateAPI.getRecentWorkspaces(),
        globalStateAPI.getOpenedWorkspaces(),
      ]);

      this.updateWorkspaceState(
        currentWorkspace,
        recentWorkspaces,
        openedWorkspaces,
        false,
        null,
        { type: 'workspace:closed', workspaceId: workspace.id }
      );

      this.emit({ type: 'workspace:active-changed', workspace: currentWorkspace });
    } catch (error) {
      log.error('Failed to remove remote workspace', { connectionId, error });
      const errorMessage = error instanceof Error ? error.message : String(error);
      this.updateState({ error: errorMessage }, { type: 'workspace:error', error: errorMessage });
      throw error;
    }
  }

  private findRemoteWorkspaceByConnectionId(connectionId: string): WorkspaceInfo | undefined {
    for (const [, ws] of this.state.openedWorkspaces) {
      if (ws.connectionId === connectionId && ws.workspaceKind === WorkspaceKind.Remote) {
        return ws;
      }
    }
    return undefined;
  }

  public async createAssistantWorkspace(): Promise<WorkspaceInfo> {
    try {
      this.setLoading(true);
      this.setError(null);

      const workspace = await globalStateAPI.createAssistantWorkspace();
      const [currentWorkspace, recentWorkspaces, openedWorkspaces] = await Promise.all([
        globalStateAPI.getCurrentWorkspace(),
        globalStateAPI.getRecentWorkspaces(),
        globalStateAPI.getOpenedWorkspaces(),
      ]);

      this.updateWorkspaceState(
        currentWorkspace,
        recentWorkspaces,
        openedWorkspaces,
        false,
        null,
        { type: 'workspace:opened', workspace }
      );

      return workspace;
    } catch (error) {
      log.error('Failed to create assistant workspace', { error });
      const errorMessage = error instanceof Error ? error.message : String(error);
      this.updateState({ loading: false, error: errorMessage }, { type: 'workspace:error', error: errorMessage });
      throw error;
    }
  }

  public async closeWorkspace(): Promise<void> {
    if (!this.state.currentWorkspace?.id) {
      return;
    }

    await this.closeWorkspaceById(this.state.currentWorkspace.id);
  }

  public async closeWorkspaceById(workspaceId: string): Promise<void> {
    try {
      this.setLoading(true);
      this.setError(null);

      log.info('Closing workspace', { workspaceId });

      await globalStateAPI.closeWorkspace(workspaceId);

      const [currentWorkspace, recentWorkspaces, openedWorkspaces] = await Promise.all([
        globalStateAPI.getCurrentWorkspace(),
        globalStateAPI.getRecentWorkspaces(),
        globalStateAPI.getOpenedWorkspaces(),
      ]);

      this.updateWorkspaceState(
        currentWorkspace,
        recentWorkspaces,
        openedWorkspaces,
        false,
        null,
        { type: 'workspace:closed', workspaceId }
      );

      this.emit({ type: 'workspace:active-changed', workspace: currentWorkspace });
    } catch (error) {
      log.error('Failed to close workspace', { workspaceId, error });
      const errorMessage = error instanceof Error ? error.message : String(error);
      this.updateState({ loading: false, error: errorMessage }, { type: 'workspace:error', error: errorMessage });
      throw error;
    }
  }

  public async deleteAssistantWorkspace(workspaceId: string): Promise<void> {
    try {
      this.setLoading(true);
      this.setError(null);

      log.info('Deleting assistant workspace', { workspaceId });

      await globalStateAPI.deleteAssistantWorkspace(workspaceId);

      const [currentWorkspace, recentWorkspaces, openedWorkspaces] = await Promise.all([
        globalStateAPI.getCurrentWorkspace(),
        globalStateAPI.getRecentWorkspaces(),
        globalStateAPI.getOpenedWorkspaces(),
      ]);

      this.updateWorkspaceState(
        currentWorkspace,
        recentWorkspaces,
        openedWorkspaces,
        false,
        null,
        { type: 'workspace:removed', workspaceId }
      );

      this.emit({ type: 'workspace:active-changed', workspace: currentWorkspace });
    } catch (error) {
      log.error('Failed to delete assistant workspace', { workspaceId, error });
      const errorMessage = error instanceof Error ? error.message : String(error);
      this.updateState({ loading: false, error: errorMessage }, { type: 'workspace:error', error: errorMessage });
      throw error;
    }
  }

  public async resetAssistantWorkspace(workspaceId: string): Promise<WorkspaceInfo> {
    try {
      this.setLoading(true);
      this.setError(null);

      log.info('Resetting assistant workspace', { workspaceId });

      const workspace = await globalStateAPI.resetAssistantWorkspace(workspaceId);

      const [currentWorkspace, recentWorkspaces, openedWorkspaces] = await Promise.all([
        globalStateAPI.getCurrentWorkspace(),
        globalStateAPI.getRecentWorkspaces(),
        globalStateAPI.getOpenedWorkspaces(),
      ]);

      this.updateWorkspaceState(
        currentWorkspace,
        recentWorkspaces,
        openedWorkspaces,
        false,
        null,
        { type: 'workspace:updated', workspace }
      );

      this.emit({ type: 'workspace:active-changed', workspace: currentWorkspace });
      return workspace;
    } catch (error) {
      log.error('Failed to reset assistant workspace', { workspaceId, error });
      const errorMessage = error instanceof Error ? error.message : String(error);
      this.updateState({ loading: false, error: errorMessage }, { type: 'workspace:error', error: errorMessage });
      throw error;
    }
  }

  public async setActiveWorkspace(workspaceId: string): Promise<WorkspaceInfo> {
    try {
      if (this.state.activeWorkspaceId === workspaceId) {
        const currentWorkspace = this.state.currentWorkspace;
        if (!currentWorkspace) {
          throw new Error(`Active workspace not found: ${workspaceId}`);
        }
        return currentWorkspace;
      }

      this.setLoading(true);
      this.setError(null);

      const workspace = await globalStateAPI.setActiveWorkspace(workspaceId);
      const [recentWorkspaces, openedWorkspaces] = await Promise.all([
        globalStateAPI.getRecentWorkspaces(),
        globalStateAPI.getOpenedWorkspaces(),
      ]);
      const orderedOpenedWorkspaces = this.preserveOpenedWorkspaceOrder(openedWorkspaces);

      this.updateWorkspaceState(
        workspace,
        recentWorkspaces,
        orderedOpenedWorkspaces,
        false,
        null,
        { type: 'workspace:switched', workspace }
      );

      this.emit({ type: 'workspace:active-changed', workspace });
      return workspace;
    } catch (error) {
      log.error('Failed to set active workspace', { workspaceId, error });
      const errorMessage = error instanceof Error ? error.message : String(error);
      this.updateState({ loading: false, error: errorMessage }, { type: 'workspace:error', error: errorMessage });
      throw error;
    }
  }

  public async reorderOpenedWorkspacesInSection(
    section: WorkspaceSection,
    sourceWorkspaceId: string,
    targetWorkspaceId: string,
    position: WorkspaceReorderPosition
  ): Promise<void> {
    const previousCurrentWorkspace = this.state.currentWorkspace;
    const previousRecentWorkspaces = this.state.recentWorkspaces;
    const previousOpenedWorkspaces = this.getOpenedWorkspacesList();
    const reorderedOpenedWorkspaceIds = this.buildReorderedOpenedWorkspaceIds(
      section,
      sourceWorkspaceId,
      targetWorkspaceId,
      position
    );

    if (!reorderedOpenedWorkspaceIds) {
      return;
    }

    const currentOpenedWorkspaceIds = previousOpenedWorkspaces.map(workspace => workspace.id);
    const hasOrderChanged = reorderedOpenedWorkspaceIds.some(
      (workspaceId, index) => workspaceId !== currentOpenedWorkspaceIds[index]
    );

    if (!hasOrderChanged) {
      return;
    }

    const workspaceMap = this.buildOpenedWorkspaceMap(previousOpenedWorkspaces);
    const reorderedOpenedWorkspaces = reorderedOpenedWorkspaceIds
      .map(workspaceId => workspaceMap.get(workspaceId))
      .filter((workspace): workspace is WorkspaceInfo => Boolean(workspace));
    const reorderedEventWorkspace = previousCurrentWorkspace
      ?? workspaceMap.get(sourceWorkspaceId)
      ?? reorderedOpenedWorkspaces[0]
      ?? previousOpenedWorkspaces[0];

    if (!reorderedEventWorkspace) {
      return;
    }

    this.updateWorkspaceState(
      previousCurrentWorkspace,
      previousRecentWorkspaces,
      reorderedOpenedWorkspaces,
      false,
      null,
      { type: 'workspace:updated', workspace: reorderedEventWorkspace }
    );

    try {
      await globalStateAPI.reorderOpenedWorkspaces(reorderedOpenedWorkspaceIds);
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      const rollbackEventWorkspace = previousCurrentWorkspace
        ?? workspaceMap.get(sourceWorkspaceId)
        ?? previousOpenedWorkspaces[0]
        ?? reorderedEventWorkspace;
      this.updateWorkspaceState(
        previousCurrentWorkspace,
        previousRecentWorkspaces,
        previousOpenedWorkspaces,
        false,
        errorMessage,
        { type: 'workspace:updated', workspace: rollbackEventWorkspace }
      );
      this.emit({ type: 'workspace:error', error: errorMessage });
      throw error;
    }
  }

  public async switchWorkspace(workspace: WorkspaceInfo): Promise<WorkspaceInfo> {
    if (this.state.currentWorkspace?.id === workspace.id) {
      return workspace;
    }

    if (this.state.openedWorkspaces.has(workspace.id)) {
      return this.setActiveWorkspace(workspace.id);
    }

    return this.openWorkspace(workspace.rootPath);
  }

  public async scanWorkspaceInfo(): Promise<WorkspaceInfo | null> {
    try {
      if (!this.state.currentWorkspace?.rootPath) {
        throw new Error('No current workspace available for scanning');
      }

      this.setLoading(true);
      this.setError(null);

      const updatedWorkspace = await globalStateAPI.scanWorkspaceInfo(this.state.currentWorkspace.rootPath);

      if (updatedWorkspace) {
        const openedWorkspaces = new Map(this.state.openedWorkspaces);
        openedWorkspaces.set(updatedWorkspace.id, updatedWorkspace);

        const recentWorkspaces = this.state.recentWorkspaces.map(workspace =>
          workspace.id === updatedWorkspace.id ? updatedWorkspace : workspace
        );

        this.updateState(
          {
            currentWorkspace: updatedWorkspace,
            openedWorkspaces,
            recentWorkspaces,
            activeWorkspaceId: updatedWorkspace.id,
            loading: false,
            error: null,
          },
          { type: 'workspace:updated', workspace: updatedWorkspace }
        );
      } else {
        this.setLoading(false);
      }

      return updatedWorkspace;
    } catch (error) {
      log.error('Failed to scan workspace info', { error });
      const errorMessage = error instanceof Error ? error.message : String(error);
      this.setError(errorMessage);
      this.setLoading(false);
      throw error;
    }
  }

  public async refreshRecentWorkspaces(): Promise<void> {
    try {
      const recentWorkspaces = await globalStateAPI.getRecentWorkspaces();
      this.updateState({ recentWorkspaces });
      log.debug('Recent workspaces refreshed', { count: recentWorkspaces.length });
    } catch (error) {
      log.error('Failed to refresh recent workspaces', { error });
    }
  }

  public async cleanupInvalidWorkspaces(): Promise<number> {
    try {
      const removedCount = await globalStateAPI.cleanupInvalidWorkspaces();

      if (removedCount === 0) {
        return 0;
      }

      const [currentWorkspace, recentWorkspaces, openedWorkspaces] = await Promise.all([
        globalStateAPI.getCurrentWorkspace(),
        globalStateAPI.getRecentWorkspaces(),
        globalStateAPI.getOpenedWorkspaces(),
      ]);

      this.updateWorkspaceState(
        currentWorkspace,
        recentWorkspaces,
        openedWorkspaces,
        false,
        null
      );
      this.emit({ type: 'workspace:active-changed', workspace: currentWorkspace });

      log.info('Invalid workspaces cleaned up', { removedCount });
      return removedCount;
    } catch (error) {
      log.error('Failed to cleanup invalid workspaces', { error });
      throw error;
    }
  }

  public hasWorkspace(): boolean {
    return !!this.state.currentWorkspace;
  }

  public getWorkspaceName(): string {
    return this.state.currentWorkspace?.name || '';
  }

  public getWorkspacePath(): string {
    return this.state.currentWorkspace?.rootPath || '';
  }
}

export const workspaceManager = WorkspaceManager.getInstance();

export { WorkspaceManager };


