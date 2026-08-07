import { RemoteSession } from '../../model/RemoteModels';

export class ConversationSessionFilterPolicy {
  static matches(
    session: RemoteSession,
    query: string,
    fallbackWorkspacePath: string,
    workspaceFilter: string,
    agentFilter: string,
    statusFilter: string,
    assistantSession: boolean
  ): boolean {
    if (session.id.length === 0 || session.status === 'archived') {
      return false;
    }
    const normalizedQuery = query.trim().toLowerCase();
    if (normalizedQuery.length > 0 && session.title.toLowerCase().indexOf(normalizedQuery) < 0) {
      return false;
    }
    const workspacePath = session.workspacePath || (assistantSession ? '' : fallbackWorkspacePath);
    if (workspaceFilter.length > 0 &&
      !ConversationSessionFilterPolicy.workspacePathsEqual(workspacePath, workspaceFilter)) {
      return false;
    }
    if (agentFilter.length > 0 && ConversationSessionFilterPolicy.agentGroup(session, assistantSession) !== agentFilter) {
      return false;
    }
    const status = (session.status || '').trim().toLowerCase();
    return statusFilter.length === 0 || status === statusFilter;
  }

  static agentGroup(session: RemoteSession, assistantSession: boolean): string {
    if (assistantSession) {
      return 'chat';
    }
    return (session.agentType || '').toLowerCase() === 'cowork' ? 'cowork' : 'code';
  }

  static workspacePathsEqual(left: string, right: string): boolean {
    return ConversationSessionFilterPolicy.normalizeWorkspacePath(left) ===
      ConversationSessionFilterPolicy.normalizeWorkspacePath(right);
  }

  private static normalizeWorkspacePath(path: string): string {
    let value = path.trim();
    while (value.length > 1 && (value.endsWith('/') || value.endsWith('\\'))) {
      value = value.slice(0, value.length - 1);
    }
    return value;
  }
}
