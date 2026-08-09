import { AssistantEntry, RecentWorkspaceEntry, RemoteSession, WorkspaceInfo } from '../model/RemoteModels';
import { RemoteSessionManager } from './RemoteSessionManager';

export interface RemoteWorkspaceDataSource {
  listRecentWorkspaces(): Promise<RecentWorkspaceEntry[]>;
  setWorkspace(path: string): Promise<WorkspaceInfo>;
  listAssistants(): Promise<AssistantEntry[]>;
  setAssistant(path: string): Promise<WorkspaceInfo>;
  listSessionsForWorkspace(path: string, limit: number): Promise<RemoteSession[]>;
  ping(): Promise<void>;
  reset(): void;
}

/** Repository boundary for workspace, assistant and connection health data. */
export class RemoteWorkspaceRepository implements RemoteWorkspaceDataSource {
  private readonly sessionManager: RemoteSessionManager;

  constructor(sessionManager: RemoteSessionManager) {
    this.sessionManager = sessionManager;
  }

  async listRecentWorkspaces(): Promise<RecentWorkspaceEntry[]> {
    return await this.sessionManager.listRecentWorkspaces();
  }

  async setWorkspace(path: string): Promise<WorkspaceInfo> {
    await this.sessionManager.setWorkspace(path);
    return await this.sessionManager.getWorkspaceInfo();
  }

  async listAssistants(): Promise<AssistantEntry[]> {
    return await this.sessionManager.listAssistants();
  }

  async setAssistant(path: string): Promise<WorkspaceInfo> {
    await this.sessionManager.setAssistant(path);
    return await this.sessionManager.getWorkspaceInfo();
  }

  async listSessionsForWorkspace(path: string, limit: number): Promise<RemoteSession[]> {
    return await this.sessionManager.listSessionsForWorkspace(path, limit);
  }

  async ping(): Promise<void> {
    await this.sessionManager.ping();
  }

  reset(): void {
    this.sessionManager.reset();
  }
}
