import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const workspaceItemSource = readFileSync(
  fileURLToPath(new URL('./WorkspaceItem.tsx', import.meta.url)),
  'utf8',
);

describe('WorkspaceItem session batch management', () => {
  it('exposes the shared batch manager from both assistant and project items', () => {
    const assistantBranchStart = workspaceItemSource.indexOf(
      'if (workspace.workspaceKind === WorkspaceKind.Assistant)',
    );
    const projectBranchStart = workspaceItemSource.indexOf('\n  return (', assistantBranchStart);

    expect(assistantBranchStart).toBeGreaterThanOrEqual(0);
    expect(projectBranchStart).toBeGreaterThan(assistantBranchStart);

    const assistantBranch = workspaceItemSource.slice(assistantBranchStart, projectBranchStart);
    const projectBranch = workspaceItemSource.slice(projectBranchStart);

    for (const branch of [assistantBranch, projectBranch]) {
      expect(branch).toContain('data-testid="nav-workspace-menu-manage-sessions"');
      expect(branch).toContain('onClick={handleOpenSessionBatchModal}');
      expect(branch).toContain('<RetainedMountBoundary present={sessionBatchModalOpen}>');
      expect(branch).toContain('workspacePath={workspace.rootPath}');
      expect(branch).toContain('workspaceLabel={workspaceDisplayName}');
    }
  });
});
