import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';

export type ProjectPermissionEffect = 'allow' | 'ask' | 'deny';

export interface ProjectPermissionRule {
  action: string;
  resource: string;
  effect: ProjectPermissionEffect;
}

export interface ProjectPermissionRulesResponse {
  rules: ProjectPermissionRule[];
  revision: string;
}

export interface PermissionGrant {
  projectId: string;
  action: string;
  resource: string;
  createdAtMs: number;
}

class PermissionAPI {
  async listProjectGrants(workspaceId: string): Promise<PermissionGrant[]> {
    try {
      return await api.invoke<PermissionGrant[]>('list_project_permission_grants', {
        request: { workspaceId },
      });
    } catch (error) {
      throw createTauriCommandError('list_project_permission_grants', error, { workspaceId });
    }
  }

  async removeProjectGrant(workspaceId: string, grant: Pick<PermissionGrant, 'action' | 'resource'>): Promise<boolean> {
    const request = { workspaceId, action: grant.action, resource: grant.resource };
    try {
      return await api.invoke<boolean>('remove_project_permission_grant', { request });
    } catch (error) {
      throw createTauriCommandError('remove_project_permission_grant', error, request);
    }
  }

  async clearProjectGrants(workspaceId: string): Promise<number> {
    try {
      return await api.invoke<number>('clear_project_permission_grants', {
        request: { workspaceId },
      });
    } catch (error) {
      throw createTauriCommandError('clear_project_permission_grants', error, { workspaceId });
    }
  }

  async getProjectRules(workspaceId: string): Promise<ProjectPermissionRulesResponse> {
    const request = { workspaceId };
    try {
      return await api.invoke<ProjectPermissionRulesResponse>('get_project_permission_rules', { request });
    } catch (error) {
      throw createTauriCommandError('get_project_permission_rules', error, request);
    }
  }

  async saveProjectRules(
    workspaceId: string,
    rules: ProjectPermissionRule[],
    revision: string,
  ): Promise<ProjectPermissionRulesResponse> {
    const request = { workspaceId, rules, revision };
    try {
      return await api.invoke<ProjectPermissionRulesResponse>('save_project_permission_rules', { request });
    } catch (error) {
      throw createTauriCommandError('save_project_permission_rules', error, request);
    }
  }
}

export const permissionAPI = new PermissionAPI();
