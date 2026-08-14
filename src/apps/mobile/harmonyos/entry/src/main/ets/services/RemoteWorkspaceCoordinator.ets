import { AssistantEntry, RecentWorkspaceEntry, RemoteSession, WorkspaceInfo } from '../model/RemoteModels';
import { RemoteWorkspaceDataSource } from './RemoteWorkspaceRepository';
import { RemoteLogger } from './RemoteLogger';

/**
 * How many workspaces are listed at once during cross-workspace discovery.
 *
 * Each listing is one round trip to the desktop over the relay, and a phone
 * that has been used for a while has twenty-odd recent workspaces: done one
 * after another that is five seconds of the session list rearranging itself
 * behind the user. Kept small because these run alongside the polling that
 * drives an open conversation, and a burst wide enough to crowd it out would
 * trade a slow list for a slow chat.
 */
const CONCURRENT_WORKSPACE_LISTINGS: number = 4;

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

  /**
   * Every session the given workspaces know about, first occurrence winning.
   *
   * Listings run a batch at a time rather than all at once or one at a time,
   * and results are merged in the order the paths were given, so widening the
   * batch changes how long this takes and nothing about what it returns.
   */
  async sessionsForWorkspaces(paths: string[]): Promise<RemoteSession[]> {
    const all: RemoteSession[] = [];
    for (let start = 0; start < paths.length; start += CONCURRENT_WORKSPACE_LISTINGS) {
      const batch: string[] = paths.slice(start, start + CONCURRENT_WORKSPACE_LISTINGS);
      const listings: RemoteSession[][] = await Promise.all(
        batch.map((path: string): Promise<RemoteSession[]> => this.sessionsForWorkspace(path))
      );
      listings.forEach((sessions: RemoteSession[]) => {
        sessions.forEach((item: RemoteSession) => {
          if (!all.some((existing: RemoteSession) => existing.id === item.id)) {
            all.push(item);
          }
        });
      });
    }
    return all;
  }

  /** One workspace's sessions, or none when that workspace cannot be read. */
  private async sessionsForWorkspace(path: string): Promise<RemoteSession[]> {
    try {
      return await this.repository.listSessionsForWorkspace(path, 50);
    } catch (err) {
      // One unreadable workspace is not a reason to lose the other twenty.
      RemoteLogger.warn(`load workspace sessions failed path=${path}: ${String(err)}`);
      return [];
    }
  }
}
