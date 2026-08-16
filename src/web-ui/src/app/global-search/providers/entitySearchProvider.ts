import { WorkspaceKind } from '@/shared/types';
import { getWorkspaceDisplayName } from '@/infrastructure/contexts/WorkspaceContext';
import { scoreTextMatch } from '../searchMatching';
import type { GlobalSearchItem, GlobalSearchProvider } from '../types';

function recencyScore(index: number): number {
  return Math.max(60, 78 - index);
}

export const entitySearchProvider: GlobalSearchProvider = {
  id: 'entities',
  groups: ['workspaces', 'assistants'],
  search: (request) => {
    if (request.scope !== 'all') return { items: [] };

    const ordered = [...request.workspaces].sort((left, right) =>
      Date.parse(right.lastAccessed || right.openedAt) - Date.parse(left.lastAccessed || left.openedAt)
    );
    const items: GlobalSearchItem[] = [];

    ordered.forEach((workspace, index) => {
      const assistant = workspace.workspaceKind === WorkspaceKind.Assistant;
      const title = getWorkspaceDisplayName(workspace);
      const score = request.query
        ? scoreTextMatch(request.query, [
            title,
            workspace.name,
            workspace.rootPath,
            workspace.description,
            ...(workspace.tags ?? []),
          ])
        : recencyScore(index);
      if (request.query && score === 0) return;

      items.push({
        id: `${assistant ? 'assistant' : 'workspace'}:${workspace.id}`,
        providerId: 'entities',
        group: assistant ? 'assistants' : 'workspaces',
        title,
        subtitle: assistant ? workspace.description : workspace.rootPath,
        badge: workspace.connectionName,
        score,
        target: assistant
          ? { kind: 'assistant', workspaceId: workspace.id }
          : { kind: 'workspace', workspaceId: workspace.id },
      });
    });

    return { items };
  },
};
