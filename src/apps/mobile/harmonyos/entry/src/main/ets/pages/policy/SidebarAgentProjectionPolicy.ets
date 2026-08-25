import { RemoteSession } from '../../model/RemoteModels';

/**
 * Projects persisted remote sessions as user-facing Agent instances.
 *
 * The wire contract remains session-based for upgrade compatibility. The
 * sidebar shows only the Agent currently controlled by the phone. Historical
 * sessions remain persisted and reachable from dedicated history surfaces;
 * they do not become hundreds of fake "Agents" in the device tree.
 */
export class SidebarAgentProjectionPolicy {
  static activeAgents(
    sessions: RemoteSession[],
    selectedSessionId: string,
    runningSessionId: string
  ): RemoteSession[] {
    const ordered: RemoteSession[] = [];
    SidebarAgentProjectionPolicy.appendById(ordered, sessions, runningSessionId);
    if (selectedSessionId !== runningSessionId) {
      SidebarAgentProjectionPolicy.appendById(ordered, sessions, selectedSessionId);
    }
    return ordered;
  }

  private static appendById(target: RemoteSession[], sessions: RemoteSession[], id: string): void {
    if (id.length === 0) {
      return;
    }
    const found = sessions.find((item: RemoteSession): boolean => item.id === id);
    if (found) {
      target.push(found);
    }
  }
}
