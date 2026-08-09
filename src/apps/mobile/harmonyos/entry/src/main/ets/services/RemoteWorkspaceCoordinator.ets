import { AssistantEntry, RecentWorkspaceEntry, RemoteSession, WorkspaceInfo } from '../model/RemoteModels';
import { RemoteWorkspaceDataSource } from './RemoteWorkspaceRepository';
import { RemoteLogger } from './RemoteLogger';

/** Coordinates workspace/assistant selection and cross-workspace session discovery. */
export class RemoteWorkspaceCoordinator {
  private readonly repository: RemoteWorkspaceDataSource;

  constructor(repository: RemoteWorkspaceDataSource) {
    this.repository = repository;
  }

  async recentWorkspaces(): Promise<RecentWorkspaceEntry[]> {
    return await this.repository.listRecentWorkspaces();
  }

  async assistants(): Promise<AssistantEntry[]> {
    return await this.repository.listAssistants();
  }

  async switchWorkspace(path: string): Promise<WorkspaceInfo> {
    return await this.repository.setWorkspace(path);
  }

  async switchAssistant(path: string): Promise<WorkspaceInfo> {
    return await this.repository.setAssistant(path);
  }

  async sessionsForWorkspaces(paths: string[]): Promise<RemoteSession[]> {
    const all: RemoteSession[] = [];
    for (const path of paths) {
      try {
        const sessions = await this.repository.listSessionsForWorkspace(path, 50);
        sessions.forEach((item: RemoteSession) => {
          if (!all.some((existing: RemoteSession) => existing.id === item.id)) {
            all.push(item);
          }
        });
      } catch (err) {
        RemoteLogger.warn(`load workspace sessions failed path=${path}: ${String(err)}`);
      }
    }
    return all;
  }
}
