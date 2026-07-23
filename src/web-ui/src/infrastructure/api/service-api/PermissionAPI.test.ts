import { beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('./ApiClient', () => ({ api: { invoke: invokeMock } }));

describe('PermissionAPI', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('scopes grant listing to a backend workspace id', async () => {
    invokeMock.mockResolvedValueOnce([]);
    const { permissionAPI } = await import('./PermissionAPI');

    await permissionAPI.listProjectGrants('workspace-1');

    expect(invokeMock).toHaveBeenCalledWith('list_project_permission_grants', {
      request: { workspaceId: 'workspace-1' },
    });
  });

  it('removes grants without accepting a frontend project id', async () => {
    invokeMock.mockResolvedValueOnce(true);
    const { permissionAPI } = await import('./PermissionAPI');

    await permissionAPI.removeProjectGrant('workspace-1', {
      action: 'edit',
      resource: 'src/main.rs',
    });

    expect(invokeMock).toHaveBeenCalledWith('remove_project_permission_grant', {
      request: {
        workspaceId: 'workspace-1',
        action: 'edit',
        resource: 'src/main.rs',
      },
    });
  });

  it('clears grants using only the backend workspace id', async () => {
    invokeMock.mockResolvedValueOnce(2);
    const { permissionAPI } = await import('./PermissionAPI');

    await permissionAPI.clearProjectGrants('workspace-1');

    expect(invokeMock).toHaveBeenCalledWith('clear_project_permission_grants', {
      request: { workspaceId: 'workspace-1' },
    });
  });

  it('loads static rules using only the backend workspace id', async () => {
    invokeMock.mockResolvedValueOnce({ rules: [], revision: 'revision-1' });
    const { permissionAPI } = await import('./PermissionAPI');

    await permissionAPI.getProjectRules('workspace-1');

    expect(invokeMock).toHaveBeenCalledWith('get_project_permission_rules', {
      request: { workspaceId: 'workspace-1' },
    });
  });

  it('saves static rules with the revision returned by the backend', async () => {
    invokeMock.mockResolvedValueOnce({ rules: [], revision: 'revision-2' });
    const { permissionAPI } = await import('./PermissionAPI');

    await permissionAPI.saveProjectRules('workspace-1', [
      { action: 'edit', resource: 'src/*', effect: 'ask' },
    ], 'revision-1');

    expect(invokeMock).toHaveBeenCalledWith('save_project_permission_rules', {
      request: {
        workspaceId: 'workspace-1',
        rules: [{ action: 'edit', resource: 'src/*', effect: 'ask' }],
        revision: 'revision-1',
      },
    });
  });
});
