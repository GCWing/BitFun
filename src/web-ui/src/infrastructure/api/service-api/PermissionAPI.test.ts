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

    const response = await permissionAPI.getProjectRules('workspace-1');

    expect(invokeMock).toHaveBeenCalledWith('get_project_permission_rules', {
      request: { workspaceId: 'workspace-1' },
    });
    expect(response.sensitiveResources).toEqual({ read: [], write: [] });
  });

  it('normalizes an empty sensitive-resources object into typed empty lists', async () => {
    // An empty backend configuration serializes without read/write arrays.
    invokeMock.mockResolvedValueOnce({ rules: [], sensitiveResources: {}, revision: 'r1' });
    const { permissionAPI } = await import('./PermissionAPI');

    const response = await permissionAPI.getProjectRules('workspace-1');

    expect(response.sensitiveResources).toEqual({ read: [], write: [] });
  });

  it('normalizes a missing sensitive-resources payload into typed empty lists', async () => {
    // Older backends may omit the field entirely; the dialog must never see
    // `undefined.read`.
    invokeMock.mockResolvedValueOnce({ rules: [], revision: 'r2' });
    const { permissionAPI } = await import('./PermissionAPI');

    const response = await permissionAPI.getProjectRules('workspace-1');

    expect(response.sensitiveResources).toEqual({ read: [], write: [] });
  });

  it('drops non-string entries from sensitive-resource lists instead of crashing', async () => {
    invokeMock.mockResolvedValueOnce({
      rules: [],
      sensitiveResources: { read: ['secrets/', 42, null], write: 'not-an-array' },
      revision: 'r3',
    });
    const { permissionAPI } = await import('./PermissionAPI');

    const response = await permissionAPI.getProjectRules('workspace-1');

    expect(response.sensitiveResources).toEqual({ read: ['secrets/'], write: [] });
  });

  it('saves static rules with the revision returned by the backend', async () => {
    invokeMock.mockResolvedValueOnce({
      rules: [],
      sensitiveResources: { read: [], write: [] },
      revision: 'revision-2',
    });
    const { permissionAPI } = await import('./PermissionAPI');

    await permissionAPI.saveProjectRules(
      'workspace-1',
      [{ action: 'edit', resource: 'src/*', effect: 'ask' }],
      { read: ['secrets/'], write: ['crash-reports/'] },
      'revision-1',
    );

    expect(invokeMock).toHaveBeenCalledWith('save_project_permission_rules', {
      request: {
        workspaceId: 'workspace-1',
        rules: [{ action: 'edit', resource: 'src/*', effect: 'ask' }],
        sensitiveResources: { read: ['secrets/'], write: ['crash-reports/'] },
        revision: 'revision-1',
      },
    });
  });
});
