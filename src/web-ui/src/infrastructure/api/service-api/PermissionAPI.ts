import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';

export type ProjectPermissionEffect = 'allow' | 'ask' | 'deny';

export interface ProjectPermissionRule {
  action: string;
  resource: string;
  effect: ProjectPermissionEffect;
}

/**
 * User-configured sensitive-resource markers, split by protection class.
 * `read` markers escalate/deny any access to those resources; `write`
 * markers keep write-class operations (edit/write/bash) out of aggressive
 * auto-approval while leaving their names visible to the judge.
 */
export interface ProjectSensitiveResources {
  read: string[];
  write: string[];
}

export interface ProjectPermissionRulesResponse {
  rules: ProjectPermissionRule[];
  sensitiveResources: ProjectSensitiveResources;
  revision: string;
}

const EMPTY_PROJECT_SENSITIVE_RESOURCES: ProjectSensitiveResources = {
  read: [],
  write: [],
};

/**
 * Normalizes the backend's sensitive-resources payload.
 *
 * An empty configuration serializes as an object without `read`/`write`
 * arrays, and older backends may omit the field entirely. Both shapes must
 * degrade to empty lists instead of leaking `undefined` into UI state.
 */
function normalizeProjectSensitiveResources(value: unknown): ProjectSensitiveResources {
  if (!value || typeof value !== 'object') {
    return { ...EMPTY_PROJECT_SENSITIVE_RESOURCES };
  }
  const record = value as { read?: unknown; write?: unknown };
  const toStringArray = (input: unknown): string[] =>
    Array.isArray(input) ? input.filter((item): item is string => typeof item === 'string') : [];
  return {
    read: toStringArray(record.read),
    write: toStringArray(record.write),
  };
}

function normalizeProjectRulesResponse(
  response: ProjectPermissionRulesResponse,
): ProjectPermissionRulesResponse {
  return {
    ...response,
    rules: Array.isArray(response?.rules) ? response.rules : [],
    sensitiveResources: normalizeProjectSensitiveResources(response?.sensitiveResources),
  };
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
      const response = await api.invoke<ProjectPermissionRulesResponse>('get_project_permission_rules', { request });
      return normalizeProjectRulesResponse(response);
    } catch (error) {
      throw createTauriCommandError('get_project_permission_rules', error, request);
    }
  }

  async saveProjectRules(
    workspaceId: string,
    rules: ProjectPermissionRule[],
    sensitiveResources: ProjectSensitiveResources,
    revision: string,
  ): Promise<ProjectPermissionRulesResponse> {
    const request = { workspaceId, rules, sensitiveResources, revision };
    try {
      const response = await api.invoke<ProjectPermissionRulesResponse>('save_project_permission_rules', { request });
      return normalizeProjectRulesResponse(response);
    } catch (error) {
      throw createTauriCommandError('save_project_permission_rules', error, request);
    }
  }
}

export const permissionAPI = new PermissionAPI();
